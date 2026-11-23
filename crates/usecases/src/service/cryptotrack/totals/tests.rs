use crate::Amount;
use crate::cryptotrack::service::totals::CryptoLeftOut;
use crate::model::chain::Chain;
use crate::model::cryptotrack::CryptoAsset;
use crate::model::cryptotrack::CryptoSnapshot;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoDirection, CryptoLeg, CryptoPosition, CryptoPricePoint, CryptoToken,
    CryptoTx, CryptoTxKind, TokenId, TokenVerdict, TrackingRef,
};
use crate::model::store::StoreScope;
use crate::model::store::{AssetRef, StoreKind};
use crate::model::transaction::Transaction;
use crate::model::vault::PricePoint;
use crate::service::test_support::*;
use crate::service::*;
use crate::time::unix_secs;
use mwt_reports::overview::{NetworthPoint, Report, ReportFilter, ReportKind};

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
fn followed(vault: &mut Session, account_id: &str, name: &str, address: &str) -> String {
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

fn archive(vault: &mut Session, store_id: &str) {
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
fn priced(vault: &mut Session, asset: &CryptoAsset, usd: &str) {
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
fn quote(vault: &mut Session, asset: &AssetRef, usd: &str) {
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&bank, "1000", "salary"),
    )
    .expect("the top-up is booked");
    quote_usd(&mut vault);
    let holdings = serde_json::to_vec(&vault.doc.holdings).expect("holdings serialize");
    let worth = serde_json::to_vec(&crate::usecases::money::capital::net_worth(&vault.books()))
        .expect("net worth serializes");

    crate::usecases::cryptotrack::recompute::apply_crypto_holdings(&mut vault);

    assert_eq!(
        serde_json::to_vec(&vault.doc.holdings).expect("holdings serialize"),
        holdings,
        "the module must not touch a holding it did not make"
    );
    assert_eq!(
        serde_json::to_vec(&crate::usecases::money::capital::net_worth(&vault.books()))
            .expect("net worth serializes"),
        worth
    );
    assert!(crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books()).is_empty());
}

