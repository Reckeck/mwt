//! What the crypto module adds to the capital (spec E8).
//!
//! # Why there is no second total
//!
//! Every capital figure of the app is read off `doc.holdings`: net worth
//! (`service::worth::net_worth_with_crypto`), the capital history
//! (`service::reports::networth_points`), the baskets
//! (`service::allocation`), "Investments" (`service::investments`). The
//! module creates no transactions, so E8's amendment of 2026-11-10 offered
//! two ways in: synthetic holdings, or a `crypto_totals(at)` addend in each
//! of those six calculations.
//!
//! The user decided on 2026-11-11 (decision P5 in `U2-decisions.md`) that
//! crypto lives in the wallets and has no section of its own: a connected
//! address *is* a `Store` of kind `Crypto` carrying
//! `Store::crypto_account_id`. So the holdings path is not a shortcut here,
//! it is the model: the engine's balances are folded into `doc.holdings` next
//! to `recompute_holdings`, and every existing calculation keeps working
//! untouched. No addend was added anywhere.
//!
//! # What the amendment warned about, and what is done about it
//!
//! **Double counting.** A user who kept a crypto wallet by hand and then
//! connected the same address has the same coins written down twice: once as
//! the transactions of the hand-kept wallet, once as the engine's balance.
//! Archiving the old wallet is exactly how the move to level 3 retires it
//! (spec E11), and an archived wallet still counts in net worth today
//! (`worth.rs` never looks at `Store::archived`). So the rule here is: an
//! **archived** wallet of kind `Crypto` or `Exchange` that the engine does
//! *not* drive yields **all** of its holdings to the module — the coins are
//! counted once, at the addresses that really hold them. A wallet that is not
//! archived keeps its holdings: it is another address the user tracks by hand,
//! and only they can say it is the same one. The same replacement happens
//! inside an engine-driven wallet, so a leftover hand-made position there
//! cannot sit next to the engine's row for one asset.
//!
//! E8 made that rule narrower — a retired wallet yielded only the assets the
//! engine accounts for — and spec E11 widened it to every asset the wallet
//! holds, because a retired record is retired whole: a coin the user wrote
//! down by hand and no connected address holds is a coin the module cannot
//! vouch for, and leaving it in the capital would quietly keep a number the
//! user has stopped maintaining. This is what the wizard warns about before
//! the move, and what `UnlockedVault::crypto_transition_plan` counts.
//! Fiat is untouched by all of this: only `Crypto` and `Exchange` wallets
//! yield, so an archived bank or cash wallet keeps counting exactly as before.
//!
//! The widened rule costs nothing in a vault without the module: the rule
//! runs only where the module is — a vault at level 3, or one that follows an
//! address (`doc.crypto_accounts`) — so a vault at level 1 or 2, which can be
//! neither, values its archived wallets exactly as it always did. Level 3 is
//! in that test on purpose: the move retires the hand-kept wallets at the
//! moment of the raise, before the first address is connected, and the
//! capital has to drop then — that is what the wizard warns about, and what
//! `crypto_transition_plan` counted for it.
//!
//! The past half of the rule ([`CryptoHistory::supersedes`]) was deliberately
//! left narrow. A past point is replayed from the transactions, and the
//! module's record of that month speaks only for what it accounted for then;
//! dropping an asset it never knew would understate a month the user really
//! did track by hand, which is a different question from what their capital
//! is today.
//!
//! **The module's own prices and marks.** A holding is valued by
//! `price_quote_for`, which knows the price cache, a hand-set price, the last
//! transaction's stamp and the dollar peg — not the module's
//! `crypto_prices` history (spec E4), which exists for the cost basis and the
//! snapshots. Two rules keep that from inventing a number:
//!
//! * a token the module refuses to trust never becomes a holding at all. The
//!   fold already drops a quarantined or rejected contract (spec E2), so it
//!   cannot be valued by the symbol it stole: a scam token calling itself
//!   `USDC` with a cached dollar price stays out of the capital
//!   (`a_quarantined_token_stays_out_even_when_something_quoted_it`);
//! * an asset nothing anywhere can price keeps its quantity (the coins are
//!   real) and is reported by [`crypto_outside_totals`] with
//!   that quantity, so the total is visibly short rather than silently zero.
//!
//! The engine's own marks (`AssetState::doubts`: `MissingPrice`, `TooLarge`,
//! `Reconciled`, `OpeningAtMarket`, `LeftTheModule`, `MoreLeftThanArrived`)
//! cannot travel inside a `Holding` — it has no field for them, and adding
//! one is a change to a stored type. They travel the two ways that need no
//! new field: a cost the engine cannot vouch for comes out as
//! `Holding::avg_cost: None`, which "Investments" already shows as
//! `cost_known: false`, and the full per-asset list with every doubt stays
//! `recompute::crypto_totals` (spec E5), which the module's screens read.
//!
//! # The months already gone: snapshots, not a replay
//!
//! `doc.holdings` is today's balance, so one file does need the module's own
//! history: `service::reports::networth_points` takes its current point from
//! `net_worth_with_crypto` (the holdings, so the module is in it) but replays
//! the transactions for every earlier month (`positions_at`), and the module
//! writes none. What it was worth then is in `doc.crypto_snapshots` — spec E5
//! writes one record per account and one for the module on every read, keyed
//! by the day — so [`crypto_history`] reads those records and
//! `networth_points` adds that value instead of replaying anything.
//!
//! **A month with no record is a gap in tracking, not a zero.** Three rules
//! keep such a month honest:
//!
//! * an account contributes the latest record **at or before** the cutoff.
//!   Carrying a known value forward is what the whole report already does
//!   with prices (`RateBook::history_price` takes the last quote at or before
//!   the moment), and it is a value the module really recorded, so a month
//!   between two readings keeps the capital it had instead of collapsing to
//!   zero and reading as a loss;
//! * nothing is ever carried **backwards**. Before an account's first record
//!   the module did not watch it, so it adds nothing — the series simply does
//!   not contain the module yet. That is the start of tracking, not a drop:
//!   every earlier point is equally without it, so there is no step down;
//! * an archived account is left out of the history exactly as it is left out
//!   of the present, so archiving does not bend the shape of the chart.
//!
//! The double-counting rule holds for a past point too, and it has to: a
//! replayed month contains the transactions of the hand-kept wallet the user
//! has since retired. [`CryptoHistory::supersedes`] drops those positions
//! from a point the module has a record for, by the same archived-wallet rule
//! and the same `AssetRef::same_as` match as the present. Where the module
//! has no record — the months before it watched anything — the hand-kept
//! wallet counts in full: that history is the only one there is.
//!
//! Two things are deliberately not done here. A point does not say *how* much
//! of it is a gap: `NetworthPoint` has no field for that, and adding one is a
//! contract change for the main session, not for this spec. And an account
//! whose wallet is out of the report's scope (or has no wallet at all) is not
//! in the history either, which keeps a scoped report and the present
//! agreeing on what the module contributes.
//!
//! # What is not counted, and says so
//!
//! [`crypto_outside_totals`] is the list the screen shows next
//! to the number: a quarantined token, an asset with no price, and an account
//! no wallet shows yet (nobody's capital until E9/E11 creates the wallet). An
//! account the user archived (`CryptoAccount::archived`) is not in the list:
//! "watched but not counted" is what archiving it means.

