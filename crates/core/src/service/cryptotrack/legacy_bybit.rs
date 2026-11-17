//! Bybit on the engine, and the one-time conversion of what the old import
//! left behind (stage 2, spec E7).
//!
//! Until now an exchange sweep *staged* every record it found
//! (`VaultDoc::pending_imports`) and the user confirmed it into a ledger
//! transaction (`TxOrigin::Import { exec_id }`). From here on an exchange is
//! read by the crypto module like any other followed account: a record
//! becomes a [`CryptoTx`] at once, with no queue in between, and the
//! verification screen is gone.
//!
//! ## What must not be lost
//!
//! The old mechanics stay readable, because a user's file holds them
//! (`AGENTS.md`, rule 5: a stored field is never renamed, removed or
//! repurposed — it simply stops being written):
//!
//! * `pending_imports` — the queue. Its entries are *put through* by this
//!   conversion, one operation each. Nothing writes the field again.
//! * `discarded_imports` — records the user refused. They stay refused: a
//!   discarded `exec_id` is never converted and never fetched in again.
//! * `TxOrigin::Import { exec_id }` — records already booked as ledger
//!   transactions. Those transactions are left exactly as they are, and
//!   their `exec_id`s are treated as decided, so no operation of the module
//!   is written for them. That is what keeps the old records and the new
//!   events from both counting.
//! * `import_runs` — the sync history, still written and still read.
//!
//! ## Why it is idempotent
//!
//! An event's id is a function of the account and of the fill
//! ([`crate::cryptotrack::engine::ids::event`] over
//! [`crate::model::cryptotrack::TrackingRef::ExchangeHistory`]), the
//! account's own id is a function of the wallet's id
//! ([`bybit::account_id_for`]), and the price points the conversion writes
//! are keyed by `(asset, day)`. Not one of them reads a clock or a counter.
//! So running the conversion twice rewrites the same records with the same
//! bytes, and two devices that converted the same file hold the same records
//! — which is what lets the sync treat an operation as one record instead of
//! two.
//!
//! ## Where the cost basis comes from
//!
//! The engine prices every leg of an operation through the document's price
//! cache, by day ([`prices`]). A fill quoted in a dollar stablecoin states
//! its own dollar price exactly, so the conversion writes that price into the
//! cache instead of leaving the basis to a market quote fetched later. When a
//! day holds several fills of one asset, the point is their
//! quantity-weighted average — which is precisely what average cost needs:
//! the dollars the day's purchases added to the pool are the dollars the
//! fills actually cost (`the_pool_costs_what_the_fills_cost`). A fill quoted
//! in anything else (`ETHBTC`) implies no dollar price and gets none; the
//! engine then says the cost is not known rather than inventing one.

use std::collections::{BTreeMap, BTreeSet};

use crate::Amount;
use crate::cryptotrack::engine::math;
use crate::cryptotrack::prices;
use crate::cryptotrack::sources::bybit;
use crate::error::{Error, Result};
use crate::model::cryptotrack::{
    CryptoAccount, CryptoAsset, CryptoPricePoint, CryptoToken, TokenId, TokenVerdict,
};
use crate::model::store::StoreKind;
use crate::model::vault::PendingImport;

use super::super::UnlockedVault;

/// What storing a batch of exchange records did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BybitStored {
    /// Operations written for the first time.
    pub added: u32,
    /// Operations that were already there and were written again, byte for
    /// byte the same unless the exchange changed its answer.
    pub rewritten: u32,
    /// Records settled before: already an operation, already a ledger
    /// transaction, or refused by the user.
    pub known: u32,
    /// `exec_id`s the module could not read into an operation. They are
    /// never dropped: the caller leaves them where they were.
    pub unread: Vec<String>,
}

impl BybitStored {
    /// Operations this batch put into the module.
    pub fn recorded(&self) -> u32 {
        self.added + self.rewritten
    }
}

