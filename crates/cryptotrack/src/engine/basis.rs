//! Holdings, cost basis and profit, folded out of the operations.
//!
//! The method is **average cost**: one pool per asset, holding the quantity
//! and what that quantity cost; a disposal takes cost out of the pool in
//! proportion to what left and books the difference against the proceeds as
//! realized profit. FIFO would need lots, a lot number inside every event,
//! and would be rewritten by every refetch; an average needs nothing stored
//! at all, which is why nothing derived is stored (`model::cryptotrack`).
//!
//! Four rules make the numbers mean what the user was promised.
//!
//! * **A move between the user's own places is not a sale** (decision 4 of
//!   the plan). The cost pool is the whole module's, so a transfer from one
//!   followed address to another touches only where the coins are, never the
//!   cost and never the profit. The basis carries across by construction,
//!   not by copying it.
//! * **A deposit into a protocol moves coins, an exit returns them**
//!   (decision 5). What is deposited leaves the wallet for
//!   [`AssetState::in_positions`] at the cost it was carried at; nothing is
//!   realized either way. What comes back above what went in is what the
//!   protocol paid: income, booked with its own cost, so no profit appears
//!   out of nowhere. The receipt token a protocol hands over (`aUSDC` for
//!   `USDC`) is not credited — it *is* the position, and counting both would
//!   double the money.
//! * **Gas is not a trade.** Coins burnt for gas leave the pool at the cost
//!   they were carried at; what they were worth goes to
//!   [`AssetState::gas_usd`], and the gain those particular coins had
//!   gathered is realized like any other disposal. So
//!   `realized + (value - cost) - gas` is the whole result and no dollar is
//!   counted twice (the test `gas_is_a_cost_of_its_own_and_not_a_trading_profit`
//!   pins that identity).
//! * **Nothing unknown becomes a zero.** A quantity whose price is not known
//!   is held outside the priced part of the pool, so the average stays the
//!   average of what is actually known, and selling it realizes nothing
//!   rather than realizing a phantom profit. Every such gap is named in
//!   [`AssetState::doubts`], which is what marks a token's profit as
//!   imprecise.
//!
//! Determinism: the fold sorts the operations by `(time, id)` itself and
//! keeps everything in `BTreeMap`s, so the order the pages arrived in cannot
//! reach the result (the property test
//! `the_order_the_batches_arrived_in_does_not_change_a_total`).

use std::collections::{BTreeMap, BTreeSet};

use mwt_base::Amount;
use mwt_model::{CryptoAsset, CryptoDirection, CryptoLeg, CryptoTxKind};

use super::math;
use super::overrides::EventView;

/// What a unit of an asset was worth. Implemented over the document's price
/// cache (and over a linked fiat operation, which outranks the market) by
/// `crate::service::UnlockedVault`.
pub trait Prices {
    /// USD for one unit of `asset` at `time` for that operation. `None` is
    /// "not known", never zero.
    fn unit_usd(&self, asset: &CryptoAsset, time: &str, event_id: &str) -> Quote;
}

/// One answer of a [`Prices`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Quote {
    /// `None` when nothing usable is known; never zero.
    pub value: Option<Amount>,
    /// A stand-in from another day, not the price of this moment.
    pub stale: bool,
    /// The user's own number rather than a market quote, and so above one:
    /// the price implied by the fiat operation this one is chained to
    /// (decision P6), or the basis the user declared for a token's old
    /// purchases (`CryptoToken::basis_usd`).
    pub linked: bool,
}

/// Why an asset's cost or profit is not exact. The presence of any of these
/// is what the screen shows as "imprecise".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Doubt {
    /// Something moved while no price of it was known.
    MissingPrice,
    /// A quantity or a product does not fit a decimal: a junk token.
    TooLarge,
    /// The network's own balance disagreed with the operations and the
    /// difference was written in ([`super::reconcile`]).
    Reconciled,
    /// What the address held before its start date was priced at the market
    /// of that day, because the user declared no basis for it.
    OpeningAtMarket,
    /// Coins went somewhere the module cannot follow (a bridge): they left
    /// with their cost and realized nothing.
    LeftTheModule,
    /// More left than ever arrived: the history does not reach far enough
    /// back yet.
    MoreLeftThanArrived,
}

