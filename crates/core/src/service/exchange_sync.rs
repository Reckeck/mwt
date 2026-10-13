//! Syncing an exchange wallet: what an import brings in (its settings), the
//! history of syncs, and setting the exchange's live balance when the ledger
//! disagrees with it after everything is booked.
//!
//! The ledger stays the source of truth: balances come from operations. The
//! live balance only enters as a correction («Корректировка»), so history is
//! kept and a later import still lands before it.

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::exchange::CoinBalance;
use crate::model::category::{Category, CategoryGroup, EXCHANGE_IMPORT_CATEGORY};
use crate::model::store::{ImportSettings, StoreKind};
use crate::model::transaction::{
    AdjustmentLine, AdjustmentTx, PriceStamp, Transaction, TxMeta, TxOrigin,
};
use crate::model::vault::{
    ImportKind, ImportRun, ImportSource, MAX_DISCARDED_IMPORTS, MAX_IMPORT_RUNS, PendingImport,
};

use super::UnlockedVault;

/// The exchange accounts an import knows: Bybit's Funding, Unified Trading, Earn.
pub const IMPORT_ACCOUNTS: [&str; 3] = [
    crate::exchange::bybit::ACCOUNT_FUND,
    crate::exchange::bybit::ACCOUNT_UNIFIED,
    crate::exchange::bybit::ACCOUNT_EARN,
];

/// Most records a preview lists; the counts cover them all.
pub const MAX_PREVIEW_ITEMS: usize = 200;

/// What a sync would bring, by the import settings: nothing is kept.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ImportPreview {
    /// Records the exchange sent.
    pub fetched: u32,
    /// Seen before: staged, booked or discarded.
    pub known: u32,
    /// Left out by the settings.
    pub filtered: u32,
    /// Would be new.
    pub new: u32,
    /// The new ones, newest first, at most [`MAX_PREVIEW_ITEMS`].
    pub items: Vec<PendingImport>,
}

/// Imported operations of one exchange wallet, newest first, a page at a time.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct ImportedTransactions {
    pub total: u32,
    pub items: Vec<Transaction>,
}

/// The account type an account name stands for, as the settings store it.
fn account_key(account: &str) -> String {
    match account.trim().to_ascii_uppercase().as_str() {
        "SPOT" | "CONTRACT" | "UNIFIED" => crate::exchange::bybit::ACCOUNT_UNIFIED.to_string(),
        "INVESTMENT" | "EARN" => crate::exchange::bybit::ACCOUNT_EARN.to_string(),
        other => other.to_string(),
    }
}

impl UnlockedVault {
    /// The import settings of the exchange wallet `store_id` belongs to.
    pub fn import_settings(&self, store_id: &str) -> Result<ImportSettings> {
        let root = self.root_of(store_id);
        let store = self
            .find_store(&root)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        Ok(store
            .exchange
            .as_ref()
            .map(|profile| profile.import.clone())
            .unwrap_or_default())
    }

