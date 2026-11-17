//! The Bybit source (stage 2, spec E7): the exchange's own history, read
//! through its V5 API and handed over as the module's operations.
//!
//! The source is pure like every source ([`crate::cryptotrack::source`]): it
//! reads the exchange and returns a [`FetchPage`]; it never touches the
//! document and it holds no clock of its own — "now" and the date to stop at
//! are given to it, so the same inputs give the same bytes on every device.
//!
//! ## Why an exchange balance is a pseudo-network asset
//!
//! A coin on an exchange sits on no network: Bybit reports `USDT`, not a
//! contract on a chain, and the user cannot point at a block. The stored
//! model has one place for "an asset that is not on a network":
//! [`Chain::Fiat`], the pseudo-variant the ledger already uses for a coin
//! whose network is unresolved (`UnlockedVault::asset_for_import` leaves
//! `chain: None` for exactly that reason). So an exchange coin is
//! [`CryptoAsset::Token`] on [`Chain::Fiat`] with the ticker as its
//! "contract", which gives it a stable key (`fiat|USDT`) and keeps it apart
//! from the same ticker on a real network. [`exchange_asset`] is the only
//! place that builds it, so a later spec can move the encoding in one edit.
//!
//! A token that is not on the built-in contract list is quarantined out of
//! every total (spec E2), and an exchange ticker never will be on it, so the
//! conversion registers every coin it meets with
//! [`crate::model::cryptotrack::TokenVerdict::Confirmed`]: these coins were
//! already in the user's ledger, and dropping them out of the totals would
//! lose money the user can see.
//!
//! ## Deduplication
//!
//! Bybit's `execId` is the key the old import already deduplicated on, and
//! it is only unique inside Bybit, so the module's event id carries the
//! source's name as well: `cev:{account}:bybit:{exec_id}`
//! ([`crate::cryptotrack::engine::ids::event`] over
//! [`TrackingRef::ExchangeHistory`]). An id is a function of the facts and of
//! nothing else — no clock, no counter — so converting twice, or converting
//! on two devices, writes the same record rather than a second one.
//!
//! ## Pages, the cursor and retries
//!
//! `/v5/execution/list` has no single cursor over all of history: it takes a
//! `startTime`/`endTime` window of at most seven days and pages inside that
//! window with `nextPageCursor`. The walk is therefore: for each product
//! (spot, linear, inverse), windows backwards from "now" to the start date,
//! and inside a window, pages. [`Cursor`] names exactly where that walk
//! stands and is what the device keeps
//! ([`crate::model::cryptotrack::CryptoCursor`]), so an interruption on page
//! N is resumed at page N and a page that arrives twice produces the same
//! ids, not duplicates.
//!
//! A failure that retrying can fix (the network, a rate limit, a busy
//! exchange) is retried with a growing wait ([`backoff_ms`]); a refusal of
//! the key itself is not ([`retryable`]), because it would only be refused
//! again.
//!
//! ## Secrets
//!
//! The key and the secret live only in
//! [`crate::vault_io::device_secrets::DeviceSecrets`] and, for the length of a
//! pass, inside the transport ([`BybitApi`]). They are in no request type, no
//! `Debug`, no cursor, no event and no error message.

use std::future::Future;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::Amount;
use crate::cryptotrack::engine::ids;
use crate::cryptotrack::source::{FetchPage, FetchRequest, Source};
use crate::error::{Error, Result};
use crate::exchange::bybit::{ACCOUNT_EARN, ACCOUNT_UNIFIED};
use crate::exchange::{BybitClient, Category, EarnYield, Execution, InterTransfer};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoFee, CryptoLeg, CryptoTx, CryptoTxKind, TokenId,
    TrackingRef,
};
use crate::model::vault::{ImportKind, PendingImport};

/// Stable id, as it is written into a cursor record and an account's
/// `source`.
pub const SOURCE_ID: &str = "bybit";

/// The exchange name a [`TrackingRef::ExchangeHistory`] stores. Lowercase and
/// never translated: it is part of an event id.
pub const EXCHANGE: &str = "bybit";

/// The pseudo-network an exchange balance lives on; see the note at the top.
pub const EXCHANGE_CHAIN: Chain = Chain::Fiat;

/// The `decimals` an exchange coin's token record carries. An exchange
/// reports whole units (`0.02 BTC`), never base units, so nothing is ever
/// scaled by it; it is a display default only.
pub const COIN_DECIMALS: u8 = 8;

/// Rows asked for in one page; Bybit's own maximum for this endpoint.
pub const PAGE_LIMIT: u32 = 100;

/// `/v5/execution/list` refuses a window wider than seven days.
pub const WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// The products the walk reads, in order.
pub const PRODUCTS: [Category; 3] = [Category::Spot, Category::Linear, Category::Inverse];

// -- what an exchange coin is ------------------------------------------------