#[test]
fn the_module_adds_its_wallet_and_moves_no_other_total() {
    let (_dir, mut vault) = empty_vault();
    let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income(&bank, "1000", "salary"),
    )
    .expect("the top-up is booked");
    quote_usd(&mut vault);
    quote(&mut vault, &eth_ref(), "2000");
    let before = crate::usecases::money::capital::net_worth(&vault.books());

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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    let after = crate::usecases::money::capital::net_worth(&vault.books());
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
    assert!(crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books()).is_empty());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    let before = crate::usecases::money::capital::net_worth(&vault.books());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    let after = crate::usecases::money::capital::net_worth(&vault.books());
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&old, eth_ref(), "2", "2000", "bought"),
    )
    .expect("the purchase is booked");
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("4000")
    );

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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&old, eth_ref(), "2", "2000", "bought"),
    )
    .expect("the purchase is booked");
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&old, doge_ref(), "1000", "1", "bought"),
    )
    .expect("the second purchase is booked");
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("5000")
    );

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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&old, eth_ref(), "2", "2000", "bought"),
    )
    .expect("the purchase is booked");
    archive(&mut vault, &old);
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert!(vault.doc.crypto_accounts.is_empty());
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        income_of(&other, eth_ref(), "2", "2000", "bought"),
    )
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("8000")
    );
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        Amount::ZERO
    );
    assert_eq!(holding_of(&vault, &wallet, &pretender), Amount::ZERO);
    let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
    let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
    assert!(left.is_empty(), "a priced leg is not unpriced: {left:?}");
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("4000")
    );
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
    let usdc = crate::cryptotrack::service::totals::crypto_asset_ref(
        &vault.doc,
        &CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC)),
    );
    quote(&mut vault, &usdc, "1");
    followed(&mut vault, "cacc-1", "Cold", ME);
    vault.doc.crypto_positions.push(aave_with_debt());
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    let usdc_key = CryptoAsset::Token(TokenId::new(Chain::Ethereum, USDC)).key();
    let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        Amount::ZERO
    );
    assert_eq!(holding_of(&vault, &wallet, &pretender), Amount::ZERO);
    let shown = crate::usecases::cryptotrack::positions::crypto_list_positions(
        &vault.books(),
        &[],
        "2026-03-06T00:00:00Z",
    );
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].value_usd, None, "unknown, not zero and not 1e9");
    assert_eq!(shown[0].assets[0].value_usd, None);

    // Once the user takes the token in, the leg counts like any other.
    crate::usecases::cryptotrack::tokens::crypto_confirm_token(&mut vault, &token)
        .expect("confirmed");
    quote(&mut vault, &pretender, "1");
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("1000000000")
    );
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    let held = vault
        .doc
        .holdings
        .iter()
        .find(|have| have.store_id == wallet)
        .expect("the coins are held even unpriced");
    assert_eq!(held.quantity, amount("100"));
    assert_eq!(held.avg_cost, None, "no price known is not a zero cost");
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        Amount::ZERO,
        "nothing values it, so it adds nothing to the capital"
    );
    let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);

    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        Amount::ZERO
    );
    let left = crate::cryptotrack::service::totals::crypto_outside_totals(&vault.books());
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
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));

    vault.doc.crypto_accounts[0].archived = true;
    crate::usecases::money::portfolio::recompute_holdings(&mut vault);
    assert_eq!(holding_of(&vault, &wallet, &eth_ref()), Amount::ZERO);
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        Amount::ZERO
    );
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

    crate::usecases::cryptotrack::recompute::crypto_sync_holdings(&mut vault)
        .expect("the holdings are saved");

    assert_eq!(holding_of(&vault, &wallet, &eth_ref()), amount("2"));
    assert_eq!(
        crate::usecases::money::capital::net_worth(&vault.books()).financial,
        amount("4000")
    );
}

// -- the capital history: the months a replay cannot reach ---------------

/// The clock every history test reads the report at.
const NOW: &str = "2026-06-15T12:00:00Z";

fn dated(mut tx: Transaction, stamp: &str) -> Transaction {
    at(&mut tx, stamp);
    tx
}

/// A vault whose capital is counted in dollars, like the reports tests.
fn usd_vault() -> (tempfile::TempDir, Session) {
    let (dir, mut vault) = empty_vault();
    vault.set_base_currency("usd");
    quote_usd(&mut vault);
    (dir, vault)
}

/// A reading the module wrote down that day (spec E5 writes one per
/// account on every read, with the day in the id).
fn recorded(vault: &mut Session, account_id: &str, moment: &str, usd: &str) {
    vault.doc.crypto_snapshots.push(CryptoSnapshot {
        id: crate::cryptotrack::engine::ids::snapshot(Some(account_id), moment),
        at: moment.to_string(),
        account_id: Some(account_id.to_string()),
        value_usd: amount(usd),
    });
}

fn points_of(vault: &Session, filter: &ReportFilter) -> Vec<NetworthPoint> {
    let now = unix_secs(NOW).expect("the clock parses");
    match crate::reports::overview::report(&vault.books(), ReportKind::Networth, filter, now, "UTC")
        .expect("the report runs")
    {
        Report::Networth(report) => report.points,
        _ => panic!("another report came back"),
    }
}

fn history(vault: &Session, from: &str, to: &str) -> Vec<NetworthPoint> {
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
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            dated(income(&bank, sum, "salary"), stamp),
        )
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
        serde_json::to_vec(&history(&vault, "2026-03", "2026-05")).expect("the points serialize"),
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        dated(
            income_of(&old, eth_ref(), "2", "2000", "bought"),
            "2026-03-02T10:00:00Z",
        ),
    )
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
    crate::usecases::money::transactions::add_transaction(
        &mut vault,
        dated(income(&bank, "1000", "salary"), "2026-03-02T10:00:00Z"),
    )
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