    /// Change what an import brings in and where it books it.
    pub fn set_import_settings(
        &mut self,
        store_id: &str,
        mut settings: ImportSettings,
    ) -> Result<ImportSettings> {
        let root = self.root_of(store_id);
        let mut seen = HashSet::new();
        settings.coins = settings
            .coins
            .into_iter()
            .map(|coin| coin.trim().to_ascii_uppercase())
            .filter(|coin| !coin.is_empty() && seen.insert(coin.clone()))
            .collect();
        settings.since = settings
            .since
            .map(|since| since.trim().to_string())
            .filter(|since| !since.is_empty());
        if let Some(since) = &settings.since
            && crate::pricing::history::unix_secs(since).is_none()
        {
            return Err(Error::InvalidInput(format!("not a date: {since}")));
        }
        for key in [
            &settings.categories.trades,
            &settings.categories.transfers,
            &settings.categories.earn,
        ]
        .into_iter()
        .flatten()
        {
            if !self
                .doc
                .categories
                .iter()
                .any(|category| category.key == *key)
            {
                return Err(Error::InvalidInput(format!("unknown category {key}")));
            }
        }
        let mut accounts = HashSet::new();
        for route in &mut settings.accounts {
            route.account = account_key(&route.account);
            if !IMPORT_ACCOUNTS.contains(&route.account.as_str()) {
                return Err(Error::InvalidInput(format!(
                    "unknown exchange account {}",
                    route.account
                )));
            }
            if !accounts.insert(route.account.clone()) {
                return Err(Error::InvalidInput(format!(
                    "the account {} is listed twice",
                    route.account
                )));
            }
            if let Some(target) = &route.store_id {
                let store = self.live_store(target)?;
                if store.kind != StoreKind::Exchange || self.root_of(target) != root {
                    return Err(Error::InvalidInput(
                        "an exchange account books only into this wallet or its sub-accounts"
                            .into(),
                    ));
                }
            }
        }
        let profile = self.exchange_profile_mut(&root)?;
        profile.import = settings.clone();
        self.persist()?;
        Ok(settings)
    }

    /// The wallet an exchange account is routed to by the settings:
    /// `None` = not set (the bound sub-account), `Some(None)` = not imported.
    pub(crate) fn import_route(&self, root_id: &str, account: &str) -> Option<Option<String>> {
        let settings = self.find_store(root_id)?.exchange.as_ref()?;
        self.route_in(&settings.import, account)
    }

    /// [`Self::import_route`] by the given settings (saved or not yet).
    fn route_in(&self, settings: &ImportSettings, account: &str) -> Option<Option<String>> {
        let key = account_key(account);
        settings
            .accounts
            .iter()
            .find(|route| route.account == key)
            .map(|route| {
                route
                    .store_id
                    .clone()
                    .filter(|id| self.find_store(id).is_some_and(|store| !store.archived))
            })
    }

    fn account_skipped(&self, settings: &ImportSettings, account: Option<&str>) -> bool {
        account.is_some_and(|account| matches!(self.route_in(settings, account), Some(None)))
    }

    /// Whether the settings let a staged record in.
    fn import_wanted(&self, imp: &PendingImport, settings: &ImportSettings) -> bool {
        use crate::exchange::bybit::{ACCOUNT_EARN, ACCOUNT_UNIFIED};
        let (kind_on, accounts) = match imp.kind {
            ImportKind::Fill => (settings.trades, vec![Some(ACCOUNT_UNIFIED)]),
            ImportKind::Transfer => (
                settings.transfers,
                vec![imp.from_account.as_deref(), imp.to_account.as_deref()],
            ),
            ImportKind::EarnIncome => (settings.earn, vec![Some(ACCOUNT_EARN)]),
        };
        if !kind_on
            || accounts
                .into_iter()
                .any(|account| self.account_skipped(settings, account))
        {
            return false;
        }
        if !settings.coins.is_empty() {
            let named = [&imp.asset_bought, &imp.asset_sold]
                .into_iter()
                .any(|coin| settings.coins.contains(&coin.to_ascii_uppercase()));
            if !named {
                return false;
            }
        }
        match (
            &settings.since,
            crate::pricing::history::unix_secs(&imp.occurred_at),
        ) {
            (Some(since), Some(at)) => {
                crate::pricing::history::unix_secs(since).is_none_or(|since| at >= since)
            }
            _ => true,
        }
    }

    /// The categories an imported operation gets: the one the settings name
    /// for its kind (or `default`), and the import mark.
    pub(crate) fn import_categories(&self, imp: &PendingImport, default: &str) -> Vec<String> {
        let settings = self.import_settings(&imp.store_id).unwrap_or_default();
        let chosen = match imp.kind {
            ImportKind::Fill => settings.categories.trades,
            ImportKind::Transfer => settings.categories.transfers,
            ImportKind::EarnIncome => settings.categories.earn,
        };
        vec![
            chosen.unwrap_or_else(|| default.to_string()),
            EXCHANGE_IMPORT_CATEGORY.to_string(),
        ]
    }