/// The module's asset for an exchange ticker: the ticker as the "contract" of
/// the [`Chain::Fiat`] pseudo-network, uppercased so `usdt` and `USDT` are
/// one asset. The one place this encoding is built.
pub fn exchange_asset(symbol: &str) -> CryptoAsset {
    CryptoAsset::Token(TokenId::new(
        EXCHANGE_CHAIN,
        &symbol.trim().to_ascii_uppercase(),
    ))
}

/// The ticker of an asset [`exchange_asset`] built, else `None`.
pub fn asset_ticker(asset: &CryptoAsset) -> Option<&str> {
    match asset {
        CryptoAsset::Token(token) if token.chain == EXCHANGE_CHAIN => Some(&token.contract),
        _ => None,
    }
}

/// Whether a ticker is a dollar stablecoin, so one unit of it is one dollar.
/// Asks the exchange module's own list rather than keeping a second one.
pub fn is_usd_coin(symbol: &str) -> bool {
    crate::exchange::is_usd_stablecoin(&format!("asset:{}", symbol.trim().to_ascii_uppercase()))
}

/// The account handle of an exchange wallet: what stands where an address
/// would stand. Derived from the wallet's id, so two devices agree and no
/// part of a key is in it.
pub fn account_handle(store_id: &str) -> String {
    format!("{EXCHANGE}:{store_id}")
}

/// The id of the followed account of an exchange wallet. A function of the
/// wallet's id alone: both devices build the same one.
pub fn account_id_for(store_id: &str) -> String {
    account_handle(store_id)
}

/// The `exec_id` an event was read under, when it came from this exchange.
pub fn exec_id_of(event: &CryptoTx) -> Option<&str> {
    match &event.tracking {
        TrackingRef::ExchangeHistory { exchange, exec_id } if exchange == EXCHANGE => {
            Some(exec_id.as_str())
        }
        _ => None,
    }
}

// -- one record of the exchange becomes one operation ------------------------

/// The operation a Bybit record becomes, or `None` when the record cannot be
/// read.
///
/// The input is a [`PendingImport`]: the shape the old import already parsed
/// Bybit's three lists into (fills, moves between the user's own accounts,
/// Earn payouts), checked and with exact quantities. Both ways into the
/// module go through this one function — the legacy queue being adopted and a
/// fresh sweep — so neither can produce an operation the other would not.
///
/// `PendingImport` itself is no longer stored (`AGENTS.md`, rule 5: its
/// fields stay readable, nothing writes them any more); here it is a
/// transport shape only.
pub fn event_from_pending(account_id: &str, imp: &PendingImport) -> Option<CryptoTx> {
    let exec_id = imp.exec_id.trim();
    if exec_id.is_empty() || account_id.trim().is_empty() || imp.occurred_at.trim().is_empty() {
        return None;
    }
    let tracking = TrackingRef::ExchangeHistory {
        exchange: EXCHANGE.to_owned(),
        exec_id: exec_id.to_owned(),
    };
    // The account's id and its address are the same handle
    // ([`account_handle`]), which is what the fold's set of own addresses
    // holds, so a move inside the exchange can name itself as its own
    // counterparty.
    let handle = account_id;
    let (kind, legs, from, to) = match imp.kind {
        // A fill is a swap: the quote leg leaves, the base leg arrives.
        ImportKind::Fill => {
            let sold = leg(CryptoDirection::Out, &imp.asset_sold, imp.qty_sold, None)?;
            let bought = leg(CryptoDirection::In, &imp.asset_bought, imp.qty_bought, None)?;
            (
                CryptoTxKind::Swap,
                vec![sold, bought],
                String::new(),
                ACCOUNT_UNIFIED.to_owned(),
            )
        }
        // A move between the user's own Bybit accounts: the counterparty is
        // this very account, which is what makes the fold treat it as a move
        // and leave the cost pool and the profit alone.
        ImportKind::Transfer => {
            let out = leg(
                CryptoDirection::Out,
                &imp.asset_sold,
                imp.qty_sold,
                Some(handle),
            )?;
            let into = leg(
                CryptoDirection::In,
                &imp.asset_bought,
                imp.qty_bought,
                Some(handle),
            )?;
            (
                CryptoTxKind::Transfer,
                vec![out, into],
                imp.from_account.clone().unwrap_or_default(),
                imp.to_account.clone().unwrap_or_default(),
            )
        }
        // A payout arrived without being paid for: income, not a purchase.
        ImportKind::EarnIncome => {
            let paid = leg(CryptoDirection::In, &imp.asset_bought, imp.qty_bought, None)?;
            (
                CryptoTxKind::Claim,
                vec![paid],
                String::new(),
                ACCOUNT_EARN.to_owned(),
            )
        }
    };
    let event = CryptoTx {
        id: ids::event(account_id, &tracking),
        chain: EXCHANGE_CHAIN,
        account_id: account_id.to_owned(),
        time: imp.occurred_at.trim().to_owned(),
        block: None,
        from,
        to,
        legs,
        // The exchange's trading fee. The engine's only slot for "coins paid
        // out of the holding for the operation itself" is `gas`, and it does
        // to the balance and to the cost exactly what a fee must do; the name
        // is the compromise (decision P8 keeps gas and an exchange's fee
        // apart, and an exchange fee is the one a source does report).
        gas: fee_of(imp),
        protocol: Some(EXCHANGE.to_owned()),
        method: None,
        kind,
        raw: None,
        tracking,
    };
    event.validate().ok()?;
    Some(event)
}