use std::collections::{BTreeSet, HashSet};

use crate::Amount;
use crate::cryptotrack::engine::position_value::held_by_account;
use crate::cryptotrack::engine::{Ledger, math};
use crate::cryptotrack::service::recompute::fold;
use crate::cryptotrack::{chains, tokens};
use crate::kernel::books::Books;
use crate::model::chain::Chain;
use crate::model::cryptotrack::CryptoAsset;
use crate::model::store::{AssetRef, Holding, StoreKind};
use crate::model::vault::VaultDoc;
use crate::pricing::history::unix_secs;

/// Why something the module knows about stays out of the capital.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoLeftOut {
    /// An unconfirmed contract: in no total until the user decides (spec E2).
    Quarantined,
    /// No wallet shows this account yet, so its coins are nobody's capital.
    NoWallet,
    /// Nothing anywhere knows what one unit is worth.
    NoPrice,
    /// The receipt a protocol handed over for a position (an aToken, stETH, a
    /// position NFT). The money is already counted as the position, so
    /// counting the receipt too would double it (spec D0,
    /// `positions::UnlockedVault::crypto_position_receipts`).
    InPosition,
}

/// One thing the capital does not count, what is held and why.
#[derive(Debug, Clone, PartialEq)]
pub struct CryptoOutsideTotals {
    /// The followed account; `None` for a token left out module-wide.
    pub account_id: Option<String>,
    /// The wallet it would have been shown in, when there is one.
    pub store_id: Option<String>,
    /// The module's own key ([`CryptoAsset::key`]).
    pub asset_key: String,
    pub symbol: String,
    /// What is held; `None` for a quarantined token, which the fold never
    /// counts, so there is no honest quantity to name.
    pub quantity: Option<Amount>,
    pub reason: CryptoLeftOut,
}