/// One asset's pool.
#[derive(Debug, Clone, PartialEq)]
pub struct AssetState {
    pub asset: CryptoAsset,
    /// In the wallets.
    pub quantity: Amount,
    /// Deposited into protocols, still owned.
    pub in_positions: Amount,
    /// What the priced part of the holding cost.
    pub cost_usd: Amount,
    /// The part of the holding [`AssetState::cost_usd`] covers.
    pub priced_quantity: Amount,
    /// Profit and loss already taken.
    pub realized_usd: Amount,
    /// What was burnt as gas was worth.
    pub gas_usd: Amount,
    /// What arrived without being paid for: protocol income, rewards.
    pub income_usd: Amount,
    /// A price of another day stood in somewhere.
    pub stale_prices: bool,
    /// A cost basis came from a linked fiat operation.
    pub linked_basis: bool,
    pub doubts: BTreeSet<Doubt>,
}

impl AssetState {
    pub fn new(asset: CryptoAsset) -> Self {
        Self {
            asset,
            quantity: Amount::ZERO,
            in_positions: Amount::ZERO,
            cost_usd: Amount::ZERO,
            priced_quantity: Amount::ZERO,
            realized_usd: Amount::ZERO,
            gas_usd: Amount::ZERO,
            income_usd: Amount::ZERO,
            stale_prices: false,
            linked_basis: false,
            doubts: BTreeSet::new(),
        }
    }

    /// Everything owned: in the wallets and in the protocols.
    pub fn owned(&self) -> Amount {
        math::add(self.quantity, self.in_positions).unwrap_or(self.quantity)
    }

    /// USD per unit of the priced part, or `None` — never zero.
    pub fn average_usd(&self) -> Option<Amount> {
        math::div(self.cost_usd, self.priced_quantity)
    }

    /// Whether this asset's cost and profit carry a known gap.
    pub fn imprecise(&self) -> bool {
        !self.doubts.is_empty()
    }

    fn note(&mut self, doubt: Doubt) {
        self.doubts.insert(doubt);
    }

    fn read(&mut self, quote: &Quote) -> Option<Amount> {
        self.stale_prices |= quote.stale && quote.value.is_some();
        self.linked_basis |= quote.linked && quote.value.is_some();
        if quote.value.is_none() {
            self.note(Doubt::MissingPrice);
        }
        quote.value
    }

    /// Take the cost of `quantity` out of the pool, in proportion, and say
    /// how much that was. The part whose cost was never known takes none and
    /// leaves a doubt behind.
    fn take_cost(&mut self, quantity: Amount) -> (Amount, Amount) {
        let priced = math::min(quantity, self.priced_quantity.max(Amount::ZERO));
        if !priced.is_positive() {
            if quantity.is_positive() {
                self.note(Doubt::MissingPrice);
            }
            return (Amount::ZERO, Amount::ZERO);
        }
        let cost = math::share(self.cost_usd, priced, self.priced_quantity).unwrap_or(Amount::ZERO);
        self.cost_usd = math::sub(self.cost_usd, cost).unwrap_or(Amount::ZERO);
        self.priced_quantity = math::sub(self.priced_quantity, priced).unwrap_or(Amount::ZERO);
        if priced < quantity {
            self.note(Doubt::MissingPrice);
        }
        (cost, priced)
    }

    fn add_quantity(&mut self, quantity: Amount) {
        match math::add(self.quantity, quantity) {
            Some(sum) => self.quantity = sum,
            None => self.note(Doubt::TooLarge),
        }
    }

    fn take_quantity(&mut self, quantity: Amount) {
        match math::sub(self.quantity, quantity) {
            Some(rest) => self.quantity = rest,
            None => self.note(Doubt::TooLarge),
        }
        if self.owned().is_negative() {
            self.note(Doubt::MoreLeftThanArrived);
        }
    }

