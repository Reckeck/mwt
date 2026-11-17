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
//! the move, and what [`UnlockedVault::crypto_transition_plan`] counts.
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
//!   real) and is reported by [`UnlockedVault::crypto_outside_totals`] with
//!   that quantity, so the total is visibly short rather than silently zero.
//!
//! The engine's own marks (`AssetState::doubts`: `MissingPrice`, `TooLarge`,
//! `Reconciled`, `OpeningAtMarket`, `LeftTheModule`, `MoreLeftThanArrived`)
//! cannot travel inside a `Holding` — it has no field for them, and adding
//! one is a change to a stored type. They travel the two ways that need no
//! new field: a cost the engine cannot vouch for comes out as
//! `Holding::avg_cost: None`, which "Investments" already shows as
//! `cost_known: false`, and the full per-asset list with every doubt stays
//! `UnlockedVault::crypto_totals` (spec E5), which the module's screens read.
//!
//! # The months already gone: snapshots, not a replay
//!
//! `doc.holdings` is today's balance, so one file does need the module's own
//! history: `service::reports::networth_points` takes its current point from
//! `net_worth_with_crypto` (the holdings, so the module is in it) but replays
//! the transactions for every earlier month (`positions_at`), and the module
//! writes none. What it was worth then is in `doc.crypto_snapshots` — spec E5
//! writes one record per account and one for the module on every read, keyed
//! by the day — so [`UnlockedVault::crypto_history`] reads those records and
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
//! [`UnlockedVault::crypto_outside_totals`] is the list the screen shows next
//! to the number: a quarantined token, an asset with no price, and an account
//! no wallet shows yet (nobody's capital until E9/E11 creates the wallet). An
//! account the user archived (`CryptoAccount::archived`) is not in the list:
//! "watched but not counted" is what archiving it means.

use std::collections::{BTreeSet, HashSet};

use crate::Amount;
use crate::cryptotrack::engine::position_value::held_by_account;
use crate::cryptotrack::engine::{Ledger, math};
use crate::cryptotrack::{chains, tokens};
use crate::error::Result;
use crate::ipc_contract::security::SecurityLevel;
use crate::model::chain::Chain;
use crate::model::cryptotrack::CryptoAsset;
use crate::model::store::{AssetRef, Holding, StoreKind};
use crate::pricing::history::unix_secs;
use crate::service::price_info::PriceOrigin;

use super::super::UnlockedVault;

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
pub(in crate::service) struct CryptoHistory {
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
    pub(in crate::service) fn value_usd(&self, index: usize) -> Option<Amount> {
        self.values_usd.get(index).copied().flatten()
    }

    /// Whether a replayed position is the same coins the module's record
    /// already counts: the past half of the rule in `apply_crypto_holdings`.
    /// The caller asks only for a point that has a record.
    pub(in crate::service) fn supersedes(&self, store_id: &str, asset: &AssetRef) -> bool {
        self.retired.contains(store_id)
            && self.engine_assets.iter().any(|known| known.same_as(asset))
    }
}

/// The engine's balances as holdings, and what had to be left out.
#[derive(Debug, Default)]
struct Capital {
    holdings: Vec<Holding>,
    left_out: Vec<CryptoOutsideTotals>,
}

impl UnlockedVault {
    /// Put the engine's balances into `doc.holdings`, replacing what they
    /// supersede. Called at the end of `recompute_holdings`; saves nothing.
    pub(in crate::service) fn apply_crypto_holdings(&mut self) {
        // A vault without the module keeps `doc.holdings` exactly as the
        // transactions left it: nothing here speaks for it, so a wallet
        // archived long ago counts as it always did. The module is there once
        // the vault is at level 3 — hand-kept crypto has no place from that
        // point on, and the move retires it before the first address is
        // connected — or once an address is followed, which is the only state
        // a vault below level 3 can be in (a level-2 vault cannot hold one).
        if self.doc.crypto_accounts.is_empty() && self.security_level() != SecurityLevel::Crypto {
            return;
        }
        let made = self.crypto_capital().holdings;
        let retired = self.crypto_retired_wallets();
        if made.is_empty() && retired.is_empty() {
            return;
        }
        self.doc.holdings.retain(|have| {
            let engine_row = made
                .iter()
                .any(|row| row.store_id == have.store_id && row.asset.same_as(&have.asset));
            // A retired wallet yields everything it held, not only the assets
            // the engine recognizes (spec E11): the record is retired whole,
            // and a coin no followed address holds is a number the user has
            // stopped maintaining. This is the drop the wizard warns about,
            // counted beforehand by `crypto_transition_plan`.
            let superseded = retired.contains(&have.store_id);
            !engine_row && !superseded
        });
        self.doc.holdings.extend(made);
    }

