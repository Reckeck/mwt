//! IPC types of the crypto module's operations: the list with its filters,
//! one operation's detail, and the corrections the user puts over what a
//! source reported.
//!
//! Owner: spec E10. There is no creating or deleting an operation anywhere: a
//! correction changes classification and attributes only, and it lives in
//! `crypto_overrides` so a refetch cannot lose it.
//!
//! Commands: `crates/app/src/commands/cryptotrack/operations.rs`.

use serde::{Deserialize, Serialize};

use mwt_base::Amount;
use mwt_model::Chain;
use mwt_model::{CryptoDirection, CryptoFee, CryptoTxKind, TrackingRef};

/// Rows of one page of the list: what one `Show more` adds.
pub const OPERATIONS_PAGE: usize = 100;

/// What the operations list is narrowed to. Every field is optional, and the
/// whole filter lives in the page's query parameters, so a link from another
/// screen (spec E15) opens the list already narrowed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpFilter {
    /// The followed accounts to read; empty is every one.
    #[serde(default)]
    pub account_ids: Vec<String>,
    #[serde(default)]
    #[ts(optional)]
    pub chain: Option<Chain>,
    /// A token: its price key (`ethereum|0x…`, `ethereum|native`) or its symbol.
    #[serde(default)]
    #[ts(optional)]
    pub token: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub protocol: Option<String>,
    /// The kind as the user sees it, a correction included.
    #[serde(default)]
    #[ts(optional)]
    pub kind: Option<CryptoTxKind>,
    /// Part of the sender or recipient address.
    #[serde(default)]
    #[ts(optional)]
    pub address: Option<String>,
    /// Also the operations the user hid from the totals.
    #[serde(default)]
    pub show_hidden: bool,
}

/// One asset movement of a row, with its symbol already resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpLeg {
    pub direction: CryptoDirection,
    /// The price key of the asset.
    pub asset_key: String,
    /// What to show: the token's symbol, the coin's, an exchange's ticker.
    pub symbol: String,
    /// Whole units, never negative; the direction says which way.
    pub amount: Amount,
}

/// What the module's own entry stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum OwnEntry {
    /// What the address held before the start date: the history before it is
    /// not read, so the balance starts from this.
    Opening,
    /// A correction to what the chain's own balance reported, so the total
    /// agrees with the network.
    Reconciliation,
}

/// One operation as the list shows it: the event with the user's correction
/// already laid over it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpRow {
    pub id: String,
    pub account_id: String,
    /// The wallet or exchange the account is shown by.
    pub account_name: String,
    /// The network. `None` for a coin on an exchange: it does not lie on a
    /// network, and the name «Fiat» must never be shown for it.
    #[serde(default)]
    #[ts(optional)]
    pub network: Option<Chain>,
    /// The exchange, when the operation is one of its fills.
    #[serde(default)]
    #[ts(optional)]
    pub exchange: Option<String>,
    /// RFC3339 UTC.
    pub time: String,
    /// The kind after the correction.
    pub kind: CryptoTxKind,
    /// The kind the source reported.
    pub source_kind: CryptoTxKind,
    pub legs: Vec<CryptoOpLeg>,
    #[serde(default)]
    #[ts(optional)]
    pub gas: Option<CryptoFee>,
    /// The gas coin's symbol.
    #[serde(default)]
    #[ts(optional)]
    pub gas_symbol: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub protocol: Option<String>,
    pub from: String,
    pub to: String,
    /// A short, readable form of the transaction hash, or of the fill id.
    pub reference: String,
    /// The USD value of the movement at its day, when a price is cached.
    #[serde(default)]
    #[ts(optional)]
    pub value_usd: Option<Amount>,
    #[serde(default)]
    #[ts(optional)]
    pub flow_id: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub contact_id: Option<String>,
    /// The contact's name: the chosen one, else the one whose saved address
    /// matches the other side (shown, never stored).
    #[serde(default)]
    #[ts(optional)]
    pub contact_name: Option<String>,
    /// Whether `contact_name` came from an address match, not from a choice.
    pub contact_guessed: bool,
    #[serde(default)]
    #[ts(optional)]
    pub comment: Option<String>,
    pub categories: Vec<String>,
    pub hidden: bool,
    /// A correction stands over this operation.
    pub corrected: bool,
    /// Set when this is not an operation of the chain but an entry the module
    /// wrote itself to explain a balance: no hash, no explorer page, and its
    /// kind cannot be corrected.
    #[serde(default)]
    #[ts(optional)]
    pub own_entry: Option<OwnEntry>,
    /// A rule for its contract decided its kind, its category or its
    /// counterparty (not its own correction).
    pub by_rule: bool,
    /// Whether a link to a page about it exists. The address itself is only
    /// built in the detail, when the user opens the operation.
    pub linkable: bool,
}

/// One page of the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpsPage {
    pub items: Vec<CryptoOpRow>,
    /// How many operations match the filter in all.
    pub total: u32,
    /// What the filter can offer, over the accounts asked for.
    pub facets: CryptoOpFacets,
    /// On the first page only: an operation that looks like an exchange.
    #[serde(default)]
    #[ts(optional)]
    pub suggestion: Option<CryptoExchangeSuggestion>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpFacets {
    pub chains: Vec<Chain>,
    pub protocols: Vec<String>,
    pub tokens: Vec<CryptoOpTokenFacet>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOpTokenFacet {
    pub key: String,
    pub symbol: String,
}

/// Which page of the list to read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoListOperationsInput {
    #[serde(default)]
    pub filter: CryptoOpFilter,
    #[serde(default)]
    pub offset: u32,
    /// Capped at [`OPERATIONS_PAGE`].
    #[serde(default)]
    #[ts(optional)]
    pub limit: Option<u32>,
}