/// What the one-time conversion of the old queue did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacyAdoption {
    /// Queue entries that became operations of the module.
    pub converted: u32,
    /// Queue entries that were settled already (booked or refused) and were
    /// therefore only dropped from the queue, never counted twice.
    pub known: u32,
    /// Queue entries left in place because they could not be read or their
    /// wallet is gone. Nothing is ever thrown away silently.
    pub kept: u32,
}

impl UnlockedVault {
    /// Whether a wallet is read by the crypto module rather than kept by
    /// hand: an exchange with a key or with a followed account of its own.
    ///
    /// Spec E11 archives the old hand-kept crypto wallets when the user moves
    /// to level 3; a wallet the engine drives must stay, because its
    /// operations are refetched and its balance is recomputed. This is the
    /// mark E11 reads.
    pub fn exchange_is_engine_driven(&self, store_id: &str) -> bool {
        let root = self.root_of(store_id);
        let Some(store) = self.find_store(&root) else {
            return false;
        };
        if store.kind != StoreKind::Exchange {
            return false;
        }
        let followed = store.crypto_account_id.as_deref().is_some_and(|id| {
            self.doc
                .crypto_accounts
                .iter()
                .any(|account| account.id == id)
        });
        followed
            || self
                .doc
                .exchanges
                .bybit
                .iter()
                .any(|keys| self.root_of(&keys.store_id) == root)
    }

    /// `exec_id`s of the exchange operations the module already holds.
    pub(crate) fn bybit_event_ids(&self) -> BTreeSet<String> {
        self.doc
            .crypto_events
            .iter()
            .filter_map(bybit::exec_id_of)
            .map(str::to_owned)
            .collect()
    }

    /// The followed account of an exchange wallet, made if it is not there
    /// yet, and bound to the wallet (`Store::crypto_account_id`, decision
    /// P5).
    ///
    /// `earliest` is the moment of the oldest record about to be stored: it
    /// becomes the account's `created_at`, which is the date the module
    /// follows the exchange from. It only ever moves earlier, so a later
    /// batch of older fills widens the followed period and never narrows it —
    /// and no clock takes part, so both devices write the same account.
    pub(crate) fn bybit_account(&mut self, store_id: &str, earliest: &str) -> Result<String> {
        let root = self.root_of(store_id);
        let store = self
            .find_store(&root)
            .ok_or_else(|| Error::Missing(format!("store {root}")))?;
        if store.kind != StoreKind::Exchange {
            return Err(Error::InvalidInput(format!(
                "store {root} is not an exchange wallet"
            )));
        }
        let name = store.name.clone();
        let account_id = bybit::account_id_for(&root);
        let handle = bybit::account_handle(&root);
        match self
            .doc
            .crypto_accounts
            .iter_mut()
            .find(|account| account.id == account_id)
        {
            Some(account) => {
                if !earliest.is_empty()
                    && (account.created_at.is_empty() || *earliest < *account.created_at)
                {
                    account.created_at = earliest.to_owned();
                }
                account.source = Some(bybit::SOURCE_ID.to_owned());
            }
            None => {
                let mut account = CryptoAccount::new(
                    &account_id,
                    &name,
                    bybit::EXCHANGE_CHAIN,
                    &handle,
                    earliest,
                );
                account.source = Some(bybit::SOURCE_ID.to_owned());
                account.validate()?;
                self.doc.crypto_accounts.push(account);
            }
        }
        if let Some(store) = self.doc.stores.iter_mut().find(|store| store.id == root)
            && store.crypto_account_id.as_deref() != Some(account_id.as_str())
        {
            store.crypto_account_id = Some(account_id.clone());
        }
        Ok(account_id)
    }