/// The module's own capital history: what it was worth at the cutoffs of a
/// report, and which replayed wallets that record speaks for.
#[derive(Debug, Default)]
pub(crate) struct CryptoHistory {
    /// USD at each cutoff, in the order they were given. `None` is a month the
    /// module has no record of — a gap in tracking, never a zero.
    values_usd: Vec<Option<Amount>>,
    /// Archived hand-kept wallets whose coins a followed address now holds.
    retired: HashSet<String>,
    /// What the engine accounts for today, to recognize a replayed position of
    /// one of those wallets.
    engine_assets: Vec<AssetRef>,
}

impl CryptoHistory {
    /// The module's value at the `index`-th cutoff, in USD; `None` for a month
    /// it has no record of.
    pub(crate) fn value_usd(&self, index: usize) -> Option<Amount> {
        self.values_usd.get(index).copied().flatten()
    }

    /// Whether a replayed position is the same coins the module's record
    /// already counts: the past half of the rule in `apply_crypto_holdings`.
    /// The caller asks only for a point that has a record.
    pub(crate) fn supersedes(&self, store_id: &str, asset: &AssetRef) -> bool {
        self.retired.contains(store_id)
            && self.engine_assets.iter().any(|known| known.same_as(asset))
    }
}

/// The engine's balances as holdings, and what had to be left out.
#[derive(Debug, Default)]
pub(crate) struct Capital {
    pub(crate) holdings: Vec<Holding>,
    pub(crate) left_out: Vec<CryptoOutsideTotals>,
}

/// What the module knows and the capital does not count, with the reason.
pub fn crypto_outside_totals(books: &Books<'_>) -> Vec<CryptoOutsideTotals> {
    capital(books).left_out
}

/// Replace what the engine's rows supersede in `doc.holdings` with `made`:
/// a row for the same wallet and asset, and everything a retired wallet
/// held, whatever the asset. A retired wallet yields everything it held, not
/// only the assets the engine recognizes (spec E11): the record is retired
/// whole, and a coin no followed address holds is a number the user has
/// stopped maintaining. This is the drop the wizard warns about, counted
/// beforehand by `crypto_transition_plan`.
pub fn supersede_holdings(doc: &mut VaultDoc, made: Vec<Holding>, retired: &HashSet<String>) {
    doc.holdings.retain(|have| {
        let engine_row = made
            .iter()
            .any(|row| row.store_id == have.store_id && row.asset.same_as(&have.asset));
        !engine_row && !retired.contains(&have.store_id)
    });
    doc.holdings.extend(made);
}

/// Whether two folds of the holdings differ: every fold gives the rows fresh
/// ids, which alone is no change worth a write.
pub fn holdings_differ(before: &[Holding], after: &[Holding]) -> bool {
    let same = before.len() == after.len()
        && after.iter().all(|row| {
            before.iter().any(|old| {
                old.store_id == row.store_id
                    && old.asset == row.asset
                    && old.quantity == row.quantity
                    && old.avg_cost == row.avg_cost
            })
        });
    !same
}

