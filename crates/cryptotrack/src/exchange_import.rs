//! Rules of booking what an exchange sends: which wallet takes a fill, which
//! asset a symbol is, the transaction a fill becomes, the staged queue and the
//! exchange terminal's state. The scenarios (confirming, discarding, the key
//! targets, the account set-up) are in `usecases::cryptotrack::exchange_import`;
//! what an import brings in at all is in [`super::exchange_sync`].
//!
//! The matching of fills to wallets is pure: it reads a [`Books`] (the
//! document and the store tree behind it) and answers with an id, an asset or
//! a transaction; it writes nothing. The HTTP stays behind `ExchangeClient`.
//!
//! The queue (`pending_imports`) is what an older build staged for the user to
//! confirm (spec E7); it is still served, and a fill is staged only once per
//! `exec_id`: whatever was staged, booked or refused before is skipped.

use std::collections::HashSet;

use super::exchange_sync::{
    children_of, find_store, import_categories, import_route, known_import_ids, remember_discarded,
    root_of,
};
use mwt_base::Amount;
use mwt_base::error::{Error, Result, WalletRule};
use mwt_model::Chain;
use mwt_model::{
    AccountRef, Fee, FuturesAction, FuturesCategory, FuturesDirection, KindDetails, Liquidity,
    PriceStamp, TopupTx, TradeFuturesTx, TradeSide, TradeSpotTx, Transaction, TransferTx, TxMeta,
    TxOrigin,
};
use mwt_model::{AssetRef, Binding, ExchangeProfile, MarketKind, TerminalState};
use mwt_model::{ExchangeKeys, ImportKind, PendingImport, VaultDoc};
use mwt_money::books::Books;

/// Most drawings JSON a terminal keeps per symbol is bounded by the model;
/// the number of watched symbols is bounded here.
pub const MAX_WATCHLIST: usize = 200;

/// Shortest and longest background import interval, in seconds.
const MIN_INTERVAL_SEC: u32 = 60;
const MAX_INTERVAL_SEC: u32 = 86_400;

/// The Bybit key that serves a store: its own, else its wallet's. A
/// sub-account never borrows another wallet's key.
pub fn bybit_key_for<'books>(
    books: &Books<'books>,
    store_id: &str,
) -> Option<&'books ExchangeKeys> {
    let mut current = find_store(books, store_id)?;
    loop {
        if let Some(keys) = books
            .doc()
            .exchanges
            .bybit
            .iter()
            .find(|keys| keys.store_id == current.id)
        {
            return Some(keys);
        }
        current = find_store(books, current.parent_id.as_deref()?)?;
    }
}

/// The live sub-account of `parent_id` that stands for a market.
pub fn bound_child(books: &Books<'_>, parent_id: &str, market: MarketKind) -> Option<String> {
    children_of(books, parent_id)
        .into_iter()
        .find(|child| !child.archived && child.binding == Binding::Market(market))
        .map(|child| child.id.clone())
}

/// The live sub-account of an exchange wallet that stands for a Bybit
/// account type: `FUND` → Funding, `UNIFIED` (and the legacy `SPOT`,
/// `CONTRACT`) → Unified, `EARN`/`INVESTMENT` → Earn.
pub fn bybit_account_store(books: &Books<'_>, root_id: &str, account: &str) -> Option<String> {
    // The import settings may send an account to a wallet of the user's choice.
    if let Some(Some(target)) = import_route(books, root_id, account) {
        return Some(target);
    }
    let market = match account.trim().to_ascii_uppercase().as_str() {
        "FUND" => MarketKind::Funding,
        "UNIFIED" | "SPOT" | "CONTRACT" => MarketKind::Unified,
        "EARN" | "INVESTMENT" => MarketKind::Earn,
        _ => return None,
    };
    bound_child(books, root_id, market)
}

/// The live Unified Trading account of an exchange wallet: where spot and
/// futures trades are booked.
pub fn unified_account_of(books: &Books<'_>, store_id: &str) -> Option<String> {
    let root = root_of(books, store_id);
    bybit_account_store(books, &root, mwt_model::ACCOUNT_UNIFIED)
}

/// Where a fill of this category is booked: the sub-account picked for
/// spot or futures on the exchange key, else the wallet's live sub-account
/// bound to that market, else the key's own store.
pub(crate) fn import_target(books: &Books<'_>, imp: &PendingImport) -> String {
    if imp.kind == ImportKind::EarnIncome {
        return bybit_account_store(books, &imp.store_id, mwt_model::ACCOUNT_EARN)
            .unwrap_or_else(|| imp.store_id.clone());
    }
    let futures = imp.category != "spot";
    let market = if futures {
        MarketKind::Futures
    } else {
        MarketKind::Spot
    };
    books
        .doc()
        .exchanges
        .bybit
        .iter()
        .find(|keys| keys.store_id == imp.store_id)
        .and_then(|keys| {
            if futures {
                keys.futures_store_id.clone()
            } else {
                keys.spot_store_id.clone()
            }
        })
        // Bybit trades spot and derivatives in one unified account.
        .or_else(|| bybit_account_store(books, &imp.store_id, mwt_model::ACCOUNT_UNIFIED))
        .or_else(|| bound_child(books, &imp.store_id, market))
        .unwrap_or_else(|| imp.store_id.clone())
}

