//! Rules of syncing an exchange wallet: what an import brings in (its
//! settings), which records are new, the history of syncs and the imported
//! operations. The scenarios (the sync itself, the settings, the live-balance
//! correction) are in `usecases::cryptotrack::exchange_sync`; where a fill is
//! booked is in [`super::exchange_import`].
//!
//! The ledger stays the source of truth: balances come from operations. The
//! live balance only enters as a correction ("Adjustment"), so history is
//! kept and a later import still lands before it.
//!
//! Since spec E7 an exchange is read by the crypto module: a record becomes a
//! [`crate::model::cryptotrack::CryptoTx`] at once instead of waiting in
//! `pending_imports` for the user to confirm it; the one-time conversion of
//! what an older build staged lives in `storage::legacy_v4`. Everything the old
//! import stored stays readable and keeps deduplicating: the queue, the refused
//! ids and the `exec_id` of a booked transaction all answer
//! [`decided_import_ids`], so a record that was already settled one way is never
//! counted the other way as well.
//!
//! The store tree belongs to `money`, which the crypto module may not name
//! (arch layer table). The three reads of it that the rules need
//! ([`root_of`], [`find_store`], [`children_of`]) are the only places that
//! reach it, through `Books::vault()`.

use std::collections::HashSet;

use super::service::legacy_bybit::bybit_event_ids;
use crate::error::{Error, Result};
use crate::kernel::books::Books;
use crate::model::category::{Category, CategoryGroup, EXCHANGE_IMPORT_CATEGORY};
use crate::model::store::{ImportSettings, Store};
use crate::model::transaction::{Transaction, TxOrigin};
use crate::model::vault::{
    ImportKind, ImportRun, ImportSource, MAX_DISCARDED_IMPORTS, MAX_IMPORT_RUNS, PendingImport,
    VaultDoc,
};

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

/// What one batch of records comes to once the settings and what was seen
/// before are applied.
#[derive(Debug, Default)]
pub struct Intake {
    /// New and wanted, in the order the exchange sent them.
    pub fresh: Vec<PendingImport>,
    /// Seen before.
    pub known: u32,
    /// Left out by the settings.
    pub filtered: u32,
}

/// Top-level wallet of `id` (the tree is `money`'s, see the module docs).
pub(crate) fn root_of(books: &Books<'_>, id: &str) -> String {
    crate::money::tree::root_of(books.doc(), id)
}

/// The store with this id.
pub(crate) fn find_store<'books>(books: &Books<'books>, id: &str) -> Option<&'books Store> {
    crate::money::tree::find_store(books.doc(), id)
}

/// Direct children of a store.
pub(crate) fn children_of<'books>(books: &Books<'books>, parent_id: &str) -> Vec<&'books Store> {
    crate::money::tree::children_of(books.doc(), parent_id)
}

/// The account type an account name stands for, as the settings store it.
pub(crate) fn account_key(account: &str) -> String {
    match account.trim().to_ascii_uppercase().as_str() {
        "SPOT" | "CONTRACT" | "UNIFIED" => crate::exchange::bybit::ACCOUNT_UNIFIED.to_string(),
        "INVESTMENT" | "EARN" => crate::exchange::bybit::ACCOUNT_EARN.to_string(),
        other => other.to_string(),
    }
}

/// The import settings of the exchange wallet `store_id` belongs to.
pub fn import_settings(books: &Books<'_>, store_id: &str) -> Result<ImportSettings> {
    let root = root_of(books, store_id);
    let store =
        find_store(books, &root).ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
    Ok(store
        .exchange
        .as_ref()
        .map(|profile| profile.import.clone())
        .unwrap_or_default())
}

/// `settings` as the vault keeps them: coins upper-cased and unique, the
/// date checked, the categories known and every account route valid.
/// `check_target` decides whether a route's wallet may take the fills.
pub fn normalize_settings(
    doc: &VaultDoc,
    mut settings: ImportSettings,
    check_target: impl Fn(&str) -> Result<()>,
) -> Result<ImportSettings> {
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
        && crate::time::unix_secs(since).is_none()
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
        if !doc.categories.iter().any(|category| category.key == *key) {
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
            check_target(target)?;
        }
    }
    Ok(settings)
}

/// The wallet an exchange account is routed to by the settings:
/// `None` = not set (the bound sub-account), `Some(None)` = not imported.
pub(crate) fn import_route(
    books: &Books<'_>,
    root_id: &str,
    account: &str,
) -> Option<Option<String>> {
    let profile = find_store(books, root_id)?.exchange.as_ref()?;
    route_in(books, &profile.import, account)
}