    /// Store exchange records as operations of the module. The one way in:
    /// both a fresh sweep and the conversion of the old queue come through
    /// here, so neither can write an operation the other would not.
    ///
    /// Does not save; the caller does, once.
    pub(crate) fn bybit_store(
        &mut self,
        store_id: &str,
        records: &[PendingImport],
    ) -> Result<BybitStored> {
        let mut out = BybitStored::default();
        let decided = self.decided_import_ids();
        let mut held = self.bybit_event_ids();
        // Deterministic order: the day's price point is folded in this order,
        // and so is the account's start date.
        let mut wanted: Vec<&PendingImport> = Vec::new();
        for record in records {
            let exec_id = record.exec_id.trim();
            if exec_id.is_empty() {
                out.unread.push(record.exec_id.clone());
                continue;
            }
            if decided.contains(exec_id) || held.contains(exec_id) {
                out.known += 1;
                continue;
            }
            held.insert(exec_id.to_owned());
            wanted.push(record);
        }
        if wanted.is_empty() {
            return Ok(out);
        }
        wanted.sort_by(|left, right| {
            (&left.occurred_at, &left.exec_id).cmp(&(&right.occurred_at, &right.exec_id))
        });
        let earliest = wanted
            .first()
            .map(|record| record.occurred_at.clone())
            .unwrap_or_default();
        let account_id = self.bybit_account(store_id, &earliest)?;

        let mut events = Vec::new();
        let mut kept: Vec<&PendingImport> = Vec::new();
        for record in wanted {
            match bybit::event_from_pending(&account_id, record) {
                Some(event) => {
                    events.push(event);
                    kept.push(record);
                }
                None => out.unread.push(record.exec_id.clone()),
            }
        }
        for symbol in coins_of(&kept) {
            self.bybit_register_coin(&symbol);
        }
        for point in day_prices(&kept) {
            prices::apply_historic(&mut self.doc, &[point]);
        }
        for event in events {
            match self
                .doc
                .crypto_events
                .iter_mut()
                .find(|have| have.id == event.id)
            {
                Some(have) => {
                    if *have != event {
                        *have = event;
                    }
                    out.rewritten += 1;
                }
                None => {
                    self.doc.crypto_events.push(event);
                    out.added += 1;
                }
            }
        }
        self.doc
            .crypto_events
            .sort_by(|left, right| (&left.time, &left.id).cmp(&(&right.time, &right.id)));
        Ok(out)
    }

    /// Put the old verification queue through the same conversion. Idempotent
    /// and safe to call on every pass: a vault with no queue does nothing and
    /// writes nothing.
    ///
    /// A queue entry is either converted into an operation, or found to be
    /// settled already (booked into a transaction, or refused) and dropped
    /// from the queue without being counted a second time, or left exactly
    /// where it is when it cannot be read or its wallet is gone. No entry is
    /// discarded silently.
    pub fn adopt_legacy_bybit(&mut self) -> Result<LegacyAdoption> {
        let mut out = LegacyAdoption::default();
        if self.doc.pending_imports.is_empty() {
            return Ok(out);
        }
        // By wallet, so each wallet's records reach their own account; sorted,
        // so two devices do the same work in the same order.
        let mut by_wallet: BTreeMap<String, Vec<PendingImport>> = BTreeMap::new();
        let mut kept: Vec<PendingImport> = Vec::new();
        for record in std::mem::take(&mut self.doc.pending_imports) {
            let root = self.root_of(&record.store_id);
            let known_wallet = self
                .find_store(&root)
                .is_some_and(|store| store.kind == StoreKind::Exchange);
            if known_wallet {
                by_wallet.entry(root).or_default().push(record);
            } else {
                kept.push(record);
            }
        }
        for (root, records) in &by_wallet {
            let stored = self.bybit_store(root, records)?;
            out.converted += stored.recorded();
            out.known += stored.known;
            let unread: BTreeSet<&str> = stored.unread.iter().map(String::as_str).collect();
            kept.extend(
                records
                    .iter()
                    .filter(|record| unread.contains(record.exec_id.as_str()))
                    .cloned(),
            );
        }
        out.kept = kept.len() as u32;
        self.doc.pending_imports = kept;
        // The converted queue is capital now, not a pending list.
        self.crypto_sync_holdings()?;
        Ok(out)
    }