    /// What the module knows and the capital does not count, with the reason.
    pub fn crypto_outside_totals(&self) -> Vec<CryptoOutsideTotals> {
        self.crypto_capital().left_out
    }

    /// Fold the engine again into `doc.holdings` and save: what the module's
    /// own writers call after they changed an event, a verdict or an account,
    /// since nothing else re-derives the holdings for them.
    pub fn crypto_sync_holdings(&mut self) -> Result<()> {
        self.recompute_holdings();
        self.persist()
    }

    /// What the module was worth at each cutoff of a report, for the months a
    /// replay of the transactions cannot reach (see the header). `in_scope` is
    /// the report's own wallet filter.
    pub(in crate::service) fn crypto_history(
        &self,
        cutoffs: &[i64],
        in_scope: impl Fn(&str) -> bool,
    ) -> CryptoHistory {
        let mut out = CryptoHistory {
            values_usd: vec![None; cutoffs.len()],
            retired: HashSet::new(),
            engine_assets: Vec::new(),
        };
        if self.doc.crypto_accounts.is_empty() {
            return out;
        }
        for account in &self.doc.crypto_accounts {
            // Watched but not counted, in the history as in the present.
            if account.archived {
                continue;
            }
            let shown = self.doc.stores.iter().any(|store| {
                store.crypto_account_id.as_deref() == Some(account.id.as_str())
                    && in_scope(&store.id)
            });
            if !shown {
                continue;
            }
            let mut points: Vec<(i64, Amount)> = self
                .doc
                .crypto_snapshots
                .iter()
                .filter(|record| record.account_id.as_deref() == Some(account.id.as_str()))
                .filter_map(|record| unix_secs(&record.at).map(|secs| (secs, record.value_usd)))
                .collect();
            points.sort_by_key(|(secs, _)| *secs);
            for (index, cutoff) in cutoffs.iter().enumerate() {
                // The latest reading at or before the cutoff: a value the
                // module really recorded, never one invented for the gap.
                let Some((_, value)) = points.iter().rev().find(|(secs, _)| *secs <= *cutoff)
                else {
                    continue;
                };
                let slot = out.values_usd[index].get_or_insert(Amount::ZERO);
                *slot = math::add(*slot, *value).unwrap_or(*slot);
            }
        }
        if out.values_usd.iter().any(Option::is_some) {
            out.retired = self.crypto_retired_wallets();
            // Only a retired wallet can be the module's coins twice, so the
            // fold is paid for only when there is one.
            if !out.retired.is_empty() {
                out.engine_assets = self
                    .crypto_fold()
                    .assets
                    .values()
                    .map(|state| self.crypto_asset_ref(&state.asset))
                    .collect();
            }
        }
        out
    }