/// Build an [`AssetRef`] for an imported symbol.
///
/// The chain is inherited from an asset the user already tracks, so an
/// imported fill lands in the same bucket as manual entries. Unknown
/// symbols stay chain-less (`None`) rather than being guessed at — the
/// user can bind a chain later.
pub(crate) fn asset_for_import(doc: &VaultDoc, symbol: &str) -> AssetRef {
    let upper = symbol.trim().to_ascii_uppercase();
    match doc
        .holdings
        .iter()
        .find(|holding| holding.asset.symbol.eq_ignore_ascii_case(symbol))
        .map(|holding| holding.asset.clone())
    {
        // Reuse the identity the user already tracks (name, coingecko id,
        // chain) so the fill lands in the same bucket as manual entries.
        Some(mut asset) => {
            asset.symbol = upper;
            asset
        }
        // Unseen symbol: a well-known coin gets its CoinGecko id (so it is
        // priced like one picked by hand); the chain stays unresolved so
        // the user can bind it later, rather than guessing a network.
        None => {
            let known = mwt_model::well_known_coin(&upper);
            let name = known.map_or(upper.as_str(), |(_, name)| name);
            let mut asset = AssetRef::crypto(&upper, name, known.map(|(id, _)| id), Chain::Fiat);
            asset.chain = None;
            asset
        }
    }
}

/// The identity a store already holds a coin under, else the import's.
pub fn held_asset(doc: &VaultDoc, store_id: &str, coin: &str) -> AssetRef {
    doc.holdings
        .iter()
        .find(|holding| {
            holding.store_id == store_id
                && !holding.asset.is_derivative()
                && holding.asset.symbol.eq_ignore_ascii_case(coin)
        })
        .map(|holding| holding.asset.clone())
        .unwrap_or_else(|| asset_for_import(doc, coin))
}