    /// An exchange coin as a token of the module, counted in the totals.
    ///
    /// An exchange ticker is on no contract list and would be quarantined out
    /// of every total (spec E2), so a coin the conversion met is marked
    /// confirmed: it was already in the user's ledger, and silently dropping
    /// it would lose money the user can see. A verdict the user has given
    /// already is never overwritten — rejecting a coin must stick.
    pub(crate) fn bybit_register_coin(&mut self, symbol: &str) {
        let symbol = symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() {
            return;
        }
        let token = TokenId::new(bybit::EXCHANGE_CHAIN, &symbol);
        let known = crate::exchange::well_known_coin(&symbol);
        if let Some(record) = self
            .doc
            .crypto_tokens
            .iter_mut()
            .find(|record| record.id() == token)
        {
            record.symbol = symbol;
            if record.verdict.is_none() {
                record.verdict = Some(TokenVerdict::Confirmed);
            }
            if record.coingecko_id.is_none() {
                record.coingecko_id = known.map(|(id, _)| id.to_owned());
            }
            return;
        }
        let mut record = CryptoToken::new(token, &symbol, bybit::COIN_DECIMALS);
        record.name = known.map(|(_, name)| name.to_owned()).unwrap_or(symbol);
        record.coingecko_id = known.map(|(id, _)| id.to_owned());
        record.verdict = Some(TokenVerdict::Confirmed);
        self.doc.crypto_tokens.push(record);
    }
}

/// Every coin named by a batch, each once, in a fixed order.
fn coins_of(records: &[&PendingImport]) -> BTreeSet<String> {
    records
        .iter()
        .flat_map(|record| {
            [
                record.asset_bought.clone(),
                record.asset_sold.clone(),
                record.fee_asset.clone(),
            ]
        })
        .map(|symbol| symbol.trim().to_ascii_uppercase())
        .filter(|symbol| !symbol.is_empty())
        .collect()
}

/// The dollar price of each asset for each day the records state one: the
/// quantity-weighted average of what the records themselves say.
///
/// Weighted, and not "the last fill of the day", because the engine can only
/// price a day (`PriceIndex::at` asks for midnight UTC): with a weighted
/// point the dollars a day's purchases add to the cost pool are exactly the
/// dollars the fills cost, which is what "the cost basis is preserved" means
/// under average cost.
fn day_prices(records: &[&PendingImport]) -> Vec<CryptoPricePoint> {
    // (asset key, day) -> (asset, Σ quantity × price, Σ quantity)
    let mut sums: BTreeMap<(String, String), (CryptoAsset, Amount, Amount)> = BTreeMap::new();
    for record in records {
        let day = crate::cryptotrack::engine::ids::day(&record.occurred_at);
        if day.len() != 10 {
            continue;
        }
        for (asset, unit) in bybit::implied_usd(record) {
            let Some(quantity) = quantity_of(record, &asset) else {
                continue;
            };
            let Some(value) = math::mul(quantity, unit) else {
                continue;
            };
            let slot = sums
                .entry((asset.key(), day.clone()))
                .or_insert_with(|| (asset.clone(), Amount::ZERO, Amount::ZERO));
            slot.1 = math::add(slot.1, value).unwrap_or(slot.1);
            slot.2 = math::add(slot.2, quantity).unwrap_or(slot.2);
        }
    }
    sums.into_iter()
        .filter_map(|((key, day), (_, value, quantity))| {
            let unit = math::div(value, quantity)?;
            let date = chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok()?;
            unit.is_positive().then(|| CryptoPricePoint {
                token_key: key,
                value_usd: unit,
                ts: prices::date_ts(date),
                source: bybit::SOURCE_ID.to_owned(),
            })
        })
        .collect()
}