    /// The hand-kept wallets the user retired when the module took over: an
    /// archived crypto wallet no followed address drives. They do not hold the
    /// coins any more, the address does (see the header).
    fn crypto_retired_wallets(&self) -> HashSet<String> {
        self.doc
            .stores
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
    /// debt over the collateral gives a negative quantity, kept as is.
    fn crypto_capital(&self) -> Capital {
        let mut out = Capital::default();
        // A vault that never followed an address pays nothing for this and
        // keeps `doc.holdings` exactly as the transactions left it.
        if self.doc.crypto_accounts.is_empty() {
            return out;
        }
        let ledger = self.crypto_fold();
        let (_, fx) = self.base_and_fx();
        let inputs = self.crypto_position_inputs();
        self.add_receipts_left_out(&ledger, &inputs.receipts, &mut out);
        for held in inputs.with(|taking| held_by_account(&ledger, taking)) {
            let owned = held.net();
            if owned.is_zero() || !self.crypto_account_counts(&held.account_id) {
                continue;
            }
            let asset_key = held.asset.key();
            let asset = self.crypto_asset_ref(&held.asset);
            let wallet =
                self.doc.stores.iter().find(|store| {
                    store.crypto_account_id.as_deref() == Some(held.account_id.as_str())
                });
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
            if self.price_quote_for(&asset.key()).info.origin == PriceOrigin::None {
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
        out.left_out.extend(self.crypto_quarantined());
        out
    }

    /// A protocol's receipt is the position, not a second holding: the money
    /// it stands for is already counted through the position's own value
    /// (spec D0). A node reports the receipt's balance anyway and the
    /// reconciliation writes it down, so the capital drops it and the screen
    /// is told why.
    fn add_receipts_left_out(
        &self,
        ledger: &Ledger,
        receipts: &BTreeSet<String>,
        out: &mut Capital,
    ) {
        for ((account_id, asset_key), quantity) in &ledger.balances {
            if !receipts.contains(asset_key)
                || !quantity.is_positive()
                || !self.crypto_account_counts(account_id)
            {
                continue;
            }
            let Some(state) = ledger.state(asset_key) else {
                continue;
            };
            out.left_out.push(CryptoOutsideTotals {
                account_id: Some(account_id.clone()),
                store_id: None,
                asset_key: asset_key.clone(),
                symbol: self.crypto_asset_ref(&state.asset).symbol,
                quantity: Some(*quantity),
                reason: CryptoLeftOut::InPosition,
            });
        }
    }

    /// Tokens the module met and refuses to fold (spec E2), named so a total
    /// can say what it left out.
    fn crypto_quarantined(&self) -> Vec<CryptoOutsideTotals> {
        let met: BTreeSet<String> = self
            .doc
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
        self.doc
            .crypto_tokens
            .iter()
            .filter(|record| {
                !tokens::counts_in_totals(&self.doc, &record.id())
                    && met.contains(&record.id().key())
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
    pub(in crate::service) fn crypto_asset_ref(&self, asset: &CryptoAsset) -> AssetRef {
        match asset {
            CryptoAsset::Native { chain } => native_ref(*chain),
            CryptoAsset::Token(token) => {
                let record = self
                    .doc
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::cryptotrack::CryptoSnapshot;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoDirection, CryptoLeg, CryptoPosition, CryptoPricePoint, CryptoToken,
        CryptoTx, CryptoTxKind, TokenId, TokenVerdict, TrackingRef,
    };
    use crate::model::store::StoreScope;
    use crate::model::transaction::Transaction;
    use crate::model::vault::PricePoint;
    use crate::service::test_support::*;
    use crate::service::{NetworthPoint, Report, ReportFilter, ReportKind};

    const ME: &str = "0xabcd00000000000000000000000000000000ef01";
    const MATE: &str = "0x1111111111111111111111111111111111111111";
    const OUTSIDE: &str = "0x2222222222222222222222222222222222222222";
    const UNKNOWN: &str = "0x3333333333333333333333333333333333333333";

    fn eth() -> CryptoAsset {
        CryptoAsset::Native {
            chain: Chain::Ethereum,
        }
    }

    /// The same coin as a hand-kept wallet would hold.
    fn eth_ref() -> AssetRef {
        AssetRef::crypto("ETH", "Ether", Some("ethereum"), Chain::Ethereum)
    }

    /// A coin the module never met: no event mentions it, so only the
    /// hand-kept record ever spoke for it.
    fn doge_ref() -> AssetRef {
        AssetRef::crypto("DOGE", "Dogecoin", Some("dogecoin"), Chain::Ethereum)
    }

    /// A followed address with the wallet that shows it (decision P5).
    fn followed(vault: &mut UnlockedVault, account_id: &str, name: &str, address: &str) -> String {
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            account_id,
            name,
            Chain::Ethereum,
            address,
            "2026-02-01T00:00:00Z",
        ));
        let wallet = store(vault, name, StoreKind::Crypto, None);
        vault
            .doc
            .stores
            .iter_mut()
            .find(|have| have.id == wallet)
            .expect("the wallet was created")
            .crypto_account_id = Some(account_id.to_string());
        wallet
    }

    fn archive(vault: &mut UnlockedVault, store_id: &str) {
        vault
            .doc
            .stores
            .iter_mut()
            .find(|have| have.id == store_id)
            .expect("the wallet exists")
            .archived = true;
    }

    /// One operation with one leg, as a source would have left it.
    fn moved(
        event_id: &str,
        account_id: &str,
        day: u32,
        asset: CryptoAsset,
        quantity: &str,
        direction: CryptoDirection,
        other: &str,
    ) -> CryptoTx {
        let inbound = direction == CryptoDirection::In;
        CryptoTx {
            id: event_id.to_string(),
            chain: Chain::Ethereum,
            account_id: account_id.to_string(),
            time: format!("2026-03-{day:02}T10:00:00Z"),
            block: Some(21_000_000),
            from: if inbound { other.into() } else { ME.into() },
            to: if inbound { ME.into() } else { other.into() },
            legs: vec![CryptoLeg {
                direction,
                asset,
                amount: amount(quantity),
                counterparty: Some(other.to_string()),
            }],
            gas: None,
            protocol: None,
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{event_id}"),
            },
        }
    }

    /// What the module knows a unit cost on every day of the month (spec E4).
    fn priced(vault: &mut UnlockedVault, asset: &CryptoAsset, usd: &str) {
        for day in 1..=31 {
            vault.doc.crypto_prices.push(CryptoPricePoint {
                token_key: asset.key(),
                value_usd: amount(usd),
                ts: format!("2026-03-{day:02}T00:00:00Z"),
                source: "test".into(),
            });
        }
    }

    /// The ordinary price cache: what every holding is valued from.
    fn quote(vault: &mut UnlockedVault, asset: &AssetRef, usd: &str) {
        vault.doc.price_cache.push(PricePoint {
            asset_key: asset.key(),
            value_usd: amount(usd),
            ts: "2026-03-09T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
    }

    /// The whole point of the holdings path: a vault that never followed an
    /// address must not notice the module at all.
    #[test]
    fn a_vault_without_the_module_keeps_its_capital_byte_for_byte() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        vault
            .add_transaction(income(&bank, "1000", "salary"))
            .expect("the top-up is booked");
        quote_usd(&mut vault);
        let holdings = serde_json::to_vec(&vault.doc.holdings).expect("holdings serialize");
        let worth = serde_json::to_vec(&vault.net_worth()).expect("net worth serializes");

        vault.apply_crypto_holdings();

        assert_eq!(
            serde_json::to_vec(&vault.doc.holdings).expect("holdings serialize"),
            holdings,
            "the module must not touch a holding it did not make"
        );
        assert_eq!(
            serde_json::to_vec(&vault.net_worth()).expect("net worth serializes"),
            worth
        );
        assert!(vault.crypto_outside_totals().is_empty());
    }

    #[test]
    fn the_module_adds_its_wallet_and_moves_no_other_total() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        vault
            .add_transaction(income(&bank, "1000", "salary"))
            .expect("the top-up is booked");
        quote_usd(&mut vault);
        quote(&mut vault, &eth_ref(), "2000");
        let before = vault.net_worth();

        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        priced(&mut vault, &eth(), "2000");
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        let after = vault.net_worth();
        assert_eq!(after.financial, before.financial + amount("4000"));
        assert_eq!(after.non_financial, before.non_financial);
        assert_eq!(after.receivables, before.receivables);
        assert_eq!(after.liabilities, before.liabilities);
        assert_eq!(after.payables, before.payables);
        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
        let held = vault
            .doc
            .holdings
            .iter()
            .find(|have| have.store_id == wallet)
            .expect("the address holds coins");
        assert_eq!(
            held.avg_cost,
            Some(amount("2000")),
            "the engine's average cost reaches the wallet"
        );
        assert!(vault.crypto_outside_totals().is_empty());
    }

    #[test]
    fn a_transfer_between_the_users_own_addresses_leaves_the_capital_alone() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        let cold = followed(&mut vault, "cacc-1", "Cold", ME);
        let hot = followed(&mut vault, "cacc-2", "Hot", MATE);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "3",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();
        let before = vault.net_worth();
        assert_eq!(before.financial, amount("6000"));

        // The same coins move to the user's other address: two sides, one move.
        vault.doc.crypto_events.push(moved(
            "ev-2",
            "cacc-1",
            6,
            eth(),
            "1",
            CryptoDirection::Out,
            MATE,
        ));
        vault.doc.crypto_events.push(moved(
            "ev-3",
            "cacc-2",
            6,
            eth(),
            "1",
            CryptoDirection::In,
            ME,
        ));
        vault.recompute_holdings();

        let after = vault.net_worth();
        assert_eq!(
            after.financial, before.financial,
            "moving one's own coins is not income"
        );
        assert_eq!(holding_of(&vault, &cold, &eth_ref()), amount("2"));
        assert_eq!(holding_of(&vault, &hot, &eth_ref()), amount("1"));
    }

    /// The risk the E8 amendment named: the coins of a hand-kept wallet and
    /// the coins of the address it stood for are the same coins.
    #[test]
    fn an_archived_hand_kept_wallet_is_not_counted_twice() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        // How it was kept before the address was connected.
        let old = store(&mut vault, "Old cold", StoreKind::Crypto, None);
        vault
            .add_transaction(income_of(&old, eth_ref(), "2", "2000", "bought"))
            .expect("the purchase is booked");
        assert_eq!(vault.net_worth().financial, amount("4000"));

        // The move to level 3 archives it and connects the address (spec E11).
        archive(&mut vault, &old);
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        assert_eq!(
            vault.net_worth().financial,
            amount("4000"),
            "the same 2 ETH, counted once"
        );
        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
        assert_eq!(
            holding_of(&vault, &old, &eth_ref()),
            Amount::ZERO,
            "the retired record yields to the address that holds them"
        );
    }

    /// Spec E11 widened the rule E8 wrote narrowly: a retired record is
    /// retired whole. The old wallet also held a coin the module never met,
    /// and leaving it in the capital would keep a number the user has stopped
    /// maintaining — so it goes with the wallet, which is exactly the drop the
    /// wizard warns about.
    #[test]
    fn a_retired_wallet_yields_the_coins_the_engine_never_met() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        quote(&mut vault, &doge_ref(), "1");
        priced(&mut vault, &eth(), "2000");
        let old = store(&mut vault, "Old cold", StoreKind::Crypto, None);
        vault
            .add_transaction(income_of(&old, eth_ref(), "2", "2000", "bought"))
            .expect("the purchase is booked");
        vault
            .add_transaction(income_of(&old, doge_ref(), "1000", "1", "bought"))
            .expect("the second purchase is booked");
        assert_eq!(vault.net_worth().financial, amount("5000"));

        // The move to level 3: the record is archived, the address connected.
        archive(&mut vault, &old);
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        assert_eq!(
            vault.net_worth().financial,
            amount("4000"),
            "only what a followed address really holds is left"
        );
        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
        assert_eq!(
            holding_of(&vault, &old, &doge_ref()),
            Amount::ZERO,
            "the coin the module never met leaves with the retired record"
        );
    }

    /// The widened rule is the module's, not the app's: without a followed
    /// address nothing here runs, so a vault at level 1 or 2 — which cannot
    /// hold one — values an archived crypto wallet exactly as it always did.
    #[test]
    fn without_the_module_an_archived_crypto_wallet_still_counts() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        let old = store(&mut vault, "Old cold", StoreKind::Crypto, None);
        vault
            .add_transaction(income_of(&old, eth_ref(), "2", "2000", "bought"))
            .expect("the purchase is booked");
        archive(&mut vault, &old);
        vault.recompute_holdings();

        assert!(vault.doc.crypto_accounts.is_empty());
        assert_eq!(
            vault.net_worth().financial,
            amount("4000"),
            "archiving alone is not the module's rule"
        );
        assert_eq!(holding_of(&vault, &old, &eth_ref()), amount("2"));
    }

    /// A wallet the user still uses is another address, not the same coins.
    #[test]
    fn a_wallet_still_in_use_keeps_its_own_coins() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        let other = store(&mut vault, "By hand", StoreKind::Crypto, None);
        vault
            .add_transaction(income_of(&other, eth_ref(), "2", "2000", "bought"))
            .expect("the purchase is booked");
        followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        assert_eq!(vault.net_worth().financial, amount("8000"));
        assert_eq!(holding_of(&vault, &other, &eth_ref()), amount("2"));
    }