/// Which operation to open, and whether to look for what it could be paid by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOperationDetailInput {
    pub id: String,
    /// Ask for the fiat operations that look like what this one was paid with.
    #[serde(default)]
    pub with_candidates: bool,
}

/// What one member of an exchange chain is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum ChainSide {
    Fiat,
    Crypto,
}

/// One member of the chain an operation is in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoChainMember {
    pub side: ChainSide,
    pub id: String,
    /// The wallet it belongs to.
    pub place: String,
    /// Signed: the way it moved for its own wallet.
    pub amount: Amount,
    pub currency: String,
    pub time: String,
    /// The operation the detail is about.
    pub is_this: bool,
}

/// Why a fiat operation looks like the payment for a crypto one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum CandidateReason {
    SameDay,
    /// The dates are `days` apart.
    NearDate {
        days: u32,
    },
    /// The sum fits the rate of the day within a few percent.
    AmountFits,
}

/// A fiat operation the user may tie to a crypto one. The app only suggests:
/// nothing is linked until the user confirms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoLinkCandidate {
    pub transaction_id: String,
    pub place: String,
    /// Signed, in the currency the wallet holds.
    pub amount: Amount,
    pub currency: String,
    pub time: String,
    pub comment: String,
    pub reasons: Vec<CandidateReason>,
    /// Fiat per one unit of the coin, when both sums are known.
    #[serde(default)]
    #[ts(optional)]
    pub rate: Option<Amount>,
    /// The coin's symbol the rate is for.
    #[serde(default)]
    #[ts(optional)]
    pub rate_symbol: Option<String>,
}

/// What the user decided for every operation that goes through one contract,
/// those already read and those a later refetch brings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoContractRule {
    #[serde(default)]
    #[ts(optional)]
    pub kind: Option<CryptoTxKind>,
    pub categories: Vec<String>,
    #[serde(default)]
    #[ts(optional)]
    pub contact_id: Option<String>,
}

/// One operation with everything the card shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoOperationDetail {
    pub row: CryptoOpRow,
    /// The page about it (`https` only), built from the stored reference now,
    /// with the user's explorer where one is set. Nothing is requested from
    /// it: the address is only opened when the user clicks.
    #[serde(default)]
    #[ts(optional)]
    pub url: Option<String>,
    /// For an exchange fill: the id to look for on the page.
    #[serde(default)]
    #[ts(optional)]
    pub url_note: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub method: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub block: Option<u64>,
    pub tracking: TrackingRef,
    /// The other members of the chain, this one among them.
    pub chain: Vec<CryptoChainMember>,
    /// The cost basis comes from the linked operation, not the market.
    pub basis_from_chain: bool,
    /// How many operations go to the same contract.
    pub same_contract_count: u32,
    /// The rule the user set for this operation's contract, if any: it is
    /// visible here and removed with `remove_contract_rule`.
    #[serde(default)]
    #[ts(optional)]
    pub contract_rule: Option<CryptoContractRule>,
    /// The contract the rule is about, as the user can read it.
    #[serde(default)]
    #[ts(optional)]
    pub contract: Option<String>,
    /// The best guesses of what paid for this operation; asked for with
    /// `with_candidates`.
    pub candidates: Vec<CryptoLinkCandidate>,
}

/// A field of a correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "snake_case")]
pub enum OverrideField {
    Kind,
    Comment,
    Categories,
    Contact,
    /// The chain: breaks the link with the fiat operation.
    Flow,
}

/// Put a correction over one operation, or over every operation of its
/// contract. The fields given are set, the ones in `clear` go back to what
/// the source reported, everything else is left. An amount cannot be given:
/// a correction changes classification and attributes only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoSetOverrideInput {
    pub event_id: String,
    #[serde(default)]
    #[ts(optional)]
    pub kind: Option<CryptoTxKind>,
    #[serde(default)]
    #[ts(optional)]
    pub comment: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub categories: Option<Vec<String>>,
    #[serde(default)]
    #[ts(optional)]
    pub contact_id: Option<String>,
    /// Hide the operation from the lists and the totals, or show it again.
    #[serde(default)]
    #[ts(optional)]
    pub hidden: Option<bool>,
    /// Tie the operation to this fiat operation: the declaration «this is an
    /// exchange». Both get one chain id; no amount changes.
    #[serde(default)]
    #[ts(optional)]
    pub link_transaction_id: Option<String>,
    #[serde(default)]
    pub clear: Vec<OverrideField>,
    /// Make what is given (the kind, the categories, the contact) a rule for
    /// the contract of this operation: it reaches every operation that goes
    /// through it, now and after any later refetch, below an operation's own
    /// correction. The fields in `clear` are taken off the rule.
    #[serde(default)]
    pub same_contract: bool,
    /// Remove the rule of this operation's contract.
    #[serde(default)]
    pub remove_contract_rule: bool,
    /// «Not an exchange»: stop suggesting a fiat operation for this one (or
    /// take the answer back).
    #[serde(default)]
    #[ts(optional)]
    pub not_exchange: Option<bool>,
}

/// What a correction did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoSetOverrideResult {
    /// The chain id the operation is in now, if any.
    #[serde(default)]
    #[ts(optional)]
    pub flow_id: Option<String>,
    /// How many operations the correction reached.
    pub applied: u32,
}

/// The one best guess of an unlinked incoming operation that was paid for by
/// a fiat one, for the banner «looks like an exchange».
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
pub struct CryptoExchangeSuggestion {
    pub event_id: String,
    pub symbol: String,
    pub amount: Amount,
    pub candidate: CryptoLinkCandidate,
}