/// [`import_route`] by the given settings (saved or not yet).
pub(crate) fn route_in(
    books: &Books<'_>,
    settings: &ImportSettings,
    account: &str,
) -> Option<Option<String>> {
    let key = account_key(account);
    settings
        .accounts
        .iter()
        .find(|route| route.account == key)
        .map(|route| {
            route
                .store_id
                .clone()
                .filter(|id| find_store(books, id).is_some_and(|store| !store.archived))
        })
}

/// Whether the settings send this exchange account nowhere.
pub(crate) fn account_skipped(
    books: &Books<'_>,
    settings: &ImportSettings,
    account: Option<&str>,
) -> bool {
    account.is_some_and(|account| matches!(route_in(books, settings, account), Some(None)))
}

/// Whether the settings let a record in.
pub(crate) fn import_wanted(
    books: &Books<'_>,
    imp: &PendingImport,
    settings: &ImportSettings,
) -> bool {
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
            .any(|account| account_skipped(books, settings, account))
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
    match (&settings.since, crate::time::unix_secs(&imp.occurred_at)) {
        (Some(since), Some(at)) => crate::time::unix_secs(since).is_none_or(|since| at >= since),
        _ => true,
    }
}

/// The categories an imported operation gets: the one the settings name
/// for its kind (or `default`), and the import mark.
pub(crate) fn import_categories(
    books: &Books<'_>,
    imp: &PendingImport,
    default: &str,
) -> Vec<String> {
    let settings = import_settings(books, &imp.store_id).unwrap_or_default();
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
pub(crate) fn ensure_import_category(doc: &mut VaultDoc) {
    if !doc
        .categories
        .iter()
        .any(|category| category.key == EXCHANGE_IMPORT_CATEGORY)
    {
        doc.categories.push(Category::builtin(
            EXCHANGE_IMPORT_CATEGORY,
            CategoryGroup::Service,
        ));
    }
}

/// Ids an import has already seen: staged, booked or discarded.
pub(crate) fn known_import_ids(doc: &VaultDoc) -> HashSet<String> {
    let mut known: HashSet<String> = doc
        .pending_imports
        .iter()
        .map(|pending| pending.exec_id.clone())
        .collect();
    known.extend(decided_import_ids(doc));
    known
}

/// Ids an import has already settled: an operation of the crypto module,
/// a ledger transaction the old import booked, or a record the user
/// refused. The staged queue is not part of it, so a queue entry can be
/// checked against it.
///
/// All three belong here because all three keep one record from being
/// counted twice (spec E7): an `exec_id` the module holds as an operation
/// must never be fetched into the ledger as well, and an `exec_id` the
/// ledger already books must never become an operation of the module.
pub fn decided_import_ids(doc: &VaultDoc) -> HashSet<String> {
    let mut decided: HashSet<String> = doc.discarded_imports.iter().cloned().collect();
    decided.extend(bybit_event_ids(doc));
    decided.extend(
        doc.transactions
            .iter()
            .filter_map(|tx| match &tx.meta().origin {
                TxOrigin::Import { exec_id } => Some(exec_id.clone()),
                _ => None,
            }),
    );
    decided
}

/// Leave out of `imports` what the settings do not want and what was seen
/// before, counting both. A record met twice in one batch counts once.
pub(crate) fn intake(
    books: &Books<'_>,
    imports: Vec<PendingImport>,
    settings: &ImportSettings,
    mut known: HashSet<String>,
) -> Intake {
    let mut taken = Intake::default();
    for imp in imports {
        if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
            taken.known += 1;
            continue;
        }
        if !import_wanted(books, &imp, settings) {
            taken.filtered += 1;
            continue;
        }
        known.insert(imp.exec_id.clone());
        taken.fresh.push(imp);
    }
    taken
}

/// What a sync would bring under `settings` (the saved ones, or the ones
/// still being edited): nothing is staged or kept.
pub fn preview_exchange_sync(
    books: &Books<'_>,
    imports: Vec<PendingImport>,
    settings: &ImportSettings,
) -> ImportPreview {
    let fetched = imports.len() as u32;
    let taken = intake(books, imports, settings, known_import_ids(books.doc()));
    let new = taken.fresh.len() as u32;
    let mut items = taken.fresh;
    items.truncate(MAX_PREVIEW_ITEMS);
    ImportPreview {
        fetched,
        known: taken.known,
        filtered: taken.filtered,
        new,
        items,
    }
}