fn leg(
    direction: CryptoDirection,
    symbol: &str,
    amount: Amount,
    counterparty: Option<&str>,
) -> Option<CryptoLeg> {
    let symbol = symbol.trim();
    if symbol.is_empty() || !amount.is_positive() {
        return None;
    }
    Some(CryptoLeg {
        direction,
        asset: exchange_asset(symbol),
        amount,
        counterparty: counterparty.map(str::to_owned),
    })
}

fn fee_of(imp: &PendingImport) -> Option<CryptoFee> {
    let paid = imp.fee.abs();
    let symbol = imp.fee_asset.trim();
    (!paid.is_zero() && !symbol.is_empty()).then(|| CryptoFee {
        asset: exchange_asset(symbol),
        amount: paid,
    })
}

// -- the cost basis the records already carry --------------------------------

/// What one unit of an asset was worth in USD according to the record itself,
/// for every leg whose dollar price the record settles.
///
/// A fill against a dollar stablecoin states both sides exactly: one unit of
/// the stablecoin is one dollar, and one unit of the base asset is the fill
/// price. Nothing else is derivable — an `ETHBTC` fill says nothing about
/// dollars — and is left to the market cache rather than guessed.
pub fn implied_usd(imp: &PendingImport) -> Vec<(CryptoAsset, Amount)> {
    let one = Amount::new(rust_decimal::Decimal::ONE);
    let mut out = Vec::new();
    let mut note = |symbol: &str, unit: Amount| {
        let symbol = symbol.trim();
        if !symbol.is_empty() && unit.is_positive() {
            out.push((exchange_asset(symbol), unit));
        }
    };
    for symbol in [imp.asset_bought.as_str(), imp.asset_sold.as_str()] {
        if is_usd_coin(symbol) {
            note(symbol, one);
        }
    }
    if imp.kind == ImportKind::Fill && imp.price.is_positive() {
        // The base asset is the one that is not the quote; a fill quoted in a
        // stablecoin therefore prices the base at the fill price.
        let buying = imp.side.eq_ignore_ascii_case("buy");
        let (base, quote) = if buying {
            (imp.asset_bought.as_str(), imp.asset_sold.as_str())
        } else {
            (imp.asset_sold.as_str(), imp.asset_bought.as_str())
        };
        if is_usd_coin(quote) && !is_usd_coin(base) {
            note(base, imp.price);
        }
    }
    out
}

// -- where the walk stands ---------------------------------------------------

const CURSOR_VERSION: u8 = 1;
const MAX_CURSOR_LEN: usize = 1024;
const MAX_PAGE_LEN: usize = 256;

/// How far the source has read one exchange account: which product, which
/// seven-day window, and which page inside it. Kept by the device
/// ([`crate::model::cryptotrack::CryptoCursor`]) and handed back as input, so
/// it is checked like any other untrusted text — its page token goes into a
/// URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor {
    #[serde(rename = "v")]
    version: u8,
    /// The product being read.
    #[serde(rename = "c")]
    pub product: Category,
    /// End of the window being read, milliseconds since the epoch.
    #[serde(rename = "e")]
    pub window_end_ms: u64,
    /// Bybit's `nextPageCursor` inside that window; `None` is its first page.
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

impl Cursor {
    /// The first page of the newest window of the first product.
    pub fn start(now_ms: u64) -> Self {
        Self {
            version: CURSOR_VERSION,
            product: PRODUCTS[0],
            window_end_ms: now_ms,
            page: None,
        }
    }

    /// `None` is the very beginning, from `now_ms`.
    pub fn decode(text: Option<&str>, now_ms: u64) -> Result<Self> {
        let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
            return Ok(Self::start(now_ms));
        };
        if text.len() > MAX_CURSOR_LEN {
            return Err(bad("too long"));
        }
        let cursor: Cursor = serde_json::from_str(text).map_err(|_| bad("not a cursor"))?;
        if cursor.version != CURSOR_VERSION {
            return Err(bad("unknown version"));
        }
        if let Some(page) = &cursor.page
            && !page_is_safe(page)
        {
            return Err(bad("unsafe page token"));
        }
        Ok(cursor)
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Start of the window this cursor reads; never below `floor_ms`.
    pub fn window_start_ms(&self, floor_ms: u64) -> u64 {
        self.window_end_ms
            .saturating_sub(WINDOW_MS)
            .max(floor_ms.min(self.window_end_ms))
    }