/// What the module was worth at each cutoff of a report, for the months a
/// replay of the transactions cannot reach (see the header). `in_scope` is
/// the report's own wallet filter.
pub(crate) fn crypto_history(
    books: &Books<'_>,
    cutoffs: &[i64],
    in_scope: impl Fn(&str) -> bool,
) -> CryptoHistory {
    let doc = books.doc();
    let mut out = CryptoHistory {
        values_usd: vec![None; cutoffs.len()],
        retired: HashSet::new(),
        engine_assets: Vec::new(),
    };
    if doc.crypto_accounts.is_empty() {
        return out;
    }
    for account in &doc.crypto_accounts {
        // Watched but not counted, in the history as in the present.
        if account.archived {
            continue;
        }
        let shown = doc.stores.iter().any(|store| {
            store.crypto_account_id.as_deref() == Some(account.id.as_str()) && in_scope(&store.id)
        });
        if !shown {
            continue;
        }
        let mut points: Vec<(i64, Amount)> = doc
            .crypto_snapshots
            .iter()
            .filter(|record| record.account_id.as_deref() == Some(account.id.as_str()))
            .filter_map(|record| unix_secs(&record.at).map(|secs| (secs, record.value_usd)))
            .collect();
        points.sort_by_key(|(secs, _)| *secs);
        for (index, cutoff) in cutoffs.iter().enumerate() {
            // The latest reading at or before the cutoff: a value the
            // module really recorded, never one invented for the gap.
            let Some((_, value)) = points.iter().rev().find(|(secs, _)| *secs <= *cutoff) else {
                continue;
            };
            let slot = out.values_usd[index].get_or_insert(Amount::ZERO);
            *slot = math::add(*slot, *value).unwrap_or(*slot);
        }
    }
    if out.values_usd.iter().any(Option::is_some) {
        out.retired = crypto_retired_wallets(doc);
        // Only a retired wallet can be the module's coins twice, so the
        // fold is paid for only when there is one.
        if !out.retired.is_empty() {
            out.engine_assets = fold(books)
                .assets
                .values()
                .map(|state| crypto_asset_ref(doc, &state.asset))
                .collect();
        }
    }
    out
}

/// The hand-kept wallets the user retired when the module took over: an
/// archived crypto wallet no followed address drives. They do not hold the
/// coins any more, the address does (see the header).
pub fn crypto_retired_wallets(doc: &VaultDoc) -> HashSet<String> {
    doc.stores
        .iter()
        .filter(|store| {
            store.archived
                && store.crypto_account_id.is_none()
                && matches!(store.kind, StoreKind::Crypto | StoreKind::Exchange)
        })
        .map(|store| store.id.clone())
        .collect()
}

/// One pass over the fold: the holdings the engine's accounts contribute
/// and everything that stays outside the capital.
///
/// What each account holds comes from one pure function
/// (`engine::position_value::held_by_account`): wallet, ledger position,
/// the adapter legs no operation accounts for, minus the debt legs. A
/// debt over the collateral gives a negative quantity, kept as is. Which
/// accounts count and what their positions are come from the positions area,
/// through the vault of the books (spec core-strangler OD6).
pub(crate) fn capital(books: &Books<'_>) -> Capital {
    let doc = books.doc();
    let mut out = Capital::default();
    // A vault that never followed an address pays nothing for this and
    // keeps `doc.holdings` exactly as the transactions left it.
    if doc.crypto_accounts.is_empty() {
        return out;
    }
    let ledger = fold(books);
    let (_, fx) = books.base_and_fx();
    let vault = books.vault();
    let account_counts = |account_id: &str| vault.crypto_account_counts(account_id);
    let inputs = vault.crypto_position_inputs();
    add_receipts_left_out(doc, &ledger, &inputs.receipts, &account_counts, &mut out);
    for held in inputs.with(|taking| held_by_account(&ledger, taking)) {
        let owned = held.net();
        if owned.is_zero() || !account_counts(&held.account_id) {
            continue;
        }
        let asset_key = held.asset.key();
        let asset = crypto_asset_ref(doc, &held.asset);
        let wallet = doc
            .stores
            .iter()
            .find(|store| store.crypto_account_id.as_deref() == Some(held.account_id.as_str()));
        let Some(wallet) = wallet else {
            out.left_out.push(CryptoOutsideTotals {
                account_id: Some(held.account_id),
                store_id: None,
                asset_key,
                symbol: asset.symbol,
                quantity: Some(owned),
                reason: CryptoLeftOut::NoWallet,
            });
            continue;
        };
        // The default origin is `PriceOrigin::None`: no price at all; the type itself
        // lives above this layer, so it is not named here.
        if vault.price_quote_for(&asset.key()).info.origin == Default::default() {
            out.left_out.push(CryptoOutsideTotals {
                account_id: Some(held.account_id.clone()),
                store_id: Some(wallet.id.clone()),
                asset_key: asset_key.clone(),
                symbol: asset.symbol.clone(),
                quantity: Some(owned),
                reason: CryptoLeftOut::NoPrice,
            });
        }
        // The engine's average is USD per unit and covers only the part
        // whose cost is known; `None` means the cost is not known, and
        // nothing downstream turns that into a zero. A debt has no cost,
        // and neither has a quantity that only an adapter reported: the
        // ledger's average would be invented for it.
        let avg_cost = ledger
            .state(&asset_key)
            .filter(|_| owned.is_positive() && held.unfolded.is_zero())
            .and_then(|state| state.average_usd())
            .and_then(|usd| math::mul(usd, fx));
        out.holdings.push(Holding {
            id: uuid::Uuid::new_v4().to_string(),
            store_id: wallet.id.clone(),
            asset,
            quantity: owned,
            avg_cost,
        });
    }
    out.left_out.extend(crypto_quarantined(doc));
    out
}