    /// The import mark exists in vaults made before it.
    pub(crate) fn ensure_import_category(&mut self) {
        if !self
            .doc
            .categories
            .iter()
            .any(|category| category.key == EXCHANGE_IMPORT_CATEGORY)
        {
            self.doc.categories.push(Category::builtin(
                EXCHANGE_IMPORT_CATEGORY,
                CategoryGroup::Service,
            ));
        }
    }

    /// Ids an import has already seen: staged, booked or discarded.
    pub(crate) fn known_import_ids(&self) -> HashSet<String> {
        let mut known: HashSet<String> = self
            .doc
            .pending_imports
            .iter()
            .map(|pending| pending.exec_id.clone())
            .collect();
        known.extend(self.doc.discarded_imports.iter().cloned());
        known.extend(
            self.doc
                .transactions
                .iter()
                .filter_map(|tx| match &tx.meta().origin {
                    TxOrigin::Import { exec_id } => Some(exec_id.clone()),
                    _ => None,
                }),
        );
        known
    }

    /// One sync of an exchange wallet: leave out what the settings do not
    /// want and what was seen before, stage the rest (or book it at once),
    /// and keep the sync in the history.
    pub fn sync_exchange(
        &mut self,
        store_id: &str,
        imports: Vec<PendingImport>,
        source: ImportSource,
        now: i64,
    ) -> Result<ImportRun> {
        let root = self.root_of(store_id);
        let settings = self.import_settings(&root)?;
        let mut known = self.known_import_ids();
        let mut run = ImportRun {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: root.clone(),
            at: crate::calendar::iso(now),
            source,
            fetched: imports.len() as u32,
            ..ImportRun::default()
        };
        let mut fresh = Vec::new();
        for imp in imports {
            if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
                run.known += 1;
                continue;
            }
            if !self.import_wanted(&imp, &settings) {
                run.filtered += 1;
                continue;
            }
            known.insert(imp.exec_id.clone());
            fresh.push(imp);
        }
        let ids: Vec<String> = fresh.iter().map(|imp| imp.exec_id.clone()).collect();
        run.staged = self.stage_imports(fresh)?;
        if settings.auto_confirm && !ids.is_empty() {
            run.confirmed = self.confirm_pending_imports(&ids)?;
        }
        self.keep_import_run(run.clone());
        self.persist()?;
        Ok(run)
    }

    /// What a sync would bring under `settings` (the saved ones, or the ones
    /// still being edited): nothing is staged or kept.
    pub fn preview_exchange_sync(
        &self,
        imports: Vec<PendingImport>,
        settings: &ImportSettings,
    ) -> ImportPreview {
        let known = self.known_import_ids();
        let mut preview = ImportPreview {
            fetched: imports.len() as u32,
            ..ImportPreview::default()
        };
        for imp in imports {
            if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
                preview.known += 1;
            } else if !self.import_wanted(&imp, settings) {
                preview.filtered += 1;
            } else {
                preview.new += 1;
                if preview.items.len() < MAX_PREVIEW_ITEMS {
                    preview.items.push(imp);
                }
            }
        }
        preview
    }

    /// A sync that did not go through, for the history.
    pub fn record_import_error(
        &mut self,
        store_id: &str,
        source: ImportSource,
        error: &str,
        now: i64,
    ) -> Result<ImportRun> {
        let run = ImportRun {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: self.root_of(store_id),
            at: crate::calendar::iso(now),
            source,
            error: Some(error.to_string()),
            ..ImportRun::default()
        };
        self.keep_import_run(run.clone());
        self.persist()?;
        Ok(run)
    }

    fn keep_import_run(&mut self, run: ImportRun) {
        self.doc.import_runs.push(run);
        let overflow = self.doc.import_runs.len().saturating_sub(MAX_IMPORT_RUNS);
        self.doc.import_runs.drain(..overflow);
    }

    /// Discarded ids are remembered, so the next sync does not stage them again.
    pub(crate) fn remember_discarded(&mut self, ids: impl IntoIterator<Item = String>) {
        self.doc.discarded_imports.extend(ids);
        let overflow = self
            .doc
            .discarded_imports
            .len()
            .saturating_sub(MAX_DISCARDED_IMPORTS);
        self.doc.discarded_imports.drain(..overflow);
    }

    /// Syncs of one exchange wallet, newest first.
    pub fn import_runs(&self, store_id: &str) -> Vec<ImportRun> {
        let root = self.root_of(store_id);
        self.doc
            .import_runs
            .iter()
            .rev()
            .filter(|run| run.store_id == root)
            .cloned()
            .collect()
    }

    /// Operations brought in from the exchange wallet `store_id` belongs to
    /// (its own and its sub-accounts'), newest first.
    pub fn imported_transactions(
        &self,
        store_id: &str,
        limit: usize,
        offset: usize,
    ) -> ImportedTransactions {
        let root = self.root_of(store_id);
        let mut found: Vec<&Transaction> = self
            .doc
            .transactions
            .iter()
            .filter(|tx| matches!(tx.meta().origin, TxOrigin::Import { .. }))
            .filter(|tx| {
                tx.legs()
                    .iter()
                    .any(|leg| self.root_of(&leg.store_id) == root)
                    || matches!(tx, Transaction::Adjustment(inner) if self.root_of(&inner.store_id) == root)
            })
            .collect();
        found.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        ImportedTransactions {
            total: found.len() as u32,
            items: found
                .into_iter()
                .skip(offset)
                .take(limit)
                .cloned()
                .collect(),
        }
    }

    /// Set the live balance of each exchange account where the ledger
    /// disagrees with it, by one correction per sub-account. Nothing happens
    /// while operations of this wallet wait for confirmation (they explain
    /// the difference) or when the settings turn it off. Returns the ids of
    /// the corrections made.
    pub fn reconcile_exchange(
        &mut self,
        store_id: &str,
        balances: &[(String, Vec<CoinBalance>)],
        now: i64,
    ) -> Result<Vec<String>> {
        let root = self.root_of(store_id);
        let settings = self.import_settings(&root)?;
        let waiting = self
            .doc
            .pending_imports
            .iter()
            .any(|pending| self.root_of(&pending.store_id) == root);
        if !settings.auto_reconcile || waiting {
            return Ok(Vec::new());
        }
        let at = crate::calendar::iso(now);
        let mut made = Vec::new();
        for account in self.bybit_reconcile(&root, balances) {
            let Some(target) = account.store_id.clone() else {
                continue;
            };
            if self.account_skipped(&settings, Some(&account.account)) {
                continue;
            }
            let lines: Vec<AdjustmentLine> = account
                .rows
                .iter()
                .filter(|row| !row.diff.is_zero() && !row.exchange.is_negative())
                .filter(|row| {
                    settings.coins.is_empty()
                        || settings.coins.contains(&row.coin.to_ascii_uppercase())
                })
                .map(|row| AdjustmentLine {
                    asset: self.held_asset(&target, &row.coin),
                    before: row.ledger,
                    after: row.exchange,
                    price: PriceStamp::default(),
                })
                .collect();
            if lines.is_empty() {
                continue;
            }
            // A correction has no categories: the import origin marks it.
            let mut meta = TxMeta::new("Сверка с биржей: живой баланс");
            meta.timestamp = at.clone();
            meta.origin = TxOrigin::Import {
                exec_id: format!("reconcile:{}:{at}", account.account),
            };
            let saved = self.add_transaction(Transaction::Adjustment(AdjustmentTx {
                meta,
                store_id: target,
                lines,
                fee: None,
            }))?;
            made.push(saved.meta().id.clone());
        }
        if let Some(run) = self
            .doc
            .import_runs
            .iter_mut()
            .rev()
            .find(|run| run.store_id == root)
            .filter(|_| !made.is_empty())
        {
            run.reconcile_tx_id = made.first().cloned();
            self.persist()?;
        }
        Ok(made)
    }

    /// The identity a store already holds a coin under, else the import's.
    fn held_asset(&self, store_id: &str, coin: &str) -> crate::model::store::AssetRef {
        self.doc
            .holdings
            .iter()
            .find(|holding| {
                holding.store_id == store_id
                    && !holding.asset.is_derivative()
                    && holding.asset.symbol.eq_ignore_ascii_case(coin)
            })
            .map(|holding| holding.asset.clone())
            .unwrap_or_else(|| self.asset_for_import(coin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Amount;
    use crate::exchange::{CoinBalance, Execution};
    use crate::model::store::ImportAccountRoute;
    use crate::model::transaction::TxKind;
    use crate::service::StoreExtras;

    const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];

    /// A Bybit wallet with its three accounts: `(dir, vault, wallet, unified)`.
    fn bybit() -> (tempfile::TempDir, UnlockedVault, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
        let wallet = vault
            .create_store_with(
                "Bybit",
                StoreKind::Exchange,
                "#f0b90b",
                "",
                None,
                StoreExtras {
                    provider: Some(crate::model::store::ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let [_, unified, _] = vault.ensure_bybit_accounts(&wallet, ACCOUNT_NAMES).unwrap();
        (dir, vault, wallet, unified)
    }

    fn buy(store: &str, exec_id: &str, symbol: &str, time_ms: &str) -> PendingImport {
        let exec = Execution {
            exec_id: exec_id.into(),
            symbol: symbol.into(),
            side: "Buy".into(),
            order_type: "Market".into(),
            exec_price: "100".into(),
            exec_qty: "1".into(),
            exec_value: "100".into(),
            exec_fee: "0".into(),
            exec_time: time_ms.into(),
            is_maker: false,
            closed_size: None,
            category: String::new(),
        };
        PendingImport::from_execution(store, "spot", &exec).unwrap()
    }

    fn balance(account: &str, coins: &[(&str, &str)]) -> (String, Vec<CoinBalance>) {
        (
            account.to_string(),
            coins
                .iter()
                .map(|(coin, amount)| CoinBalance {
                    coin: (*coin).into(),
                    amount: (*amount).into(),
                })
                .collect(),
        )
    }

    fn now() -> i64 {
        crate::calendar::now_secs()
    }

    #[test]
    fn a_sync_never_stages_what_was_booked_or_discarded() {
        let (_dir, mut vault, wallet, _) = bybit();
        let first = vault
            .sync_exchange(
                &wallet,
                vec![
                    buy(&wallet, "a", "SOLUSDT", "1700000000000"),
                    buy(&wallet, "b", "SOLUSDT", "1700000000000"),
                ],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        assert_eq!((first.fetched, first.staged), (2, 2));
        vault.confirm_pending_imports(&["a".to_string()]).unwrap();
        vault.discard_pending_imports(&["b".to_string()]).unwrap();
        let again = vault
            .sync_exchange(
                &wallet,
                vec![
                    buy(&wallet, "a", "SOLUSDT", "1700000000000"),
                    buy(&wallet, "b", "SOLUSDT", "1700000000000"),
                ],
                ImportSource::Auto,
                now(),
            )
            .unwrap();
        assert_eq!((again.staged, again.known), (0, 2));
        assert_eq!(vault.import_runs(&wallet).len(), 2);
    }

    #[test]
    fn imported_operations_carry_the_import_mark_and_a_known_coin_id() {
        let (_dir, mut vault, wallet, _) = bybit();
        vault
            .sync_exchange(
                &wallet,
                vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        vault.confirm_pending_imports(&[]).unwrap();
        let tx = vault.doc.transactions.last().unwrap();
        assert!(
            tx.meta()
                .categories
                .contains(&EXCHANGE_IMPORT_CATEGORY.to_string())
        );
        assert!(
            vault
                .doc
                .categories
                .iter()
                .any(|category| category.key == EXCHANGE_IMPORT_CATEGORY)
        );
        let Transaction::TradeSpot(trade) = tx else {
            panic!("a spot trade");
        };
        assert_eq!(trade.bought.asset.coingecko_id.as_deref(), Some("solana"));
        assert_eq!(vault.imported_transactions(&wallet, 10, 0).total, 1);
    }

    #[test]
    fn the_settings_leave_out_kinds_coins_dates_and_accounts() {
        let (_dir, mut vault, wallet, _) = bybit();
        let settings = ImportSettings {
            coins: vec!["sol".into()],
            since: Some("2023-11-15T00:00:00Z".into()),
            ..ImportSettings::default()
        };
        vault.set_import_settings(&wallet, settings).unwrap();
        let run = vault
            .sync_exchange(
                &wallet,
                vec![
                    buy(&wallet, "sol-new", "SOLUSDT", "1700100000000"),
                    buy(&wallet, "sol-old", "SOLUSDT", "1690000000000"),
                    buy(&wallet, "eth-new", "ETHUSDT", "1700100000000"),
                ],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        assert_eq!((run.staged, run.filtered), (1, 2));

        // Unified not imported at all: fills are left out.
        let mut off = vault.import_settings(&wallet).unwrap();
        off.coins.clear();
        off.since = None;
        off.accounts = vec![ImportAccountRoute {
            account: "UNIFIED".into(),
            store_id: None,
        }];
        vault.set_import_settings(&wallet, off).unwrap();
        let run = vault
            .sync_exchange(
                &wallet,
                vec![buy(&wallet, "x", "SOLUSDT", "1700100000000")],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        assert_eq!((run.staged, run.filtered), (0, 1));
    }

    #[test]
    fn an_account_can_book_into_a_chosen_sub_account() {
        let (_dir, mut vault, wallet, _) = bybit();
        let [funding, _, _] = vault.ensure_bybit_accounts(&wallet, ACCOUNT_NAMES).unwrap();
        let settings = ImportSettings {
            accounts: vec![ImportAccountRoute {
                account: "unified".into(),
                store_id: Some(funding.clone()),
            }],
            auto_confirm: true,
            ..ImportSettings::default()
        };
        vault.set_import_settings(&wallet, settings).unwrap();
        let run = vault
            .sync_exchange(
                &wallet,
                vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
                ImportSource::Auto,
                now(),
            )
            .unwrap();
        assert_eq!(run.confirmed, 1);
        assert!(vault.pending_imports().is_empty());
        let Transaction::TradeSpot(trade) = vault.doc.transactions.last().unwrap() else {
            panic!("a spot trade");
        };
        assert_eq!(trade.store_id, funding);

        // A wallet outside this exchange is refused.
        let other = vault
            .create_store_with(
                "Card",
                StoreKind::Bank,
                "#000",
                "",
                None,
                StoreExtras::default(),
            )
            .unwrap()
            .id;
        let bad = ImportSettings {
            accounts: vec![ImportAccountRoute {
                account: "FUND".into(),
                store_id: Some(other),
            }],
            ..ImportSettings::default()
        };
        assert!(vault.set_import_settings(&wallet, bad).is_err());
    }

    #[test]
    fn the_live_balance_is_set_once_everything_is_booked() {
        let (_dir, mut vault, wallet, unified) = bybit();
        vault
            .sync_exchange(
                &wallet,
                vec![buy(&wallet, "a", "SOLUSDT", "1700000000000")],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        let live = vec![balance("UNIFIED", &[("SOL", "1.5"), ("USDT", "400")])];
        // Something waits for confirmation: it explains the difference.
        assert!(
            vault
                .reconcile_exchange(&wallet, &live, now())
                .unwrap()
                .is_empty()
        );

        vault.confirm_pending_imports(&[]).unwrap();
        let made = vault.reconcile_exchange(&wallet, &live, now()).unwrap();
        assert_eq!(made.len(), 1);
        let holding = |symbol: &str| {
            vault
                .doc
                .holdings
                .iter()
                .find(|holding| holding.store_id == unified && holding.asset.symbol == symbol)
                .map(|holding| holding.quantity)
                .unwrap_or_default()
        };
        assert_eq!(holding("SOL"), Amount::parse("1.5").unwrap());
        assert_eq!(holding("USDT"), Amount::parse("400").unwrap());
        let correction = vault.doc.transactions.last().unwrap();
        assert_eq!(correction.kind(), TxKind::Adjustment);
        assert!(matches!(correction.meta().origin, TxOrigin::Import { .. }));

        // In step now: nothing more to set.
        assert!(
            vault
                .reconcile_exchange(&wallet, &live, now() + 60)
                .unwrap()
                .is_empty()
        );

        // Turned off in the settings: the ledger is left alone.
        let mut settings = vault.import_settings(&wallet).unwrap();
        settings.auto_reconcile = false;
        vault.set_import_settings(&wallet, settings).unwrap();
        let moved = vec![balance("UNIFIED", &[("SOL", "2"), ("USDT", "400")])];
        assert!(
            vault
                .reconcile_exchange(&wallet, &moved, now() + 120)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_failed_sync_is_kept_with_its_reason() {
        let (_dir, mut vault, wallet, _) = bybit();
        vault
            .record_import_error(&wallet, ImportSource::Auto, "rate_limited", now())
            .unwrap();
        let runs = vault.import_runs(&wallet);
        assert_eq!(runs[0].error.as_deref(), Some("rate_limited"));
    }

    #[test]
    fn a_stablecoin_without_a_price_is_worth_a_dollar() {
        assert!(crate::exchange::is_usd_stablecoin("asset:USDT"));
        assert!(crate::exchange::is_usd_stablecoin("coin:tether"));
        assert!(!crate::exchange::is_usd_stablecoin("coin:bitcoin"));
        assert!(!crate::exchange::is_usd_stablecoin("fiat:USD"));
    }

    #[test]
    fn a_preview_follows_unsaved_settings_and_keeps_nothing() {
        let (_dir, mut vault, wallet, _) = bybit();
        vault
            .sync_exchange(
                &wallet,
                vec![buy(&wallet, "seen", "SOLUSDT", "1700000000000")],
                ImportSource::Manual,
                now(),
            )
            .unwrap();
        let fills = vec![
            buy(&wallet, "seen", "SOLUSDT", "1700000000000"),
            buy(&wallet, "sol", "SOLUSDT", "1700100000000"),
            buy(&wallet, "eth", "ETHUSDT", "1700100000000"),
        ];
        let draft = ImportSettings {
            coins: vec!["SOL".into()],
            ..ImportSettings::default()
        };
        let preview = vault.preview_exchange_sync(fills.clone(), &draft);
        assert_eq!(
            (
                preview.fetched,
                preview.known,
                preview.filtered,
                preview.new
            ),
            (3, 1, 1, 1)
        );
        assert_eq!(preview.items[0].exec_id, "sol");
        // Not one record staged, the saved settings untouched.
        assert_eq!(vault.pending_imports().len(), 1);
        assert!(vault.import_settings(&wallet).unwrap().coins.is_empty());
        // Skipping the unified account in the draft leaves every fill out.
        let skip = ImportSettings {
            accounts: vec![ImportAccountRoute {
                account: "UNIFIED".into(),
                store_id: None,
            }],
            ..ImportSettings::default()
        };
        assert_eq!(vault.preview_exchange_sync(fills, &skip).new, 0);
    }
}