    /// The next page of the same window.
    pub fn with_page(&self, page: String) -> Self {
        Self {
            page: Some(page),
            ..self.clone()
        }
    }

    /// Where to go once this window is read out: the window before it, else
    /// the first window of the next product, else nowhere — the walk is done.
    ///
    /// `now_ms` is where a product's walk starts; it comes from the caller,
    /// never from a clock in here.
    pub fn after_window(&self, floor_ms: u64, now_ms: u64) -> Option<Self> {
        let start = self.window_start_ms(floor_ms);
        if start > floor_ms {
            return Some(Self {
                version: CURSOR_VERSION,
                product: self.product,
                window_end_ms: start.saturating_sub(1),
                page: None,
            });
        }
        let next = PRODUCTS
            .iter()
            .skip_while(|product| **product != self.product)
            .nth(1)?;
        Some(Self {
            version: CURSOR_VERSION,
            product: *next,
            window_end_ms: now_ms,
            page: None,
        })
    }
}

/// A page token that is safe to put into a URL query.
fn page_is_safe(page: &str) -> bool {
    !page.is_empty()
        && page.len() <= MAX_PAGE_LEN
        && page
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'%'))
}

fn bad(why: &str) -> Error {
    Error::InvalidInput(format!("bybit cursor: {why}"))
}

// -- retries -----------------------------------------------------------------

/// Tries one page gets, the first included. The bound is what keeps a pass
/// that cannot succeed from running forever.
pub const MAX_ATTEMPTS: u32 = 4;

/// Whether retrying this failure can help. The network and a busy or
/// rate-limiting exchange can; a key the exchange refuses cannot, and trying
/// again would only be refused again.
pub fn retryable(err: &Error) -> bool {
    matches!(err, Error::Network(_))
}

/// How long to wait after attempt `attempt` failed, in milliseconds: 500,
/// then 1000, then 2000. Doubling rather than fixed, so a rate limit has room
/// to clear; capped, so no pass hangs on a long wait.
pub fn backoff_ms(attempt: u32) -> u64 {
    500u64 << attempt.clamp(1, 3).saturating_sub(1)
}

// -- the transport -----------------------------------------------------------

/// One page of the exchange's history to ask for. Carries no credential: the
/// key lives inside the transport, so it cannot reach a `Debug` line.
#[derive(Debug, Clone, Copy)]
pub struct PageRequest<'a> {
    pub product: Category,
    pub start_ms: u64,
    pub end_ms: u64,
    /// Bybit's `nextPageCursor` from the previous page of this window; `None`
    /// for its first page. Resuming at page N is what makes an interrupted
    /// pass cost one page and not a whole window.
    pub page: Option<&'a str>,
    pub limit: u32,
}

/// What one page answered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FillsPage {
    pub fills: Vec<Execution>,
    /// Bybit's `nextPageCursor`; empty when this was the last page.
    pub next_page: Option<String>,
}

/// A transport that answers one page of fills. Exists so the walk, the cursor
/// and the retries are testable with no network and no key.
pub trait Fills {
    fn page(&self, request: PageRequest<'_>) -> impl Future<Output = Result<FillsPage>> + Send;

    /// Moves between the user's own exchange accounts, newest first. Read
    /// once per pass rather than per window: the endpoint pages on its own.
    fn transfers(&self, limit: usize) -> impl Future<Output = Result<Vec<InterTransfer>>> + Send;

    /// Earn payouts, newest first.
    fn payouts(&self, limit: usize) -> impl Future<Output = Result<Vec<EarnYield>>> + Send;
}

/// The real transport: signed read-only GETs against Bybit V5. The key and
/// the secret are held zeroizing and are in no `Debug` and in no log.
pub struct BybitApi {
    client: BybitClient,
    api_key: Zeroizing<String>,
    api_secret: Zeroizing<String>,
}

impl BybitApi {
    pub fn new(client: BybitClient, api_key: &str, api_secret: &str) -> Self {
        Self {
            client,
            api_key: Zeroizing::new(api_key.to_owned()),
            api_secret: Zeroizing::new(api_secret.to_owned()),
        }
    }
}

impl std::fmt::Debug for BybitApi {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("BybitApi")
            .field("api_key", &"<redacted>")
            .field("api_secret", &"<redacted>")
            .finish()
    }
}

impl Fills for BybitApi {
    async fn page(&self, request: PageRequest<'_>) -> Result<FillsPage> {
        let page = self
            .client
            .executions_page(
                &self.api_key,
                &self.api_secret,
                request.product,
                request.start_ms,
                request.end_ms,
                request.page,
                request.limit,
            )
            .await?;
        Ok(FillsPage {
            fills: page.fills,
            next_page: page.next_page,
        })
    }