/// How much of `asset` the record moved: the weight of its price in the day's
/// average.
fn quantity_of(record: &PendingImport, asset: &CryptoAsset) -> Option<Amount> {
    let ticker = bybit::asset_ticker(asset)?;
    let bought = record.asset_bought.trim().eq_ignore_ascii_case(ticker);
    let sold = record.asset_sold.trim().eq_ignore_ascii_case(ticker);
    let quantity = match (bought, sold) {
        (true, _) => record.qty_bought,
        (false, true) => record.qty_sold,
        (false, false) => return None,
    };
    quantity.is_positive().then_some(quantity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cryptotrack::sources::bybit::exchange_asset;
    use crate::exchange::{Execution, InterTransfer};
    use crate::model::store::ExchangeProvider;
    use crate::model::transaction::TxOrigin;
    use crate::service::StoreExtras;

    const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];
    /// The fills happen on 2023-11-14 UTC.
    const DAY_MS: u64 = 1_700_000_000_000;
    const AT: &str = "2023-11-15T00:00:00Z";

    /// A Bybit wallet as an older build left it: `(dir, vault, wallet)`.
    fn bybit_wallet() -> (tempfile::TempDir, UnlockedVault, String) {
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
                    provider: Some(ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault.ensure_bybit_accounts(&wallet, ACCOUNT_NAMES).unwrap();
        (dir, vault, wallet)
    }

    fn fill(exec_id: &str, side: &str, qty: &str, price: &str, value: &str, at: u64) -> Execution {
        Execution {
            exec_id: exec_id.into(),
            symbol: "BTCUSDT".into(),
            side: side.into(),
            order_type: "Market".into(),
            exec_price: price.into(),
            exec_qty: qty.into(),
            exec_value: value.into(),
            exec_fee: "0".into(),
            exec_time: at.to_string(),
            is_maker: false,
            closed_size: None,
            category: "spot".into(),
        }
    }

    /// A bought-one-bitcoin fill, as the old import staged it.
    fn bought(store: &str, exec_id: &str, price: &str, at: u64) -> PendingImport {
        PendingImport::from_execution(store, "spot", &fill(exec_id, "Buy", "1", price, price, at))
            .expect("a readable fill")
    }

    fn moved(store: &str, transfer_id: &str, at: u64) -> PendingImport {
        PendingImport::from_transfer(
            store,
            &InterTransfer {
                transfer_id: transfer_id.into(),
                coin: "USDT".into(),
                amount: "250".into(),
                from_account_type: "FUND".into(),
                to_account_type: "UNIFIED".into(),
                timestamp: at.to_string(),
                status: "SUCCESS".into(),
            },
        )
        .expect("a readable transfer")
    }

    /// Everything the conversion writes, as the sync would compare it between
    /// two devices.
    fn written(vault: &UnlockedVault) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "accounts": vault.doc.crypto_accounts,
            "events": vault.doc.crypto_events,
            "tokens": vault.doc.crypto_tokens,
            "prices": vault.doc.crypto_prices,
            "queue": vault.doc.pending_imports,
        }))
        .unwrap()
    }

    fn event_ids(vault: &UnlockedVault) -> Vec<String> {
        vault
            .doc
            .crypto_events
            .iter()
            .map(|event| event.id.clone())
            .collect()
    }

    #[test]
    fn converting_twice_writes_the_same_records() {
        let (_dir, mut vault, wallet) = bybit_wallet();
        vault
            .stage_imports(vec![
                bought(&wallet, "exec-1", "60000", DAY_MS),
                bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
                moved(&wallet, "t-1", DAY_MS + 120_000),
            ])
            .unwrap();

        let first = vault.adopt_legacy_bybit().unwrap();
        assert_eq!(
            first,
            LegacyAdoption {
                converted: 3,
                known: 0,
                kept: 0
            }
        );
        let after_first = written(&vault);

        // The second pass has nothing left to convert and changes nothing.
        let second = vault.adopt_legacy_bybit().unwrap();
        assert_eq!(second, LegacyAdoption::default());
        assert_eq!(written(&vault), after_first);

        // And storing the very same records again, as a later sweep would,
        // rewrites them instead of adding a second copy.
        let again = vault
            .bybit_store(
                &wallet,
                &[
                    bought(&wallet, "exec-1", "60000", DAY_MS),
                    bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
                    moved(&wallet, "t-1", DAY_MS + 120_000),
                ],
            )
            .unwrap();
        assert_eq!(again.recorded(), 0, "all three were settled already");
        assert_eq!(again.known, 3);
        assert_eq!(written(&vault), after_first);
    }

    #[test]
    fn two_devices_convert_the_same_file_to_the_same_records() {
        let (_dir, mut first, wallet) = bybit_wallet();
        first
            .stage_imports(vec![
                bought(&wallet, "exec-1", "60000", DAY_MS),
                bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
                moved(&wallet, "t-1", DAY_MS + 120_000),
            ])
            .unwrap();

        // The same file opened on another device: same document, own path.
        let other_dir = tempfile::tempdir().unwrap();
        let mut second = UnlockedVault::create(other_dir.path().join("copy.wallet"), "pw").unwrap();
        second.replace_doc(first.doc.clone()).unwrap();

        first.adopt_legacy_bybit().unwrap();
        second.adopt_legacy_bybit().unwrap();
        // Byte for byte: ids, times, prices and token records. Had any of
        // them come from a clock or a counter, the two would differ here and
        // the sync would see one operation as two.
        assert_eq!(written(&first), written(&second));
    }

    #[test]
    fn an_old_queue_loses_not_one_operation() {
        let (_dir, mut vault, wallet) = bybit_wallet();
        // The file an older build left: one record booked into the ledger,
        // one refused, three still waiting, and one the module cannot read.
        let mut broken = bought(&wallet, "exec-broken", "60000", DAY_MS);
        broken.occurred_at = String::new();
        vault
            .stage_imports(vec![
                bought(&wallet, "exec-booked", "50000", DAY_MS),
                bought(&wallet, "exec-refused", "55000", DAY_MS),
                bought(&wallet, "exec-1", "60000", DAY_MS),
                bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
                moved(&wallet, "t-1", DAY_MS + 120_000),
                broken,
            ])
            .unwrap();
        vault
            .confirm_pending_imports(&["exec-booked".to_owned()])
            .unwrap();
        vault
            .discard_pending_imports(&["exec-refused".to_owned()])
            .unwrap();
        let booked: Vec<String> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| format!("{:?}", tx.meta().origin))
            .collect();

        let out = vault.adopt_legacy_bybit().unwrap();

        // Operation by operation: the three readable ones became operations
        // of the module, under the id their facts give them.
        assert_eq!(
            event_ids(&vault),
            vec![
                format!("cev:bybit:{wallet}:bybit:exec-1"),
                format!("cev:bybit:{wallet}:bybit:exec-2"),
                format!("cev:bybit:{wallet}:bybit:transfer:t-1"),
            ]
        );
        assert_eq!(
            out,
            LegacyAdoption {
                converted: 3,
                known: 0,
                kept: 1
            }
        );
        // The one that cannot be read is still in the queue, not thrown away.
        assert_eq!(
            vault
                .doc
                .pending_imports
                .iter()
                .map(|record| record.exec_id.as_str())
                .collect::<Vec<_>>(),
            vec!["exec-broken"]
        );
        // The booked one is still the ledger transaction it was, untouched.
        assert_eq!(
            vault
                .doc
                .transactions
                .iter()
                .map(|tx| format!("{:?}", tx.meta().origin))
                .collect::<Vec<_>>(),
            booked
        );
        // The refused one stays refused.
        assert_eq!(vault.doc.discarded_imports, vec!["exec-refused"]);
        // Six records in, six accounted for, none twice.
        assert_eq!(out.converted + out.kept + 1 + 1, 6);
    }

    #[test]
    fn a_record_is_never_in_the_ledger_and_in_the_module_at_once() {
        let (_dir, mut vault, wallet) = bybit_wallet();
        vault
            .stage_imports(vec![
                bought(&wallet, "exec-booked", "50000", DAY_MS),
                bought(&wallet, "exec-1", "60000", DAY_MS),
            ])
            .unwrap();
        vault
            .confirm_pending_imports(&["exec-booked".to_owned()])
            .unwrap();
        vault.adopt_legacy_bybit().unwrap();

        let in_ledger: BTreeSet<String> = vault
            .doc
            .transactions
            .iter()
            .filter_map(|tx| match &tx.meta().origin {
                TxOrigin::Import { exec_id } => Some(exec_id.clone()),
                _ => None,
            })
            .collect();
        let in_module = vault.bybit_event_ids();
        assert_eq!(in_ledger, BTreeSet::from(["exec-booked".to_owned()]));
        assert_eq!(in_module, BTreeSet::from(["exec-1".to_owned()]));
        assert!(in_ledger.is_disjoint(&in_module));

        // In the totals: the module counts the one bitcoin it was given and
        // not the one the ledger already holds.
        let totals = vault.crypto_totals(AT);
        let bitcoin = totals
            .assets
            .iter()
            .find(|total| total.asset == exchange_asset("BTC"))
            .expect("the coin is counted, not quarantined");
        assert_eq!(bitcoin.quantity, Amount::parse("1").unwrap());
        assert_eq!(bitcoin.cost_usd, Amount::parse("60000").unwrap());
        // And the profit is not touched by the booked record either.
        assert_eq!(bitcoin.realized_usd, Amount::ZERO);
        assert_eq!(totals.realized_usd, Amount::ZERO);
    }

    #[test]
    fn the_cost_pool_costs_what_the_fills_cost() {
        let (_dir, mut vault, wallet) = bybit_wallet();
        // Two buys of one bitcoin on one day, at different prices. The engine
        // can only price a day, so the day's point is their weighted mean —
        // and the pool must still cost 60000 + 70000.
        vault
            .stage_imports(vec![
                bought(&wallet, "exec-1", "60000", DAY_MS),
                bought(&wallet, "exec-2", "70000", DAY_MS + 60_000),
            ])
            .unwrap();
        vault.adopt_legacy_bybit().unwrap();

        let totals = vault.crypto_totals(AT);
        let bitcoin = totals
            .assets
            .iter()
            .find(|total| total.asset == exchange_asset("BTC"))
            .expect("the coin is counted");
        assert_eq!(bitcoin.quantity, Amount::parse("2").unwrap());
        assert_eq!(bitcoin.cost_usd, Amount::parse("130000").unwrap());
        assert_eq!(bitcoin.average_usd, Some(Amount::parse("65000").unwrap()));
        // Nothing about the coin is in doubt: every leg was priced from the
        // records themselves. (The dollar stablecoin the buys were paid with
        // goes negative, because the history does not say where it came
        // from, and the engine says so about that asset — not about this
        // one.)
        assert!(!bitcoin.imprecise, "doubts: {:?}", bitcoin.doubts);
        assert!(bitcoin.doubts.is_empty());
    }

    #[test]
    fn a_wallet_the_engine_drives_is_not_archivable() {
        let (_dir, mut vault, wallet) = bybit_wallet();
        // Before anything is followed, nothing claims the wallet.
        assert!(!vault.exchange_is_engine_driven(&wallet));
        vault
            .stage_imports(vec![bought(&wallet, "exec-1", "60000", DAY_MS)])
            .unwrap();
        vault.adopt_legacy_bybit().unwrap();
        assert!(vault.exchange_is_engine_driven(&wallet));
        // A sub-account of it answers the same, because archiving reaches the
        // whole wallet.
        let unified = vault
            .doc
            .stores
            .iter()
            .find(|store| store.parent_id.as_deref() == Some(wallet.as_str()))
            .expect("the wallet has its exchange accounts")
            .id
            .clone();
        assert!(vault.exchange_is_engine_driven(&unified));
        // A wallet that is not an exchange never does.
        let plain = vault
            .doc
            .stores
            .iter()
            .find(|store| store.kind != StoreKind::Exchange && store.parent_id.is_none())
            .expect("the new vault has an ordinary wallet")
            .id
            .clone();
        assert!(!vault.exchange_is_engine_driven(&plain));
    }
}