    /// Coins arrived from outside the module and were paid for.
    fn acquire(&mut self, quantity: Amount, quote: &Quote) {
        self.add_quantity(quantity);
        if let Some(price) = self.read(quote) {
            match math::mul(quantity, price) {
                Some(cost) => {
                    self.cost_usd = math::add(self.cost_usd, cost).unwrap_or(self.cost_usd);
                    self.priced_quantity =
                        math::add(self.priced_quantity, quantity).unwrap_or(self.priced_quantity);
                }
                None => self.note(Doubt::TooLarge),
            }
        }
    }

    /// Coins left the module for someone else: a sale.
    fn dispose(&mut self, quantity: Amount, quote: &Quote) {
        let (cost, priced) = self.take_cost(quantity);
        self.take_quantity(quantity);
        let Some(price) = self.read(quote) else {
            return;
        };
        // Only the part whose cost was known realizes anything; the rest
        // would be a profit invented out of a missing purchase.
        match math::mul(priced, price) {
            Some(proceeds) => {
                let profit = math::sub(proceeds, cost).unwrap_or(Amount::ZERO);
                self.realized_usd =
                    math::add(self.realized_usd, profit).unwrap_or(self.realized_usd);
            }
            None => self.note(Doubt::TooLarge),
        }
    }

    /// Coins went where the module cannot follow them (a bridge): they take
    /// their cost with them and realize nothing.
    fn release(&mut self, quantity: Amount) {
        self.take_cost(quantity);
        self.take_quantity(quantity);
        self.note(Doubt::LeftTheModule);
    }

    /// Coins burnt as gas.
    fn spend_gas(&mut self, quantity: Amount, quote: &Quote) {
        let (cost, priced) = self.take_cost(quantity);
        self.take_quantity(quantity);
        let Some(price) = self.read(quote) else {
            return;
        };
        match math::mul(priced, price) {
            Some(worth) => {
                self.gas_usd = math::add(self.gas_usd, worth).unwrap_or(self.gas_usd);
                let profit = math::sub(worth, cost).unwrap_or(Amount::ZERO);
                self.realized_usd =
                    math::add(self.realized_usd, profit).unwrap_or(self.realized_usd);
            }
            None => self.note(Doubt::TooLarge),
        }
    }

    /// Coins arrived without being paid for: a reward, a protocol's yield.
    fn earn(&mut self, quantity: Amount, quote: &Quote) {
        self.add_quantity(quantity);
        let Some(price) = self.read(quote) else {
            return;
        };
        match math::mul(quantity, price) {
            Some(worth) => {
                self.income_usd = math::add(self.income_usd, worth).unwrap_or(self.income_usd);
                self.cost_usd = math::add(self.cost_usd, worth).unwrap_or(self.cost_usd);
                self.priced_quantity =
                    math::add(self.priced_quantity, quantity).unwrap_or(self.priced_quantity);
            }
            None => self.note(Doubt::TooLarge),
        }
    }

    /// Into a protocol: out of the wallet, still owned, same cost.
    fn stake(&mut self, quantity: Amount) {
        self.take_quantity(quantity);
        self.in_positions = math::add(self.in_positions, quantity).unwrap_or(self.in_positions);
    }

    /// Out of a protocol: back into the wallet. What comes back above what
    /// went in is the protocol's yield.
    fn unstake(&mut self, quantity: Amount, quote: &Quote) {
        let back = math::min(quantity, self.in_positions.max(Amount::ZERO));
        self.in_positions = math::sub(self.in_positions, back).unwrap_or(Amount::ZERO);
        self.add_quantity(back);
        if let Some(extra) = math::sub(quantity, back).filter(|rest| rest.is_positive()) {
            self.earn(extra, quote);
        }
    }
}

/// What the fold needs besides the operations.
pub struct Inputs<'run, P: Prices> {
    /// Every address the user follows, normalized: a leg to one of them is a
    /// move, not a sale.
    pub own: &'run BTreeSet<String>,
    pub prices: &'run P,
    /// Whether an asset counts at all. A quarantined token answers `false`
    /// and never reaches a holding, a total or a price request (spec E2).
    pub counts: &'run dyn Fn(&CryptoAsset) -> bool,
}