    async fn transfers(&self, limit: usize) -> Result<Vec<InterTransfer>> {
        self.client
            .inter_transfers(&self.api_key, &self.api_secret, limit)
            .await
    }

    async fn payouts(&self, limit: usize) -> Result<Vec<EarnYield>> {
        self.client
            .earn_yields(&self.api_key, &self.api_secret, limit)
            .await
    }
}

// -- the source --------------------------------------------------------------

/// The Bybit source of one exchange account.
///
/// One instance per account, because the key is per account. `now_ms` and
/// `floor_ms` are given by the caller — the source reads no clock, so a test
/// pins the whole walk.
pub struct BybitSource<F: Fills> {
    fills: F,
    account_id: String,
    /// The moment the pass started, milliseconds since the epoch.
    now_ms: u64,
    /// The date to stop walking back at, milliseconds since the epoch.
    floor_ms: u64,
    /// How long to wait after a failed attempt ([`backoff_ms`] by default).
    /// Given rather than hardcoded for the same reason `now_ms` is: the
    /// source holds no clock of its own, so a test can pin every wait to
    /// zero and still watch the retries happen.
    backoff: fn(u32) -> u64,
}

impl<F: Fills> BybitSource<F> {
    pub fn new(fills: F, account_id: &str, now_ms: u64, floor_ms: u64) -> Self {
        Self {
            fills,
            account_id: account_id.to_owned(),
            now_ms,
            floor_ms: floor_ms.min(now_ms),
            backoff: backoff_ms,
        }
    }

    /// The same source with another wait between attempts.
    pub fn with_backoff(mut self, backoff: fn(u32) -> u64) -> Self {
        self.backoff = backoff;
        self
    }

    /// One page of fills, retried while retrying can help and at most
    /// [`MAX_ATTEMPTS`] times. The failure the caller sees is the last one the
    /// exchange gave, not a made-up one.
    async fn page(&self, request: PageRequest<'_>) -> Result<FillsPage> {
        let mut attempt = 1;
        loop {
            match self.fills.page(request).await {
                Ok(page) => return Ok(page),
                Err(err) if !retryable(&err) || attempt >= MAX_ATTEMPTS => return Err(err),
                Err(_) => {
                    let wait = (self.backoff)(attempt);
                    if wait > 0 {
                        tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                    }
                    attempt += 1;
                }
            }
        }
    }

    /// The next page of this account's history.
    ///
    /// Fills page through the windows; the moves between the user's own
    /// accounts and the Earn payouts are read once, on the first page of the
    /// walk, because their endpoints page on their own.
    pub async fn fetch_page(&self, cursor: Option<&str>, limit: usize) -> Result<FetchPage> {
        let here = Cursor::decode(cursor, self.now_ms)?;
        let first = cursor.is_none();
        let request = PageRequest {
            product: here.product,
            start_ms: here.window_start_ms(self.floor_ms),
            end_ms: here.window_end_ms,
            page: here.page.as_deref(),
            limit: limit.clamp(1, PAGE_LIMIT as usize) as u32,
        };
        let page = self.page(request).await?;
        let mut events = Vec::new();
        for fill in &page.fills {
            let category = here.product.as_str();
            if let Some(imp) = PendingImport::from_execution(&self.account_id, category, fill)
                && let Some(event) = event_from_pending(&self.account_id, &imp)
            {
                events.push(event);
            }
        }
        if first {
            for row in self.fills.transfers(limit).await? {
                if let Some(imp) = PendingImport::from_transfer(&self.account_id, &row)
                    && let Some(event) = event_from_pending(&self.account_id, &imp)
                {
                    events.push(event);
                }
            }
            for row in self.fills.payouts(limit).await? {
                if let Some(imp) = PendingImport::from_earn_yield(&self.account_id, &row)
                    && let Some(event) = event_from_pending(&self.account_id, &imp)
                {
                    events.push(event);
                }
            }
        }
        let next = match page.next_page.filter(|token| page_is_safe(token)) {
            Some(token) => Some(here.with_page(token)),
            None => here.after_window(self.floor_ms, self.now_ms),
        };
        Ok(FetchPage {
            events,
            cursor: next.as_ref().map(Cursor::encode),
            done: next.is_none(),
        })
    }
}