    #[test]
    fn a_quarantined_token_stays_out_even_when_something_quoted_it() {
        let (_dir, mut vault) = empty_vault();
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        let token = TokenId::new(Chain::Ethereum, UNKNOWN);
        // It calls itself USDC, and a dollar price for that key is cached.
        vault
            .doc
            .crypto_tokens
            .push(CryptoToken::new(token.clone(), "USDC", 6));
        let pretender = AssetRef {
            symbol: "USDC".into(),
            name: "USDC".into(),
            coingecko_id: None,
            chain: Some(Chain::Ethereum),
            contract: Some(UNKNOWN.into()),
            quote: "USDC".into(),
        };
        quote(&mut vault, &pretender, "1");
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            CryptoAsset::Token(token.clone()),
            "100",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        assert_eq!(vault.net_worth().financial, Amount::ZERO);
        assert_eq!(holding_of(&vault, &wallet, &pretender), Amount::ZERO);
        let left = vault.crypto_outside_totals();
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].reason, CryptoLeftOut::Quarantined);
        assert_eq!(left[0].asset_key, token.key());
        assert_eq!(left[0].symbol, "USDC");
    }

    /// A position an adapter read, holding `quantity` of `asset` as a leg
    /// no operation accounts for.
    fn position_of(account_id: &str, asset: CryptoAsset, quantity: &str) -> CryptoPosition {
        CryptoPosition {
            id: format!("pos-{account_id}"),
            account_id: account_id.to_string(),
            chain: Chain::Ethereum,
            protocol: "uniswap-v3".into(),
            name: "Pool".into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::In,
                asset,
                amount: amount(quantity),
                counterparty: None,
            }],
            receipts: Vec::new(),
            updated_at: "2026-03-05T10:00:00Z".into(),
        }
    }

    #[test]
    fn a_priced_leg_of_an_unfolded_position_is_not_left_out_for_want_of_a_price() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault
            .doc
            .crypto_positions
            .push(position_of("cacc-1", eth(), "2"));
        vault.recompute_holdings();

        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
        let left = vault.crypto_outside_totals();
        assert!(left.is_empty(), "a priced leg is not unpriced: {left:?}");
        assert_eq!(vault.net_worth().financial, amount("4000"));
    }

    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

    /// One Aave position: 1 ETH of collateral and 1 000 USDC of debt.
    fn aave_with_debt() -> CryptoPosition {
        let mut record = position_of("cacc-1", eth(), "1");
        record.protocol = "aave-v3".into();
        record.legs.push(CryptoLeg {
            direction: CryptoDirection::Out,
            asset: CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC)),
            amount: amount("1000"),
            counterparty: None,
        });
        record
    }

    #[test]
    fn an_aave_debt_lowers_the_capital() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "3000");
        let usdc = vault.crypto_asset_ref(&CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC)));
        quote(&mut vault, &usdc, "1");
        followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_positions.push(aave_with_debt());
        vault.recompute_holdings();

        assert_eq!(
            vault.net_worth().financial,
            amount("2000"),
            "3 000 of collateral less 1 000 of debt (was 3 000)"
        );
    }

    #[test]
    fn an_unpriced_debt_is_listed_as_left_out() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "3000");
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_positions.push(aave_with_debt());
        vault.recompute_holdings();

        let usdc_key = CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC)).key();
        let left = vault.crypto_outside_totals();
        let row = left
            .iter()
            .find(|row| row.asset_key == usdc_key)
            .expect("the debt is named, not dropped");
        assert_eq!(row.reason, CryptoLeftOut::NoPrice);
        assert_eq!(row.quantity, Some(amount("-1000")));
        assert_eq!(row.store_id.as_deref(), Some(wallet.as_str()));
    }

    #[test]
    fn a_stale_old_key_twin_of_a_position_is_not_counted_twice() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        let mut current = position_of("cacc-1", eth(), "2");
        current.id = "cacc-1|uniswap-v3|ethereum|7".into();
        let mut twin = current.clone();
        twin.id = "uniswap-v3|ethereum|7".into();
        vault.doc.crypto_positions.push(current);
        vault.doc.crypto_positions.push(twin);
        vault.recompute_holdings();

        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
    }

    #[test]
    fn an_unregistered_stranger_token_is_still_dropped() {
        let (_dir, mut vault) = empty_vault();
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        let token = TokenId::new(Chain::Ethereum, UNKNOWN);
        // A stranger's NFT position holds a token that calls itself USDC, and
        // the dollar peg would price anything by that symbol at 1 USD.
        vault
            .doc
            .crypto_tokens
            .push(CryptoToken::new(token.clone(), "USDC", 6));
        let pretender = AssetRef {
            symbol: "USDC".into(),
            name: "USDC".into(),
            coingecko_id: None,
            chain: Some(Chain::Ethereum),
            contract: Some(UNKNOWN.into()),
            quote: "USDC".into(),
        };
        vault.doc.crypto_positions.push(position_of(
            "cacc-1",
            CryptoAsset::Token(token.clone()),
            "1000000000",
        ));
        vault.recompute_holdings();

        assert_eq!(vault.net_worth().financial, Amount::ZERO);
        assert_eq!(holding_of(&vault, &wallet, &pretender), Amount::ZERO);
        let shown = vault.crypto_list_positions(&[], "2026-03-06T00:00:00Z");
        assert_eq!(shown.len(), 1);
        assert_eq!(shown[0].value_usd, None, "unknown, not zero and not 1e9");
        assert_eq!(shown[0].assets[0].value_usd, None);

        // Once the user takes the token in, the leg counts like any other.
        vault.crypto_confirm_token(&token).expect("confirmed");
        quote(&mut vault, &pretender, "1");
        vault.recompute_holdings();
        assert_eq!(vault.net_worth().financial, amount("1000000000"));
    }

    #[test]
    fn a_token_nothing_can_price_keeps_its_coins_and_says_it_is_not_valued() {
        let (_dir, mut vault) = empty_vault();
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        let token = TokenId::new(Chain::Ethereum, UNKNOWN);
        let mut record = CryptoToken::new(token.clone(), "XYZ", 18);
        record.verdict = Some(TokenVerdict::Confirmed);
        vault.doc.crypto_tokens.push(record);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            CryptoAsset::Token(token.clone()),
            "100",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        let held = vault
            .doc
            .holdings
            .iter()
            .find(|have| have.store_id == wallet)
            .expect("the coins are held even unpriced");
        assert_eq!(held.quantity, amount("100"));
        assert_eq!(held.avg_cost, None, "no price known is not a zero cost");
        assert_eq!(
            vault.net_worth().financial,
            Amount::ZERO,
            "nothing values it, so it adds nothing to the capital"
        );
        let left = vault.crypto_outside_totals();
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].reason, CryptoLeftOut::NoPrice);
        assert_eq!(left[0].quantity, Some(amount("100")));
        assert_eq!(left[0].store_id.as_deref(), Some(wallet.as_str()));
        assert_eq!(left[0].asset_key, token.key());
    }

    #[test]
    fn an_account_no_wallet_shows_is_nobodys_capital_and_says_so() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        vault.doc.crypto_accounts.push(CryptoAccount::new(
            "cacc-1",
            "Cold",
            Chain::Ethereum,
            ME,
            "2026-02-01T00:00:00Z",
        ));
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();

        assert_eq!(vault.net_worth().financial, Amount::ZERO);
        let left = vault.crypto_outside_totals();
        assert_eq!(left.len(), 1, "{left:?}");
        assert_eq!(left[0].reason, CryptoLeftOut::NoWallet);
        assert_eq!(left[0].quantity, Some(amount("2")));
    }

    /// An address watched without being counted is what its own flag says.
    #[test]
    fn an_archived_account_is_watched_and_not_counted() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));
        vault.recompute_holdings();
        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));

        vault.doc.crypto_accounts[0].archived = true;
        vault.recompute_holdings();
        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), Amount::ZERO);
        assert_eq!(vault.net_worth().financial, Amount::ZERO);
    }

    /// The module's own writers have to re-derive the holdings themselves:
    /// `persist` does not, and the engine writes no transactions.
    #[test]
    fn the_modules_own_sync_puts_the_balances_into_the_holdings() {
        let (_dir, mut vault) = empty_vault();
        quote(&mut vault, &eth_ref(), "2000");
        priced(&mut vault, &eth(), "2000");
        let wallet = followed(&mut vault, "cacc-1", "Cold", ME);
        vault.doc.crypto_events.push(moved(
            "ev-1",
            "cacc-1",
            5,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        ));

        vault
            .crypto_sync_holdings()
            .expect("the holdings are saved");

        assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
        assert_eq!(vault.net_worth().financial, amount("4000"));
    }

    // -- the capital history: the months a replay cannot reach ---------------

    /// The clock every history test reads the report at.
    const NOW: &str = "2026-06-15T12:00:00Z";

    fn dated(mut tx: Transaction, stamp: &str) -> Transaction {
        at(&mut tx, stamp);
        tx
    }

    /// A vault whose capital is counted in dollars, like the reports tests.
    fn usd_vault() -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        quote_usd(&mut vault);
        (dir, vault)
    }

    /// A reading the module wrote down that day (spec E5 writes one per
    /// account on every read, with the day in the id).
    fn recorded(vault: &mut UnlockedVault, account_id: &str, moment: &str, usd: &str) {
        vault.doc.crypto_snapshots.push(CryptoSnapshot {
            id: crate::cryptotrack::engine::ids::snapshot(Some(account_id), moment),
            at: moment.to_string(),
            account_id: Some(account_id.to_string()),
            value_usd: amount(usd),
        });
    }

    fn points_of(vault: &UnlockedVault, filter: &ReportFilter) -> Vec<NetworthPoint> {
        let now = unix_secs(NOW).expect("the clock parses");
        match vault
            .report(ReportKind::Networth, filter, now, "UTC")
            .expect("the report runs")
        {
            Report::Networth(report) => report.points,
            _ => panic!("another report came back"),
        }
    }

    fn history(vault: &UnlockedVault, from: &str, to: &str) -> Vec<NetworthPoint> {
        points_of(
            vault,
            &ReportFilter {
                from: Some(from.to_string()),
                to: Some(to.to_string()),
                ..ReportFilter::default()
            },
        )
    }

    #[test]
    fn a_history_without_the_module_is_the_series_it_always_was() {
        let (_dir, mut vault) = usd_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        for (sum, stamp) in [
            ("1000", "2026-03-02T10:00:00Z"),
            ("500", "2026-05-02T10:00:00Z"),
        ] {
            vault
                .add_transaction(dated(income(&bank, sum, "salary"), stamp))
                .expect("the top-up is booked");
        }
        let points = history(&vault, "2026-03", "2026-05");
        assert_eq!(points.len(), 3);
        assert_eq!(
            points[2].financial,
            amount("1500"),
            "the series has something in it, so the comparison means something"
        );
        let before = serde_json::to_vec(&points).expect("the points serialize");

        // Readings without a followed address: the module is not in this vault.
        recorded(&mut vault, "cacc-1", "2026-04-10T00:00:00Z", "5000");

        assert_eq!(
            serde_json::to_vec(&history(&vault, "2026-03", "2026-05"))
                .expect("the points serialize"),
            before,
            "a vault without the module keeps its series byte for byte"
        );
    }

    #[test]
    fn three_months_of_readings_are_three_points_of_the_history() {
        let (_dir, mut vault) = usd_vault();
        followed(&mut vault, "cacc-1", "Cold", ME);
        recorded(&mut vault, "cacc-1", "2026-03-20T00:00:00Z", "100");
        recorded(&mut vault, "cacc-1", "2026-04-20T00:00:00Z", "200");
        recorded(&mut vault, "cacc-1", "2026-05-20T00:00:00Z", "300");

        let points = history(&vault, "2026-03", "2026-05");
        let values: Vec<Amount> = points.iter().map(|point| point.crypto).collect();
        assert_eq!(values, vec![amount("100"), amount("200"), amount("300")]);
        for point in &points {
            assert_eq!(
                point.financial, point.crypto,
                "what the module holds is financial capital"
            );
            assert_eq!(point.net, point.crypto);
        }
    }

    #[test]
    fn a_month_with_no_reading_keeps_the_capital_it_had() {
        let (_dir, mut vault) = usd_vault();
        followed(&mut vault, "cacc-1", "Cold", ME);
        recorded(&mut vault, "cacc-1", "2026-03-20T00:00:00Z", "100");
        recorded(&mut vault, "cacc-1", "2026-05-20T00:00:00Z", "300");

        let points = history(&vault, "2026-02", "2026-05");
        assert_eq!(points.len(), 4);
        assert_eq!(
            points[0].crypto,
            Amount::ZERO,
            "before the first reading the module is not in the series at all"
        );
        assert_eq!(points[1].crypto, amount("100"));
        assert_eq!(
            points[2].crypto,
            amount("100"),
            "a month the module did not report is not a drop to zero"
        );
        assert_eq!(points[3].crypto, amount("300"));
    }

    #[test]
    fn a_past_point_does_not_count_an_archived_hand_kept_wallet_twice() {
        let (_dir, mut vault) = usd_vault();
        // Kept by hand until the address was connected.
        let old = store(&mut vault, "Old cold", StoreKind::Crypto, None);
        vault
            .add_transaction(dated(
                income_of(&old, eth_ref(), "2", "2000", "bought"),
                "2026-03-02T10:00:00Z",
            ))
            .expect("the purchase is booked");
        assert_eq!(
            history(&vault, "2026-03", "2026-03")[0].crypto,
            amount("4000")
        );

        archive(&mut vault, &old);
        followed(&mut vault, "cacc-1", "Cold", ME);
        let mut arrival = moved(
            "ev-1",
            "cacc-1",
            2,
            eth(),
            "2",
            CryptoDirection::In,
            OUTSIDE,
        );
        arrival.time = "2026-04-02T10:00:00Z".into();
        vault.doc.crypto_events.push(arrival);
        recorded(&mut vault, "cacc-1", "2026-04-20T00:00:00Z", "4000");

        let points = history(&vault, "2026-03", "2026-04");
        assert_eq!(
            points[0].crypto,
            amount("4000"),
            "before the module watched them, the hand-kept history is the only one there is"
        );
        assert_eq!(
            points[1].crypto,
            amount("4000"),
            "the same 2 ETH, counted once"
        );
        assert_eq!(points[1].financial, amount("4000"));
    }

    #[test]
    fn an_archived_account_is_out_of_the_history_as_it_is_out_of_the_present() {
        let (_dir, mut vault) = usd_vault();
        followed(&mut vault, "cacc-1", "Cold", ME);
        recorded(&mut vault, "cacc-1", "2026-03-20T00:00:00Z", "100");
        assert_eq!(
            history(&vault, "2026-03", "2026-03")[0].crypto,
            amount("100")
        );

        vault.doc.crypto_accounts[0].archived = true;

        assert_eq!(
            history(&vault, "2026-03", "2026-03")[0].crypto,
            Amount::ZERO,
            "watched but not counted, with no step left in the chart"
        );
    }

    #[test]
    fn a_report_scoped_to_another_wallet_leaves_the_module_out() {
        let (_dir, mut vault) = usd_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        vault
            .add_transaction(dated(
                income(&bank, "1000", "salary"),
                "2026-03-02T10:00:00Z",
            ))
            .expect("the top-up is booked");
        followed(&mut vault, "cacc-1", "Cold", ME);
        recorded(&mut vault, "cacc-1", "2026-03-20T00:00:00Z", "100");

        let points = points_of(
            &vault,
            &ReportFilter {
                from: Some("2026-03".into()),
                to: Some("2026-03".into()),
                scope: StoreScope {
                    store_ids: vec![bank.clone()],
                    include_children: false,
                },
                ..ReportFilter::default()
            },
        );
        assert_eq!(points[0].crypto, Amount::ZERO);
        assert_eq!(points[0].financial, amount("1000"));
    }
}