/// A run of `source` over the wallet `root` at `now`, with nothing counted yet.
pub(crate) fn new_run(root: &str, source: ImportSource, now: i64) -> ImportRun {
    ImportRun {
        id: uuid::Uuid::new_v4().to_string(),
        store_id: root.to_string(),
        at: crate::calendar::iso(now),
        source,
        ..ImportRun::default()
    }
}

/// Count what a run is about to bring in, by kind.
pub(crate) fn count_kinds(run: &mut ImportRun, fresh: &[PendingImport]) {
    let count = |kind: ImportKind| fresh.iter().filter(|imp| imp.kind == kind).count() as u32;
    run.trades = count(ImportKind::Fill);
    run.transfers = count(ImportKind::Transfer);
}

/// An empty run replaces the wallet's previous one if that was empty too,
/// so the history shows «checked at …» without a row per poll. Not saved
/// by itself: it reaches the file with the next real change.
pub(crate) fn note_empty_run(doc: &mut VaultDoc, run: ImportRun) {
    let last = doc
        .import_runs
        .iter_mut()
        .rev()
        .find(|kept| kept.store_id == run.store_id);
    match last {
        Some(kept) if kept.staged == 0 && kept.confirmed == 0 && kept.error.is_none() => {
            *kept = run
        }
        _ => keep_import_run(doc, run),
    }
}

/// Add a run to the history, which keeps the newest [`MAX_IMPORT_RUNS`].
pub(crate) fn keep_import_run(doc: &mut VaultDoc, run: ImportRun) {
    doc.import_runs.push(run);
    let overflow = doc.import_runs.len().saturating_sub(MAX_IMPORT_RUNS);
    doc.import_runs.drain(..overflow);
}

/// Discarded ids are remembered, so the next sync does not stage them again.
pub(crate) fn remember_discarded(doc: &mut VaultDoc, ids: impl IntoIterator<Item = String>) {
    doc.discarded_imports.extend(ids);
    let overflow = doc
        .discarded_imports
        .len()
        .saturating_sub(MAX_DISCARDED_IMPORTS);
    doc.discarded_imports.drain(..overflow);
}

/// Where the newest run of one wallet sits in the history.
pub(crate) fn last_run_index(doc: &VaultDoc, root: &str) -> Option<usize> {
    doc.import_runs.iter().rposition(|run| run.store_id == root)
}

/// Name the correction `tx_id` on the run at `index` (see [`last_run_index`]).
pub(crate) fn mark_reconciled(doc: &mut VaultDoc, index: usize, tx_id: String) {
    doc.import_runs[index].reconcile_tx_id = Some(tx_id);
}

/// Syncs of one exchange wallet, newest first.
pub fn import_runs(books: &Books<'_>, store_id: &str) -> Vec<ImportRun> {
    let root = root_of(books, store_id);
    books
        .doc()
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
    books: &Books<'_>,
    store_id: &str,
    limit: usize,
    offset: usize,
) -> ImportedTransactions {
    let root = root_of(books, store_id);
    let mut found: Vec<&Transaction> = books
        .doc()
        .transactions
        .iter()
        .filter(|tx| matches!(tx.meta().origin, TxOrigin::Import { .. }))
        .filter(|tx| {
            tx.legs()
                .iter()
                .any(|leg| root_of(books, &leg.store_id) == root)
                || matches!(tx, Transaction::Adjustment(inner) if root_of(books, &inner.store_id) == root)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::UnlockedVault;

    fn fill(exec_id: &str) -> PendingImport {
        PendingImport {
            exec_id: exec_id.into(),
            kind: ImportKind::Fill,
            ..PendingImport::default()
        }
    }

    #[test]
    fn a_preview_counts_a_repeated_exec_id_once_like_the_import() {
        let dir = tempfile::tempdir().unwrap();
        let vault = UnlockedVault::create(dir.path().join("test.wallet"), "pw").unwrap();
        let books = vault.books();
        let batch = vec![fill("twice"), fill("twice"), fill(""), fill("other")];
        let settings = ImportSettings::default();
        let preview = preview_exchange_sync(&books, batch.clone(), &settings);
        let taken = intake(&books, batch, &settings, known_import_ids(books.doc()));
        assert_eq!(
            (
                preview.fetched,
                preview.known,
                preview.filtered,
                preview.new
            ),
            (4, 2, 0, 2)
        );
        assert_eq!(preview.items, taken.fresh);
    }
}