impl<F: Fills + Sync> Source for BybitSource<F> {
    fn id(&self) -> &'static str {
        SOURCE_ID
    }

    /// Only the pseudo-network an exchange balance lives on.
    fn supports(&self, chain: Chain) -> bool {
        chain == EXCHANGE_CHAIN
    }

    async fn fetch(&self, request: FetchRequest<'_>) -> Result<FetchPage> {
        self.fetch_page(request.cursor, request.limit).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;
    use crate::model::cryptotrack::normalize_address;

    const ACCOUNT: &str = "bybit:wallet-1";

    fn execution(exec_id: &str, side: &str, qty: &str, value: &str, price: &str) -> Execution {
        Execution {
            exec_id: exec_id.into(),
            symbol: "BTCUSDT".into(),
            side: side.into(),
            order_type: "Market".into(),
            exec_price: price.into(),
            exec_qty: qty.into(),
            exec_value: value.into(),
            exec_fee: "0.5".into(),
            exec_time: "1700000000000".into(),
            is_maker: false,
            closed_size: None,
            category: "spot".into(),
        }
    }

    fn pending(exec_id: &str, side: &str) -> PendingImport {
        PendingImport::from_execution(
            ACCOUNT,
            "spot",
            &execution(exec_id, side, "0.5", "32500", "65000"),
        )
        .expect("a readable fill")
    }

    #[test]
    fn an_event_id_carries_the_source_and_nothing_from_a_clock() {
        let event = event_from_pending(ACCOUNT, &pending("exec-1", "Buy")).unwrap();
        assert_eq!(event.id, "cev:bybit:wallet-1:bybit:exec-1");
        // The same facts give the same id, however often they are converted.
        assert_eq!(
            event.id,
            event_from_pending(ACCOUNT, &pending("exec-1", "Buy"))
                .unwrap()
                .id
        );
        // `exec_id` is only unique inside Bybit, so the exchange is part of it.
        assert!(event.id.contains(":bybit:"));
        assert_eq!(exec_id_of(&event), Some("exec-1"));
        // And two accounts never collide on one fill.
        assert_ne!(
            event.id,
            event_from_pending("bybit:wallet-2", &pending("exec-1", "Buy"))
                .unwrap()
                .id
        );
    }

    #[test]
    fn a_fill_is_a_swap_of_two_legs_with_its_fee() {
        let event = event_from_pending(ACCOUNT, &pending("exec-1", "Buy")).unwrap();
        assert_eq!(event.kind, CryptoTxKind::Swap);
        assert_eq!(event.chain, EXCHANGE_CHAIN);
        assert_eq!(event.time, "2023-11-14T22:13:20Z");
        let [sold, bought] = &event.legs[..] else {
            panic!("two legs: {:?}", event.legs)
        };
        assert_eq!(sold.direction, CryptoDirection::Out);
        assert_eq!(sold.asset, exchange_asset("USDT"));
        assert_eq!(sold.amount, Amount::parse("32500").unwrap());
        assert_eq!(bought.direction, CryptoDirection::In);
        assert_eq!(bought.asset, exchange_asset("BTC"));
        assert_eq!(bought.amount, Amount::parse("0.5").unwrap());
        // The exchange's fee is the one fee a source does report, and it has
        // to leave the holding.
        let fee = event.gas.as_ref().expect("the fee is kept");
        assert_eq!(fee.asset, exchange_asset("USDT"));
        assert_eq!(fee.amount, Amount::parse("0.5").unwrap());
    }

    #[test]
    fn an_exchange_coin_is_one_asset_whatever_the_case() {
        assert_eq!(exchange_asset(" usdt "), exchange_asset("USDT"));
        assert_eq!(exchange_asset("USDT").key(), "fiat|USDT");
        assert_eq!(asset_ticker(&exchange_asset("btc")), Some("BTC"));
        assert_eq!(
            asset_ticker(&CryptoAsset::Native {
                chain: Chain::Ethereum
            }),
            None
        );
        assert!(is_usd_coin("usdt") && is_usd_coin("USDC"));
        assert!(!is_usd_coin("BTC"));
    }

    #[test]
    fn a_move_between_the_exchange_accounts_is_its_own_counterparty() {
        let row = InterTransfer {
            transfer_id: "t-1".into(),
            coin: "USDT".into(),
            amount: "250".into(),
            from_account_type: "FUND".into(),
            to_account_type: "UNIFIED".into(),
            timestamp: "1700000000000".into(),
            status: "SUCCESS".into(),
        };
        let imp = PendingImport::from_transfer(ACCOUNT, &row).unwrap();
        let event = event_from_pending(ACCOUNT, &imp).unwrap();
        assert_eq!(event.kind, CryptoTxKind::Transfer);
        assert_eq!(event.from, "FUND");
        assert_eq!(event.to, "UNIFIED");
        // The fold treats a leg whose counterparty is a followed address as a
        // move, so the cost pool and the profit are untouched; the account's
        // address is its handle, normalized the same way.
        for leg in &event.legs {
            assert_eq!(leg.counterparty.as_deref(), Some(ACCOUNT));
            assert_eq!(
                normalize_address(EXCHANGE_CHAIN, ACCOUNT),
                account_handle("wallet-1")
            );
        }
        assert_eq!(event.id, "cev:bybit:wallet-1:bybit:transfer:t-1");
    }

    #[test]
    fn an_earn_payout_is_income_and_not_a_purchase() {
        let row = EarnYield {
            id: "y-1".into(),
            coin: "USDT".into(),
            product_id: "p".into(),
            amount: "0.42".into(),
            created_at: "1700000000000".into(),
        };
        let imp = PendingImport::from_earn_yield(ACCOUNT, &row).unwrap();
        let event = event_from_pending(ACCOUNT, &imp).unwrap();
        assert_eq!(event.kind, CryptoTxKind::Claim);
        assert_eq!(event.to, ACCOUNT_EARN);
        assert_eq!(event.legs.len(), 1);
        assert_eq!(event.legs[0].direction, CryptoDirection::In);
        assert_eq!(event.id, "cev:bybit:wallet-1:bybit:earn:y-1");
    }

    #[test]
    fn a_record_that_names_nothing_is_not_an_operation() {
        let mut imp = pending("exec-1", "Buy");
        imp.exec_id = String::new();
        assert!(event_from_pending(ACCOUNT, &imp).is_none());
        let mut imp = pending("exec-1", "Buy");
        imp.occurred_at = String::new();
        assert!(event_from_pending(ACCOUNT, &imp).is_none());
        let mut imp = pending("exec-1", "Buy");
        imp.qty_bought = Amount::ZERO;
        assert!(event_from_pending(ACCOUNT, &imp).is_none());
        assert!(event_from_pending("", &pending("exec-1", "Buy")).is_none());
    }

    #[test]
    fn only_a_dollar_quote_states_a_dollar_price() {
        let buy = implied_usd(&pending("exec-1", "Buy"));
        assert_eq!(
            buy,
            vec![
                (exchange_asset("USDT"), Amount::parse("1").unwrap()),
                (exchange_asset("BTC"), Amount::parse("65000").unwrap()),
            ]
        );
        // A sell states the same two prices: which leg is the base does not
        // depend on the direction.
        let sell = implied_usd(&pending("exec-2", "Sell"));
        assert!(sell.contains(&(exchange_asset("BTC"), Amount::parse("65000").unwrap())));
        // `ETHBTC` says nothing about dollars, and nothing is invented.
        let mut crossed = pending("exec-3", "Buy");
        crossed.asset_bought = "ETH".into();
        crossed.asset_sold = "BTC".into();
        assert!(implied_usd(&crossed).is_empty());
    }

    #[test]
    fn a_cursor_round_trips_and_a_bad_one_is_refused() {
        let cursor = Cursor::start(2_000_000).with_page("abc-1".into());
        assert_eq!(Cursor::decode(Some(&cursor.encode()), 0).unwrap(), cursor);
        assert_eq!(Cursor::decode(None, 7).unwrap(), Cursor::start(7));
        assert_eq!(Cursor::decode(Some("  "), 7).unwrap(), Cursor::start(7));
        for text in [
            "{}",
            "not json",
            r#"{"v":2,"c":"spot","e":1}"#,
            r#"{"v":1,"c":"nope","e":1}"#,
            r#"{"v":1,"c":"spot","e":1,"p":"a&b=c"}"#,
            r#"{"v":1,"c":"spot","e":1,"p":""}"#,
        ] {
            assert!(Cursor::decode(Some(text), 0).is_err(), "{text}");
        }
        assert!(Cursor::decode(Some(&"x".repeat(2000)), 0).is_err());
    }

    #[test]
    fn the_walk_goes_window_by_window_then_product_by_product() {
        let now = 10 * WINDOW_MS;
        let floor = now - 2 * WINDOW_MS;
        let first = Cursor::start(now);
        assert_eq!(first.window_start_ms(floor), now - WINDOW_MS);
        let second = first.after_window(floor, now).unwrap();
        assert_eq!(second.product, Category::Spot);
        assert_eq!(second.window_end_ms, now - WINDOW_MS - 1);
        assert_eq!(second.page, None);
        // The second window reaches the floor, so the next step is the next
        // product, starting again at "now".
        let third = second.after_window(floor, now).unwrap();
        assert_eq!(third.product, Category::Linear);
        assert_eq!(third.window_end_ms, now);
        let fourth = third.after_window(now, now).unwrap();
        assert_eq!(fourth.product, Category::Inverse);
        // And after the last product the walk is over.
        assert!(fourth.after_window(now, now).is_none());
    }

    #[test]
    fn a_failure_is_retried_only_while_retrying_can_help() {
        assert!(retryable(&Error::Network("timeout".into())));
        assert!(!retryable(&Error::ExchangeKeyRejected("10003".into())));
        assert!(!retryable(&Error::InvalidInput("nonsense".into())));
        // Doubling, and the wait never grows past the cap however many
        // attempts a caller allows.
        assert_eq!(backoff_ms(1), 500);
        assert_eq!(backoff_ms(2), 1000);
        assert_eq!(backoff_ms(3), 2000);
        assert_eq!(backoff_ms(MAX_ATTEMPTS), 2000);
        assert_eq!(backoff_ms(0), 500);
    }

    /// A transport that fails a set number of times, then answers; it counts
    /// its calls and remembers what it was asked, so the walk and the retries
    /// are visible with no network and no key.
    /// What one page was asked for: product, window and page token.
    type Asked = (Category, u64, u64, Option<String>);

    struct Flaky {
        fail: AtomicU32,
        calls: AtomicU32,
        asked: std::sync::Mutex<Vec<Asked>>,
        error: fn() -> Error,
        page: FillsPage,
    }

    impl Fills for Flaky {
        async fn page(&self, request: PageRequest<'_>) -> Result<FillsPage> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.asked.lock().unwrap().push((
                request.product,
                request.start_ms,
                request.end_ms,
                request.page.map(str::to_owned),
            ));
            if self.fail.load(Ordering::Relaxed) > 0 {
                self.fail.fetch_sub(1, Ordering::Relaxed);
                return Err((self.error)());
            }
            Ok(self.page.clone())
        }

        async fn transfers(&self, _limit: usize) -> Result<Vec<InterTransfer>> {
            Ok(Vec::new())
        }

        async fn payouts(&self, _limit: usize) -> Result<Vec<EarnYield>> {
            Ok(Vec::new())
        }
    }

    fn network() -> Error {
        Error::Network("timeout".into())
    }

    fn flaky(fail: u32, error: fn() -> Error) -> Flaky {
        Flaky {
            fail: AtomicU32::new(fail),
            calls: AtomicU32::new(0),
            asked: std::sync::Mutex::new(Vec::new()),
            error,
            page: FillsPage {
                fills: vec![execution("exec-1", "Buy", "0.5", "32500", "65000")],
                next_page: None,
            },
        }
    }

    #[tokio::test]
    async fn a_page_survives_a_network_failure_and_gives_up_on_a_refused_key() {
        let source = BybitSource::new(flaky(2, network), ACCOUNT, WINDOW_MS, 0).with_backoff(|_| 0);
        let page = source.fetch_page(None, 50).await.unwrap();
        assert_eq!(
            source.fills.calls.load(Ordering::Relaxed),
            3,
            "two failures, then the answer"
        );
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].id, "cev:bybit:wallet-1:bybit:exec-1");

        // A key the exchange refuses is not retried: it would be refused again.
        let refused = BybitSource::new(
            flaky(1, || Error::ExchangeKeyRejected("10003".into())),
            ACCOUNT,
            WINDOW_MS,
            0,
        )
        .with_backoff(|_| 0);
        let err = refused.fetch_page(None, 50).await.unwrap_err();
        assert_eq!(err.code(), "EXCHANGE_KEY_REJECTED");
        assert_eq!(refused.fills.calls.load(Ordering::Relaxed), 1);

        // And a failure that never clears runs out of attempts rather than
        // running forever.
        let dead =
            BybitSource::new(flaky(u32::MAX, network), ACCOUNT, WINDOW_MS, 0).with_backoff(|_| 0);
        assert!(dead.fetch_page(None, 50).await.is_err());
        assert_eq!(dead.fills.calls.load(Ordering::Relaxed), MAX_ATTEMPTS);
    }

    #[tokio::test]
    async fn a_resumed_pass_asks_for_the_page_it_stopped_on() {
        let source =
            BybitSource::new(flaky(0, network), ACCOUNT, 4 * WINDOW_MS, 0).with_backoff(|_| 0);
        let cursor = Cursor::start(4 * WINDOW_MS).with_page("page-2".into());
        source.fetch_page(Some(&cursor.encode()), 50).await.unwrap();
        let asked = source.fills.asked.lock().unwrap().clone();
        assert_eq!(
            asked,
            vec![(
                Category::Spot,
                3 * WINDOW_MS,
                4 * WINDOW_MS,
                Some("page-2".into())
            )]
        );
    }

    #[tokio::test]
    async fn the_same_fills_give_the_same_events_whenever_they_are_read() {
        let morning = BybitSource::new(flaky(0, network), ACCOUNT, 1, 0).with_backoff(|_| 0);
        let evening =
            BybitSource::new(flaky(0, network), ACCOUNT, 99 * WINDOW_MS, 0).with_backoff(|_| 0);
        let left = morning.fetch_page(None, 50).await.unwrap();
        let right = evening.fetch_page(None, 50).await.unwrap();
        assert_eq!(left.events, right.events);
        assert_eq!(morning.id(), SOURCE_ID);
        assert!(morning.supports(EXCHANGE_CHAIN));
        assert!(!morning.supports(Chain::Ethereum));
    }
}