/// A protocol's receipt is the position, not a second holding: the money
/// it stands for is already counted through the position's own value
/// (spec D0). A node reports the receipt's balance anyway and the
/// reconciliation writes it down, so the capital drops it and the screen
/// is told why.
fn add_receipts_left_out(
    doc: &VaultDoc,
    ledger: &Ledger,
    receipts: &BTreeSet<String>,
    account_counts: &dyn Fn(&str) -> bool,
    out: &mut Capital,
) {
    for ((account_id, asset_key), quantity) in &ledger.balances {
        if !receipts.contains(asset_key) || !quantity.is_positive() || !account_counts(account_id) {
            continue;
        }
        let Some(state) = ledger.state(asset_key) else {
            continue;
        };
        out.left_out.push(CryptoOutsideTotals {
            account_id: Some(account_id.clone()),
            store_id: None,
            asset_key: asset_key.clone(),
            symbol: crypto_asset_ref(doc, &state.asset).symbol,
            quantity: Some(*quantity),
            reason: CryptoLeftOut::InPosition,
        });
    }
}

/// Tokens the module met and refuses to fold (spec E2), named so a total
/// can say what it left out.
pub fn crypto_quarantined(doc: &VaultDoc) -> Vec<CryptoOutsideTotals> {
    let met: BTreeSet<String> = doc
        .crypto_events
        .iter()
        .flat_map(|event| {
            event
                .legs
                .iter()
                .map(|leg| leg.asset.key())
                .chain(event.gas.iter().map(|gas| gas.asset.key()))
        })
        .collect();
    doc.crypto_tokens
        .iter()
        .filter(|record| {
            !tokens::counts_in_totals(doc, &record.id()) && met.contains(&record.id().key())
        })
        .map(|record| CryptoOutsideTotals {
            account_id: None,
            store_id: None,
            asset_key: record.id().key(),
            symbol: record.symbol.clone(),
            quantity: None,
            reason: CryptoLeftOut::Quarantined,
        })
        .collect()
}

/// The identity a module holding is shown and priced under: the same
/// [`AssetRef`] a hand-kept wallet would carry, so one coin is one row
/// and one price everywhere, and the ordinary price refresh (it reads
/// `doc.holdings`) quotes it like any other coin.
pub fn crypto_asset_ref(doc: &VaultDoc, asset: &CryptoAsset) -> AssetRef {
    match asset {
        CryptoAsset::Native { chain } => native_ref(*chain),
        CryptoAsset::Token(token) => {
            let record = doc
                .crypto_tokens
                .iter()
                .find(|record| record.id() == *token);
            let symbol = record.map_or("?", |record| record.symbol.as_str());
            let name = record
                .map(|record| record.name.as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or(symbol);
            AssetRef {
                symbol: symbol.to_uppercase(),
                name: name.to_string(),
                coingecko_id: record.and_then(|record| record.coingecko_id.clone()),
                chain: Some(token.chain),
                // Normalized on the way in (`normalize_address`), so one
                // contract is one key however the source spelled it.
                contract: Some(token.contract.clone()),
                quote: symbol.to_uppercase(),
            }
        }
    }
}

/// The network's gas coin as an asset: the EVM table first (it is the one
/// checked against a published source), then the well-known tickers, and the
/// network's own name when neither knows it — a name is cosmetic, the
/// CoinGecko id is what prices it.
fn native_ref(chain: Chain) -> AssetRef {
    let coin = chain.info().cg_coin_id;
    if let Some(row) = chains::evm_chain(chain) {
        return AssetRef::crypto(row.native.symbol, row.native.name, coin, chain);
    }
    if let Some((ticker, _, name)) =
        coin.and_then(|id| crate::exchange::well_known_coins().find(|(_, known, _)| *known == id))
    {
        return AssetRef::crypto(ticker, name, coin, chain);
    }
    AssetRef::crypto(chain.label(), chain.label(), coin, chain)
}