/// The result of the fold.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ledger {
    /// By [`CryptoAsset::key`].
    pub assets: BTreeMap<String, AssetState>,
    /// What each followed account holds in its wallet, by `(account, asset
    /// key)`: this is what the reconciliation compares with the network.
    pub balances: BTreeMap<(String, String), Amount>,
    /// What each followed account has inside protocols, by `(account, asset
    /// key)`. Kept apart from [`Ledger::balances`] because a node reports the
    /// wallet and not the protocol, and kept per account so the value of one
    /// wallet and the value of the module add up to the same number.
    pub positions: BTreeMap<(String, String), Amount>,
    /// How many operations were counted.
    pub counted: usize,
}

impl Ledger {
    pub fn state(&self, asset_key: &str) -> Option<&AssetState> {
        self.assets.get(asset_key)
    }

    pub fn balance(&self, account_id: &str, asset_key: &str) -> Amount {
        self.balances
            .get(&(account_id.to_owned(), asset_key.to_owned()))
            .copied()
            .unwrap_or(Amount::ZERO)
    }

    /// What one account has inside protocols.
    pub fn position(&self, account_id: &str, asset_key: &str) -> Amount {
        self.positions
            .get(&(account_id.to_owned(), asset_key.to_owned()))
            .copied()
            .unwrap_or(Amount::ZERO)
    }

    /// Every account that holds or held anything, in a fixed order.
    pub fn accounts(&self) -> BTreeSet<&str> {
        self.balances
            .keys()
            .chain(self.positions.keys())
            .map(|(account_id, _)| account_id.as_str())
            .collect()
    }

    /// What the asset's pool holds beyond what the accounts account for:
    /// zero when every side of every move was seen. A non-zero drift means
    /// coins moved to an address the module does not follow yet.
    pub fn drift(&self, asset_key: &str) -> Amount {
        let owned = self
            .assets
            .get(asset_key)
            .map(|state| state.quantity)
            .unwrap_or(Amount::ZERO);
        let held = self
            .balances
            .iter()
            .filter(|((_, key), _)| key == asset_key)
            .fold(Amount::ZERO, |sum, (_, quantity)| {
                math::add(sum, *quantity).unwrap_or(sum)
            });
        math::sub(owned, held).unwrap_or(Amount::ZERO)
    }

    fn asset_mut(&mut self, asset: &CryptoAsset) -> &mut AssetState {
        self.assets
            .entry(asset.key())
            .or_insert_with(|| AssetState::new(asset.clone()))
    }

    fn move_balance(&mut self, account_id: &str, asset: &CryptoAsset, delta: Amount) {
        let slot = self
            .balances
            .entry((account_id.to_owned(), asset.key()))
            .or_insert(Amount::ZERO);
        *slot = math::add(*slot, delta).unwrap_or(*slot);
    }

    fn move_position(&mut self, account_id: &str, asset: &CryptoAsset, delta: Amount) {
        let slot = self
            .positions
            .entry((account_id.to_owned(), asset.key()))
            .or_insert(Amount::ZERO);
        *slot = math::add(*slot, delta).unwrap_or(*slot);
    }
}