/// The transaction a staged fill becomes: a spot trade, or a futures
/// open/close for linear and inverse contracts.
pub fn transaction_for_import(books: &Books<'_>, imp: &PendingImport) -> Transaction {
    let doc = books.doc();
    let mut meta = TxMeta::new(&imp.comment);
    meta.categories = import_categories(books, imp, "trading");
    meta.origin = TxOrigin::Import {
        exec_id: imp.exec_id.clone(),
    };
    if !imp.occurred_at.is_empty() {
        meta.timestamp = imp.occurred_at.clone();
    }
    match imp.kind {
        ImportKind::Fill => {}
        ImportKind::Transfer => {
            meta.categories = import_categories(books, imp, "transfer");
            let side = |account: &Option<String>| {
                account
                    .as_deref()
                    .and_then(|account| bybit_account_store(books, &imp.store_id, account))
                    .unwrap_or_else(|| imp.store_id.clone())
            };
            return Transaction::Transfer(TransferTx {
                meta,
                from: AccountRef::Store(side(&imp.from_account)),
                to: AccountRef::Store(side(&imp.to_account)),
                asset: asset_for_import(doc, &imp.asset_bought),
                amount: imp.qty_bought,
                fee: None,
                network: None,
                tx_hash: None,
                debt: None,
                price: PriceStamp::default(),
            });
        }
        ImportKind::EarnIncome => {
            meta.categories = import_categories(books, imp, "deposit_interest");
            return Transaction::Topup(TopupTx {
                meta,
                store_id: import_target(books, imp),
                asset: asset_for_import(doc, &imp.asset_bought),
                amount: imp.qty_bought,
                fee: None,
                from: None,
                details: KindDetails::default(),
                price: PriceStamp::default(),
            });
        }
    }
    let store_id = import_target(books, imp);
    let is_buy = imp.side.eq_ignore_ascii_case("buy");
    let (base_symbol, quote_symbol) = if is_buy {
        (&imp.asset_bought, &imp.asset_sold)
    } else {
        (&imp.asset_sold, &imp.asset_bought)
    };
    let inverse = imp.category == "inverse";
    let settle_symbol = if inverse { base_symbol } else { quote_symbol };
    // Spot fees come in the coin Bybit names; futures fees are always
    // charged in the settle asset.
    let fee_symbol = match imp.category.as_str() {
        "spot" if !imp.fee_asset.is_empty() => imp.fee_asset.as_str(),
        "spot" => quote_symbol.as_str(),
        _ => settle_symbol.as_str(),
    };
    let fee = (!imp.fee.is_zero()).then(|| Fee {
        asset: asset_for_import(doc, fee_symbol),
        amount: imp.fee.abs(),
        price: PriceStamp::default(),
    });
    // A stablecoin quote makes the fill price a USD price.
    let usd_quoted = ["USDT", "USDC", "USD", "FDUSD", "BUSD"]
        .contains(&quote_symbol.to_ascii_uppercase().as_str());
    let stamp = |usd: Option<Amount>| PriceStamp {
        price_usd: usd,
        ..PriceStamp::default()
    };
    if imp.category == "spot" {
        let base_stamp = stamp(usd_quoted.then_some(imp.price));
        let quote_stamp = stamp(usd_quoted.then(|| Amount::new(rust_decimal::Decimal::ONE)));
        let (sold_stamp, bought_stamp) = if is_buy {
            (quote_stamp, base_stamp)
        } else {
            (base_stamp, quote_stamp)
        };
        return Transaction::TradeSpot(TradeSpotTx {
            meta,
            store_id,
            sold: TradeSide {
                asset: asset_for_import(doc, &imp.asset_sold),
                qty: imp.qty_sold,
                price: sold_stamp,
            },
            bought: TradeSide {
                asset: asset_for_import(doc, &imp.asset_bought),
                qty: imp.qty_bought,
                price: bought_stamp,
            },
            fee,
            liquidity: imp.is_maker.map(|maker| {
                if maker {
                    Liquidity::Maker
                } else {
                    Liquidity::Taker
                }
            }),
            order_id: None,
        });
    }
    let closing = imp.closed_size.is_some_and(|size| size.is_positive());
    // Buying closes a short, selling closes a long.
    let direction = match (is_buy, closing) {
        (true, false) | (false, true) => FuturesDirection::Long,
        (false, false) | (true, true) => FuturesDirection::Short,
    };
    let settle = asset_for_import(doc, settle_symbol);
    Transaction::TradeFutures(TradeFuturesTx {
        meta,
        store_id,
        contract: AssetRef::perp("bybit", &imp.symbol),
        settle,
        direction,
        action: if closing {
            FuturesAction::Close
        } else {
            FuturesAction::Open
        },
        quantity: if is_buy { imp.qty_bought } else { imp.qty_sold },
        fill_price: imp.price,
        leverage: None,
        fee,
        closed_pnl: if closing { imp.closed_pnl } else { None },
        order_id: None,
        category: if inverse {
            FuturesCategory::Inverse
        } else {
            FuturesCategory::Linear
        },
        price: stamp((usd_quoted && !inverse).then(|| Amount::new(rust_decimal::Decimal::ONE))),
    })
}

/// Every staged fill, newest first.
pub fn pending_imports(doc: &VaultDoc) -> Vec<PendingImport> {
    let mut out = doc.pending_imports.clone();
    out.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
    out
}

/// The fills of `imports` worth staging: not seen before (staged, booked or
/// discarded, and not met twice in the batch), on a wallet that is still
/// there (`live` says so), and no more than the queue has room for. `held` is
/// how many are staged already, `cap` how many the queue may hold.
///
/// Fills are keyed by Bybit's `execId`, so re-importing the same history
/// stages nothing twice.
pub fn plan_staging(
    books: &Books<'_>,
    imports: Vec<PendingImport>,
    held: usize,
    cap: usize,
    live: impl Fn(&str) -> bool,
) -> Vec<PendingImport> {
    let mut known = known_import_ids(books.doc());
    let mut staged = Vec::new();
    for imp in imports {
        if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
            continue;
        }
        if !live(&imp.store_id) {
            continue;
        }
        known.insert(imp.exec_id.clone());
        staged.push(imp);
        if held + staged.len() >= cap {
            break;
        }
    }
    staged
}

/// What confirming the staged fills comes to: the transactions that pass the
/// rules, and the staged entries that leave the queue (booked, or their
/// wallet vanished). Entries are named by their place in the queue.
#[derive(Debug, Default)]
pub struct Confirmation {
    booked: Vec<(usize, Transaction)>,
    vanished: Vec<usize>,
}

impl Confirmation {
    /// How many fills become transactions.
    pub fn booked(&self) -> u32 {
        self.booked.len() as u32
    }
}

/// An empty `exec_ids` selects everything staged. `accepts` is the validation
/// of a transaction: a fill that fails it (its wallet was archived, say) stays
/// staged.
pub fn plan_confirmation(
    books: &Books<'_>,
    exec_ids: &[String],
    accepts: impl Fn(&Transaction) -> bool,
) -> Confirmation {
    let mut plan = Confirmation::default();
    for (index, imp) in selected(&books.doc().pending_imports, exec_ids) {
        if find_store(books, &imp.store_id).is_none() {
            // The wallet vanished: drop the staged fill, don't book it.
            plan.vanished.push(index);
            continue;
        }
        let tx = transaction_for_import(books, imp);
        if accepts(&tx) {
            plan.booked.push((index, tx));
        }
    }
    plan
}