/// Operations in, holdings and profit out.
pub fn fold<P: Prices>(views: &[EventView<'_>], inputs: &Inputs<'_, P>) -> Ledger {
    let mut ordered: Vec<&EventView<'_>> = views.iter().collect();
    ordered.sort_by(|left, right| {
        (&left.event.time, &left.event.id).cmp(&(&right.event.time, &right.event.id))
    });
    let mut ledger = Ledger::default();
    for view in ordered {
        count(&mut ledger, view, inputs);
    }
    ledger
}

fn count<P: Prices>(ledger: &mut Ledger, view: &EventView<'_>, inputs: &Inputs<'_, P>) {
    let event = view.event;
    ledger.counted += 1;
    // A difference the engine itself wrote says that this asset's quantity
    // did not come from operations, so its cost cannot be exact. The record
    // carries that fact, which is why every device marks the same assets
    // without storing anything derived.
    let adjustment = super::ids::adjustment(&event.id);
    if adjustment == Some(super::ids::Adjustment::Reconciled) {
        for leg in &event.legs {
            if (inputs.counts)(&leg.asset) {
                ledger.asset_mut(&leg.asset).note(Doubt::Reconciled);
            }
        }
    }
    for leg in &event.legs {
        if !(inputs.counts)(&leg.asset) {
            continue;
        }
        let quote = inputs.prices.unit_usd(&leg.asset, &event.time, &event.id);
        let action = action_of(view.kind, leg.direction, own_side(leg, inputs));
        if action == Action::Ignore {
            // A receipt token is the position itself: it gets no holding of
            // its own, not even an empty one.
            continue;
        }
        {
            let state = ledger.asset_mut(&leg.asset);
            // What the address held before its start date is priced at the
            // market of that day — a guess — unless the user declared what
            // those coins cost (`CryptoToken::basis_usd`), which comes back
            // as the user's own number and needs no apology.
            if adjustment == Some(super::ids::Adjustment::Opening) && !quote.linked {
                state.note(Doubt::OpeningAtMarket);
            }
            match action {
                Action::Acquire => state.acquire(leg.amount, &quote),
                Action::Dispose => state.dispose(leg.amount, &quote),
                Action::Earn => state.earn(leg.amount, &quote),
                Action::Release => state.release(leg.amount),
                Action::ToPosition => state.stake(leg.amount),
                Action::FromPosition => state.unstake(leg.amount, &quote),
                // A move between the user's own addresses: only the wallets
                // change, the pool and the profit do not.
                Action::Move | Action::Ignore => {}
            }
        }
        let wallet = match leg.direction {
            CryptoDirection::In => leg.amount,
            CryptoDirection::Out => -leg.amount,
        };
        if !wallet.is_zero() {
            ledger.move_balance(&event.account_id, &leg.asset, wallet);
        }
        // A deposit leaves the wallet for the protocol and an exit comes
        // back, so the account's two sides move together.
        match action {
            Action::ToPosition => {
                ledger.move_position(&event.account_id, &leg.asset, leg.amount);
            }
            Action::FromPosition => {
                let back = math::min(
                    leg.amount,
                    ledger
                        .position(&event.account_id, &leg.asset.key())
                        .max(Amount::ZERO),
                );
                ledger.move_position(&event.account_id, &leg.asset, -back);
            }
            _ => {}
        }
    }
    if let Some(gas) = &event.gas
        && (inputs.counts)(&gas.asset)
    {
        let quote = inputs.prices.unit_usd(&gas.asset, &event.time, &event.id);
        ledger.asset_mut(&gas.asset).spend_gas(gas.amount, &quote);
        ledger.move_balance(&event.account_id, &gas.asset, -gas.amount);
    }
}

/// Whether the other side of this leg is an address the user also follows.
fn own_side<P: Prices>(leg: &CryptoLeg, inputs: &Inputs<'_, P>) -> bool {
    leg.counterparty
        .as_deref()
        .is_some_and(|address| inputs.own.contains(address))
}

/// What one leg does to the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    /// Bought from outside the module.
    Acquire,
    /// Sold outside the module.
    Dispose,
    /// Arrived without being paid for.
    Earn,
    /// Went where the module cannot follow.
    Release,
    /// Into a protocol.
    ToPosition,
    /// Back out of a protocol.
    FromPosition,
    /// Between the user's own addresses.
    Move,
    /// A receipt token of a protocol: not a holding of its own.
    Ignore,
}

/// What a leg of this kind, in this direction, with that counterparty does.
fn action_of(kind: CryptoTxKind, direction: CryptoDirection, own: bool) -> Action {
    match (kind, direction) {
        (CryptoTxKind::Stake, CryptoDirection::Out) => Action::ToPosition,
        (CryptoTxKind::Unstake, CryptoDirection::In) => Action::FromPosition,
        (CryptoTxKind::Stake, CryptoDirection::In)
        | (CryptoTxKind::Unstake, CryptoDirection::Out) => Action::Ignore,
        (CryptoTxKind::Claim, CryptoDirection::In) => Action::Earn,
        (CryptoTxKind::Bridge, CryptoDirection::Out) => Action::Release,
        (_, _) if own => Action::Move,
        (_, CryptoDirection::In) => Action::Acquire,
        (_, CryptoDirection::Out) => Action::Dispose,
    }
}