/// Book the confirmed fills and take them, and the ones whose wallet is gone,
/// out of the queue. The import mark exists in vaults made before it.
pub fn apply_confirmation(doc: &mut VaultDoc, plan: Confirmation) {
    super::exchange_sync::ensure_import_category(doc);
    let leaving: HashSet<usize> = plan
        .booked
        .iter()
        .map(|(index, _)| *index)
        .chain(plan.vanished)
        .collect();
    doc.transactions
        .extend(plan.booked.into_iter().map(|(_, tx)| tx));
    let mut index = 0;
    doc.pending_imports.retain(|_| {
        index += 1;
        !leaving.contains(&(index - 1))
    });
}

/// How many staged fills `exec_ids` names (everything when it is empty).
pub fn pending_named(doc: &VaultDoc, exec_ids: &[String]) -> usize {
    selected(&doc.pending_imports, exec_ids).count()
}

/// Drop the staged fills `exec_ids` names (everything when it is empty)
/// without booking them, and remember their ids so the next sync does not
/// stage them again. Returns how many were dropped.
pub fn discard_pending(doc: &mut VaultDoc, exec_ids: &[String]) -> u32 {
    let all = exec_ids.is_empty();
    let before = doc.pending_imports.len();
    let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
    let (dropped, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut doc.pending_imports)
        .into_iter()
        .partition(|pending| all || wanted.contains(pending.exec_id.as_str()));
    doc.pending_imports = kept;
    remember_discarded(doc, dropped.into_iter().map(|pending| pending.exec_id));
    (before - doc.pending_imports.len()) as u32
}

/// The staged fills `exec_ids` names, with their place in the queue.
fn selected<'queue>(
    pending: &'queue [PendingImport],
    exec_ids: &'queue [String],
) -> impl Iterator<Item = (usize, &'queue PendingImport)> {
    pending
        .iter()
        .enumerate()
        .filter(move |(_, imp)| exec_ids.is_empty() || exec_ids.contains(&imp.exec_id))
}

/// The profile of an exchange wallet.
pub fn exchange_profile<'doc>(
    doc: &'doc VaultDoc,
    store_id: &str,
) -> Result<&'doc ExchangeProfile> {
    doc.stores
        .iter()
        .find(|store| store.id == store_id)
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
        .exchange
        .as_ref()
        .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
}

/// [`exchange_profile`], to change it.
pub fn exchange_profile_mut<'doc>(
    doc: &'doc mut VaultDoc,
    store_id: &str,
) -> Result<&'doc mut ExchangeProfile> {
    doc.stores
        .iter_mut()
        .find(|store| store.id == store_id)
        .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
        .exchange
        .as_mut()
        .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
}

/// The terminal state of an exchange wallet.
pub fn terminal_state(doc: &VaultDoc, store_id: &str) -> Result<TerminalState> {
    exchange_profile(doc, store_id).map(|profile| profile.terminal.clone())
}

/// Background import interval of one exchange: `0` disables it, otherwise it
/// is kept between a minute and a day.
pub fn clamp_interval(interval_sec: u32) -> u32 {
    if interval_sec == 0 {
        0
    } else {
        interval_sec.clamp(MIN_INTERVAL_SEC, MAX_INTERVAL_SEC)
    }
}

/// A watchlist as the terminal keeps it: symbols trimmed and upper-cased,
/// unique, at most [`MAX_WATCHLIST`].
pub fn normalize_watchlist(watchlist: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    watchlist
        .into_iter()
        .map(|symbol| symbol.trim().to_uppercase())
        .filter(|symbol| !symbol.is_empty() && seen.insert(symbol.clone()))
        .take(MAX_WATCHLIST)
        .collect()
}

/// The symbol a terminal setting names, as it is kept: trimmed and
/// upper-cased; an empty one names nothing.
pub fn terminal_symbol(symbol: &str) -> Option<String> {
    Some(symbol.trim().to_uppercase()).filter(|symbol| !symbol.is_empty())
}

/// Replace one symbol's drawings; an empty export removes the entry. Reports
/// whether the terminal changed.
pub fn set_drawings(terminal: &mut TerminalState, symbol: &str, json: &str) -> bool {
    let empty = json.trim().is_empty() || json.trim() == "[]";
    if empty {
        terminal.drawings.remove(symbol).is_some()
    } else {
        terminal
            .drawings
            .insert(symbol.to_string(), json.to_string())
            .as_deref()
            != Some(json)
    }
}
