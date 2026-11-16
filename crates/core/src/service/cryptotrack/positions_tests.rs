//! What a DeFi position is worth and what it earned (spec D0).
//!
//! The reference numbers of the pool example are the ones worked out by hand
//! in `cryptotrack::protocols::fees`; here they travel the whole way — stored
//! operations, stored legs, prices — and come out the same.

use super::*;
use crate::cryptotrack::protocols::{FeeReading, PositionKey, PositionKind, ProtocolReview};
use crate::model::chain::Chain;
use crate::model::cryptotrack::{
    CryptoAccount, CryptoFee, CryptoPricePoint, CryptoToken, TokenId, TokenVerdict, TrackingRef,
};
use crate::model::store::StoreKind;
use crate::model::vault::PricePoint;
use crate::service::cryptotrack::totals::CryptoLeftOut;
use crate::service::test_support::{amount, empty_vault, store};

const ME: &str = "0xabcd00000000000000000000000000000000ef01";
const OUTSIDE: &str = "0x2222222222222222222222222222222222222222";
const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
/// Aave's receipt for a USDC supply: a token the wallet really holds.
const AUSDC: &str = "0x98c23e9d8f34fefb1b7bd6a91b7ff122d4aa2f93";
const ACCOUNT: &str = "cacc-1";
/// 73 days after the deposit of every example: a fifth of a 365-day year.
const LATER: &str = "2026-05-13T10:00:00Z";

fn ether() -> CryptoAsset {
    CryptoAsset::Native {
        chain: Chain::Ethereum,
    }
}

fn token(contract: &str) -> CryptoAsset {
    CryptoAsset::Token(TokenId::new(Chain::Ethereum, contract))
}

fn leg(direction: CryptoDirection, asset: CryptoAsset, quantity: &str) -> CryptoLeg {
    CryptoLeg {
        direction,
        asset,
        amount: amount(quantity),
        counterparty: None,
    }
}

/// A followed address with the wallet that shows it (decision P5).
fn followed() -> (tempfile::TempDir, UnlockedVault, String) {
    let (dir, mut vault) = empty_vault();
    vault.doc.crypto_accounts.push(CryptoAccount::new(
        ACCOUNT,
        "Cold EVM",
        Chain::Ethereum,
        ME,
        "2026-02-01T00:00:00Z",
    ));
    let wallet = store(&mut vault, "Cold EVM", StoreKind::Crypto, None);
    vault
        .doc
        .stores
        .iter_mut()
        .find(|have| have.id == wallet)
        .expect("the wallet was created")
        .crypto_account_id = Some(ACCOUNT.to_string());
    (dir, vault, wallet)
}

/// A token the module knows and counts (spec E2), so nothing here is left out
/// of a total for being in quarantine.
fn known(vault: &mut UnlockedVault, contract: &str, symbol: &str) {
    vault.doc.crypto_tokens.push(CryptoToken {
        verdict: Some(TokenVerdict::Confirmed),
        verdict_at: Some("2026-02-01T00:00:00Z".into()),
        coingecko_id: Some(symbol.to_lowercase()),
        ..CryptoToken::new(TokenId::new(Chain::Ethereum, contract), symbol, 6)
    });
}

/// What the module knows a unit cost on every day of March 2026 (spec E4):
/// the prices "at the deposit".
fn priced_then(vault: &mut UnlockedVault, asset: &CryptoAsset, usd: &str) {
    for day in 1..=31 {
        vault.doc.crypto_prices.push(CryptoPricePoint {
            token_key: asset.key(),
            value_usd: amount(usd),
            ts: format!("2026-03-{day:02}T00:00:00Z"),
            source: "test".into(),
        });
    }
}

/// The ordinary price cache: what every holding, and so every position leg,
/// is valued at now.
fn priced_now(vault: &mut UnlockedVault, asset: &CryptoAsset, usd: &str) {
    let key = vault.crypto_asset_ref(asset).key();
    vault.doc.price_cache.push(PricePoint {
        asset_key: key,
        value_usd: amount(usd),
        ts: "2026-05-13T00:00:00Z".into(),
        source: "test".into(),
        change_24h_pct: None,
    });
}

/// One operation of a position, as its adapter would have reported it.
fn operation(
    event_id: &str,
    key: &PositionKey,
    day: u32,
    kind: CryptoTxKind,
    legs: Vec<CryptoLeg>,
    gas_ether: Option<&str>,
) -> CryptoTx {
    CryptoTx {
        id: event_id.to_string(),
        chain: key.chain,
        account_id: ACCOUNT.to_string(),
        time: format!("2026-03-{day:02}T10:00:00Z"),
        block: Some(21_000_000),
        from: ME.into(),
        to: ME.into(),
        legs,
        gas: gas_ether.map(|quantity| CryptoFee {
            asset: ether(),
            amount: amount(quantity),
        }),
        protocol: Some(key.protocol.clone()),
        method: None,
        kind,
        raw: None,
        tracking: key.tracking(),
    }
}

/// Coins arriving at the followed address from outside: what the user had
/// before they supplied any of it to a protocol.
fn arrival(event_id: &str, day: u32, asset: CryptoAsset, quantity: &str) -> CryptoTx {
    CryptoTx {
        id: event_id.to_string(),
        chain: Chain::Ethereum,
        account_id: ACCOUNT.to_string(),
        time: format!("2026-03-{day:02}T09:00:00Z"),
        block: Some(21_000_000),
        from: OUTSIDE.into(),
        to: ME.into(),
        legs: vec![CryptoLeg {
            direction: CryptoDirection::In,
            asset,
            amount: amount(quantity),
            counterparty: Some(OUTSIDE.to_string()),
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

fn reading(
    key: &PositionKey,
    kind: PositionKind,
    legs: Vec<CryptoLeg>,
    fees: FeeReading,
) -> PositionReading {
    PositionReading {
        key: key.clone(),
        account_id: ACCOUNT.to_string(),
        name: "Test position".into(),
        kind,
        status: PositionStatus::Open,
        legs,
        fees,
        receipts: Vec::new(),
        approximate: false,
        opened_at: None,
        closed_at: None,
        read_at: "2026-05-13T09:00:00Z".into(),
    }
}

fn only(vault: &UnlockedVault, live: &[PositionReading]) -> PositionView {
    let rows = vault.crypto_list_positions(live, LATER);
    assert_eq!(rows.len(), 1, "exactly one position was stored");
    rows.into_iter().next().expect("the one row")
}

// -- a one-sided position -------------------------------------------------------

#[test]
fn a_supply_reports_what_it_cost_what_it_accrued_and_its_rate() {
    // Reference example, worked out by hand.
    //
    // 1000 USDC supplied on 2026-03-01 at 1.00 USD, gas 0.001 ETH at
    // 2000 USD = 2.00 USD. The balance has grown to 1050 USDC by 2026-05-13
    // (73 days = a fifth of a 365-day year); USDC is still 1.00 USD.
    //
    //   invested  = 1000 * 1.00                = 1000.00
    //   value     = 1050 * 1.00                = 1050.00
    //   hold      = 1000 * 1.00                = 1000.00
    //   accrued   = 1050.00 - 1000.00          =   50.00   (informational)
    //   gas       = 0.001 * 2000               =    2.00
    //   net       = (1050 - 1000) + 0 + 0 - 2  =   48.00
    //   apr       = 48 / 1000 * 365 / 73       =    0.24
    let (_dir, mut vault, wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_then(&mut vault, &token(USDC), "1");
    priced_then(&mut vault, &ether(), "2000");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &ether(), "2000");

    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
    let deposit = operation(
        "ev-stake",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![
            leg(CryptoDirection::Out, token(USDC), "1000"),
            leg(CryptoDirection::In, token(AUSDC), "1000"),
        ],
        Some("0.001"),
    );
    let live = reading(
        &key,
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, token(USDC), "1050")],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[deposit], &[])
        .expect("the reading is stored");

    let row = only(&vault, &[live]);
    assert_eq!(row.id, "cacc-1|aave-v3|ethereum|usdc-reserve");
    assert_eq!(row.store_id.as_deref(), Some(wallet.as_str()));
    assert_eq!(row.status, PositionStatus::Open);
    assert_eq!(row.invested_usd, Some(amount("1000")));
    assert_eq!(row.value_usd, Some(amount("1050")));
    assert_eq!(row.accrued_usd, Some(amount("50")));
    assert_eq!(
        row.il_usd, None,
        "a one-sided position has nothing to compare against"
    );
    assert_eq!(row.fees_collected_usd, Some(Amount::ZERO));
    assert_eq!(row.fees_uncollected_usd, Some(Amount::ZERO));
    assert_eq!(row.gas_usd, Some(amount("2")));
    assert_eq!(row.net_usd, Some(amount("48")));
    assert_eq!(row.apr, Some(amount("0.24")));
    assert!(!row.approximate);
    assert_eq!(row.opened_at.as_deref(), Some("2026-03-01T10:00:00Z"));
    assert_eq!(row.tracking, key.tracking());
}

#[test]
fn a_debt_position_is_the_collateral_minus_the_loan_and_has_no_loss() {
    // 10 000 USDC supplied, 1 ETH borrowed at 4000 USD: the position is worth
    // 10 000 - 4000 = 6000. 5000 USDC of it came out of the user's pocket,
    // the rest is the loan's own money, so "invested" is the 5000 the
    // operations show. No gas in this one.
    //   net = 6000 - 5000 + 0 + 0 - 0 = 1000
    let (_dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_then(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &ether(), "4000");

    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-eth");
    let deposit = operation(
        "ev-stake",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![leg(CryptoDirection::Out, token(USDC), "5000")],
        None,
    );
    let live = reading(
        &key,
        PositionKind::Lending,
        vec![
            leg(CryptoDirection::In, token(USDC), "10000"),
            leg(CryptoDirection::Out, ether(), "1"),
        ],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[deposit], &[])
        .expect("the reading is stored");

    let row = only(&vault, &[live]);
    assert_eq!(row.value_usd, Some(amount("6000")));
    assert_eq!(row.invested_usd, Some(amount("5000")));
    assert_eq!(row.net_usd, Some(amount("1000")));
    assert_eq!(row.il_usd, None);
    assert_eq!(row.il_ratio, None);
    let borrowed = row
        .assets
        .iter()
        .find(|asset| asset.direction == CryptoDirection::Out)
        .expect("the debt is a leg of its own");
    assert_eq!(borrowed.value_usd, Some(amount("4000")));
}

// -- a pool ---------------------------------------------------------------------

/// The hand-worked pool of `protocols::fees`, stored and read back.
fn pool_vault() -> (
    tempfile::TempDir,
    UnlockedVault,
    PositionKey,
    PositionReading,
) {
    let (dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_then(&mut vault, &token(USDC), "1");
    priced_then(&mut vault, &ether(), "2000");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &ether(), "4000");

    let key = PositionKey::new("uniswap-v3", Chain::Ethereum, "123456");
    let mint = operation(
        "ev-mint",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![
            leg(CryptoDirection::Out, ether(), "1"),
            leg(CryptoDirection::Out, token(USDC), "2000"),
        ],
        Some("0.005"),
    );
    let collect = operation(
        "ev-collect",
        &key,
        2,
        CryptoTxKind::Claim,
        vec![leg(CryptoDirection::In, token(USDC), "120")],
        Some("0.00425"),
    );
    let live = reading(
        &key,
        PositionKind::Pool,
        vec![
            leg(CryptoDirection::In, ether(), "0.7071067811865475"),
            leg(CryptoDirection::In, token(USDC), "2828.4271247461902"),
        ],
        FeeReading {
            collected: Some(Vec::new()),
            uncollected: Some(vec![AssetAmount {
                asset: token(USDC),
                amount: amount("30"),
            }]),
        },
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[mint, collect], &[])
        .expect("the reading is stored");
    (dir, vault, key, live)
}

#[test]
fn a_pool_shows_the_loss_against_holding_and_never_sums_it_twice() {
    // Reference example, worked out by hand (the same as
    // `protocols::fees::tests::impermanent_loss_matches_the_hand_worked_two_times_pool`
    // and `the_net_result_counts_impermanent_loss_only_once`).
    //
    // 1 ETH + 2000 USDC into a 50/50 pool at ETH = 2000, so 4000 USD in and
    // k = 2000. ETH doubles to 4000.
    //   hold  = 1 * 4000 + 2000 * 1                   = 6000.0000
    //   pool  = 0.7071067811865475 * 4000
    //           + 2828.4271247461902 * 1              = 5656.8542494923802
    //   IL    = 5656.8542494923802 - 6000             = -343.1457505076198
    //   ratio = -343.1457505076198 / 6000             = -0.0571909584...
    //   gas   = (0.005 + 0.00425) * 2000              =   18.5000
    //   net   = (5656.8542494923802 - 4000)
    //           + 120 + 30 - 18.5                     = 1788.3542494923802
    //   apr   = 1788.3542494923802 / 4000 * 365 / 73  =    2.2354428118654752
    let (_dir, vault, key, live) = pool_vault();
    let row = only(&vault, &[live]);
    assert_eq!(row.kind, PositionKind::Pool);
    assert_eq!(row.tracking, key.tracking());
    assert_eq!(row.invested_usd, Some(amount("4000")));
    assert_eq!(
        row.value_usd.map(|value| value.round(4)),
        Some(amount("5656.8542"))
    );
    assert_eq!(
        row.il_usd.map(|loss| loss.round(4)),
        Some(amount("-343.1458"))
    );
    assert_eq!(
        row.il_ratio.map(|ratio| ratio.round(6)),
        Some(amount("-0.057191"))
    );
    assert_eq!(
        row.accrued_usd, None,
        "a pool accrues through its fees, not through its balance"
    );
    assert_eq!(row.fees_collected_usd, Some(amount("120")));
    assert_eq!(row.fees_uncollected_usd, Some(amount("30")));
    assert_eq!(row.gas_usd, Some(amount("18.5")));
    let net = row.net_usd.expect("every part of the net is known");
    assert_eq!(net.round(4), amount("1788.3542"));
    // The loss is inside the value change already: adding it again would give
    // 1788.3542494923802 - 343.1457505076198 = 1445.2084989847604, which is
    // the mistake this assertion exists to catch.
    let loss = row.il_usd.expect("a loss");
    assert_eq!((net + loss).round(4), amount("1445.2085"));
    assert_ne!(
        net.round(4),
        (net + loss).round(4),
        "the loss must not be an addend of the net result"
    );
    assert_eq!(row.apr.map(|rate| rate.round(6)), Some(amount("2.235443")));
}

#[test]
fn a_pools_uncollected_fees_are_absent_without_a_reading_of_this_device() {
    // A device that has only synced has the legs and the operations, so it
    // can say what the position is worth and what it cost — but the fees it
    // still holds are a reading of a moment nobody took here. A dash, not a
    // zero, and so no net result either.
    let (_dir, vault, _key, _live) = pool_vault();
    let row = only(&vault, &[]);
    assert_eq!(row.invested_usd, Some(amount("4000")));
    assert_eq!(row.fees_collected_usd, Some(amount("120")));
    assert_eq!(row.fees_uncollected_usd, None);
    assert_eq!(row.net_usd, None);
    assert_eq!(row.apr, None);
    assert!(
        !row.approximate,
        "approximation is the adapter's word, and there is no reading to carry it"
    );
}

#[test]
fn the_approximate_flag_comes_from_the_adapter_and_from_nowhere_else() {
    let (_dir, vault, _key, live) = pool_vault();
    assert!(!only(&vault, std::slice::from_ref(&live)).approximate);
    let hooked = PositionReading {
        approximate: true,
        ..live
    };
    assert!(only(&vault, &[hooked]).approximate);
}

// -- absent, not zero -----------------------------------------------------------

#[test]
fn a_position_whose_token_has_no_price_shows_a_dash_and_not_a_zero() {
    let (_dir, mut vault, _wallet) = followed();
    // Not a stablecoin, so no dollar peg stands in for the missing price,
    // and no price anywhere: neither the ordinary cache nor the module's own
    // history.
    known(&mut vault, USDC, "FOO");
    let key = PositionKey::new("aave-v3", Chain::Ethereum, "foo-reserve");
    let deposit = operation(
        "ev-stake",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![leg(CryptoDirection::Out, token(USDC), "1000")],
        None,
    );
    let live = reading(
        &key,
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, token(USDC), "1000")],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[deposit], &[])
        .expect("the reading is stored");

    let row = only(&vault, &[live]);
    assert_eq!(row.value_usd, None);
    assert_eq!(row.invested_usd, None);
    assert_eq!(row.net_usd, None);
    assert_eq!(row.apr, None);
    assert_eq!(row.accrued_usd, None);
    // The quantity is still exact: it is the money that is unknown.
    assert_eq!(row.assets.len(), 1);
    assert_eq!(row.assets[0].symbol, "FOO");
    assert_eq!(row.assets[0].quantity, amount("1000"));
    assert_eq!(row.assets[0].value_usd, None);
    // And a fee side that really is empty stays a zero next to all of it.
    assert_eq!(row.fees_collected_usd, Some(Amount::ZERO));
}

#[test]
fn a_position_without_a_single_stored_operation_shows_a_dash_and_not_a_zero() {
    // An adapter can find a position on chain without being able to say how
    // it was entered: Lido reads the balance, the deposits are not in its
    // reach (spec D4). Nothing stored means the history is *unknown*, not
    // empty — a zero for "invested" would turn the whole balance into profit.
    let (_dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_now(&mut vault, &token(USDC), "1");
    let key = PositionKey::new("lido", Chain::Ethereum, "steth");
    let live = reading(
        &key,
        PositionKind::Staking,
        vec![leg(CryptoDirection::In, token(USDC), "1000")],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");

    let row = only(&vault, &[live]);
    // What is on chain is known exactly.
    assert_eq!(row.value_usd, Some(amount("1000")));
    // Everything that would have come from the operations is a dash.
    assert_eq!(
        row.invested_usd, None,
        "with no operations stored, what went in is unknown, not zero"
    );
    assert_eq!(row.put_in_usd, None);
    assert_eq!(row.fees_collected_usd, None);
    assert_eq!(row.gas_usd, None);
    assert_eq!(
        row.net_usd, None,
        "a net result against an unknown stake would be the whole balance"
    );
    assert_eq!(
        row.accrued_usd, None,
        "the balance to compare against is unknown: a zero would claim the whole stake as reward"
    );
    assert_eq!(row.il_usd, None);
    assert_eq!(row.apr, None);
}

#[test]
fn a_pool_without_a_stored_history_shows_a_dash_for_the_loss_against_holding() {
    // The same pool as the hand-worked example, read on chain with nothing
    // stored about how it was entered. The deposit is unknown, so holding it
    // instead is not a figure: a zero base would report the position's whole
    // value as a loss against holding on every pool this adapter finds.
    let (_dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &ether(), "4000");
    let key = PositionKey::new("uniswap-v3", Chain::Ethereum, "654321");
    let live = reading(
        &key,
        PositionKind::Pool,
        vec![
            leg(CryptoDirection::In, ether(), "0.7071067811865475"),
            leg(CryptoDirection::In, token(USDC), "2828.4271247461902"),
        ],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");

    let row = only(&vault, &[live]);
    assert_eq!(
        row.value_usd.map(|value| value.round(4)),
        Some(amount("5656.8542"))
    );
    assert_eq!(row.invested_usd, None);
    assert_eq!(
        row.il_usd, None,
        "with no deposit stored there is no holding to compare against"
    );
    assert_eq!(row.il_ratio, None);
    assert_eq!(row.net_usd, None);
}

#[test]
fn a_closed_position_is_frozen_and_adds_nothing_to_the_value() {
    // In on 2026-03-01, out on 2026-03-31: 1000 USDC in at 1.00, 1020 USDC
    // back at 1.00.
    //   invested = 1000 - 1020 = -20   (more came back than went in)
    //   value    = 0                   (a real zero: nothing is left in it)
    //   net      = 0 - (-20) + 0 + 0 - 0 = 20
    // and the rate is measured to the day it closed, not to today.
    let (_dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    priced_then(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &token(USDC), "1");

    let key = PositionKey::new("lido", Chain::Ethereum, "steth");
    let deposit = operation(
        "ev-stake",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![leg(CryptoDirection::Out, token(USDC), "1000")],
        None,
    );
    let exit = operation(
        "ev-unstake",
        &key,
        31,
        CryptoTxKind::Unstake,
        vec![leg(CryptoDirection::In, token(USDC), "1020")],
        None,
    );
    let live = reading(
        &key,
        PositionKind::Staking,
        Vec::new(),
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, &[live], &[deposit, exit], &[])
        .expect("the reading is stored");

    // Read without a live reading: a closed position's numbers must stand on
    // the stored record alone.
    let row = only(&vault, &[]);
    assert_eq!(row.status, PositionStatus::Closed);
    assert_eq!(
        row.value_usd,
        Some(Amount::ZERO),
        "a closed position is worth a real zero and leaves the portfolio"
    );
    assert_eq!(row.invested_usd, Some(amount("-20")));
    assert_eq!(
        row.put_in_usd,
        Some(amount("1000")),
        "what was put in does not fall when the position pays out"
    );
    assert_eq!(row.net_usd, Some(amount("20")));
    assert_eq!(row.closed_at.as_deref(), Some("2026-03-31T10:00:00Z"));
    // Its period ended with it: a year later the rate is the same number.
    let frozen = vault.crypto_list_positions(&[], "2027-05-13T10:00:00Z");
    assert_eq!(
        frozen[0].apr, row.apr,
        "a closed position's rate must not move with the calendar"
    );
    // The rate is measured on what was put in (1 000), not on what is still
    // in it (−20): 20 / 1000 * 365 / 30 = 0.2433…, and above all positive,
    // because the position earned.
    let rate = row.apr.expect("a closed position still has a rate");
    assert!(
        rate.is_positive(),
        "a position that earned must not report a negative rate: {rate}"
    );
    assert_eq!(
        rate.0.round_dp(4),
        amount("0.2433").0,
        "20 on 1 000 held for 30 days"
    );
}

// -- counted once ---------------------------------------------------------------

#[test]
fn a_position_is_not_counted_twice_against_the_receipt_token_on_the_wallet() {
    // The acceptance of spec D0. 1000 USDC arrived at the address and was
    // supplied to Aave; the protocol handed over 1000 aUSDC, which really
    // sits in the wallet and which the node reports, so the reconciliation
    // wrote it down as an arrival (spec E5). Both are confirmed, priced
    // tokens, so without the rule the capital would hold 1000 USD of USDC
    // *and* 1000 USD of aUSDC.
    let (_dir, mut vault, wallet) = followed();
    known(&mut vault, USDC, "USDC");
    known(&mut vault, AUSDC, "AUSDC");
    priced_then(&mut vault, &token(USDC), "1");
    priced_then(&mut vault, &token(AUSDC), "1");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &token(AUSDC), "1");

    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
    let came_in = arrival("ev-in", 1, token(USDC), "1000");
    let deposit = operation(
        "ev-stake",
        &key,
        1,
        CryptoTxKind::Stake,
        vec![
            leg(CryptoDirection::Out, token(USDC), "1000"),
            leg(CryptoDirection::In, token(AUSDC), "1000"),
        ],
        None,
    );
    // What the reconciliation writes for a balance only the node knows: an
    // ordinary arrival at the followed address, under the address itself.
    let mut from_the_node = arrival(
        &ids::reconciled(ACCOUNT, &token(AUSDC).key(), "2026-03-02T00:00:00Z"),
        2,
        token(AUSDC),
        "1000",
    );
    from_the_node.tracking = ids::address_ref(Chain::Ethereum, ME);
    from_the_node.legs[0].counterparty = None;
    vault
        .crypto_apply_positions(ACCOUNT, &[], &[came_in, deposit, from_the_node], &[])
        .expect("the operations are stored");

    // The receipt is recognized from the stored operations alone, so a device
    // that only synced agrees.
    let receipts = vault.crypto_position_receipts();
    assert!(receipts.contains(&token(AUSDC).key()));
    assert!(!receipts.contains(&token(USDC).key()));

    let held: Vec<(String, Amount)> = vault
        .doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == wallet)
        .map(|holding| (holding.asset.symbol.clone(), holding.quantity))
        .collect();
    assert_eq!(
        held,
        vec![("USDC".to_string(), amount("1000"))],
        "the supply is counted once, inside the protocol; the receipt is not a second holding"
    );
    let left_out = vault.crypto_outside_totals();
    let receipt_row = left_out
        .iter()
        .find(|row| row.reason == CryptoLeftOut::InPosition)
        .expect("the receipt is named, not silently dropped");
    assert_eq!(receipt_row.asset_key, token(AUSDC).key());
    assert_eq!(receipt_row.quantity, Some(amount("1000")));
}

#[test]
fn a_position_read_without_a_single_operation_still_counts_in_the_capital() {
    // The other side of the rule above. The receipt is dropped from the
    // capital because the money is counted as the position — and the position
    // reaches the capital through the fold, which only `Stake` and `Unstake`
    // operations fill. Lido reads a balance and emits no operation at all, so
    // without a stand-in the whole stake would leave the capital the moment it
    // was first read: the dashboard would drop by the size of the stake.
    let (_dir, mut vault, wallet) = followed();
    known(&mut vault, USDC, "USDC");
    known(&mut vault, AUSDC, "AUSDC");
    priced_now(&mut vault, &token(USDC), "1");
    priced_now(&mut vault, &token(AUSDC), "1");
    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
    let live = PositionReading {
        receipts: vec![token(AUSDC)],
        ..reading(
            &key,
            PositionKind::Lending,
            vec![leg(CryptoDirection::In, token(USDC), "1000")],
            FeeReading::none_earned(),
        )
    };
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");

    let held: Vec<(String, Amount)> = vault
        .doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == wallet)
        .map(|holding| (holding.asset.symbol.clone(), holding.quantity))
        .collect();
    assert_eq!(
        held,
        vec![("USDC".to_string(), amount("1000"))],
        "what the protocol holds is counted once, from the reading"
    );
    // What went in is unknown without operations, so no cost is invented.
    assert!(
        vault
            .doc
            .holdings
            .iter()
            .all(|holding| holding.avg_cost.is_none())
    );

    // A second reading of the same position replaces it instead of adding to
    // it: the capital follows the chain, it does not accumulate passes.
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored again");
    assert_eq!(
        vault
            .doc
            .holdings
            .iter()
            .filter(|holding| holding.store_id == wallet)
            .count(),
        1
    );

    // A debt leg is not a holding: it is the position's own figure.
    let borrowed = PositionReading {
        receipts: vec![token(AUSDC)],
        ..reading(
            &key,
            PositionKind::Lending,
            vec![
                leg(CryptoDirection::In, token(USDC), "1000"),
                leg(CryptoDirection::Out, token(AUSDC), "400"),
            ],
            FeeReading::none_earned(),
        )
    };
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&borrowed), &[], &[])
        .expect("the reading is stored");
    let symbols: Vec<String> = vault
        .doc
        .holdings
        .iter()
        .filter(|holding| holding.store_id == wallet)
        .map(|holding| holding.asset.symbol.clone())
        .collect();
    assert_eq!(symbols, vec!["USDC".to_string()]);
}

#[test]
fn a_position_without_a_history_learns_its_receipt_from_the_adapter_and_the_table() {
    // The other half of the rule. A position read with no history behind it
    // (the address was followed from a later date) has no `Stake` operation
    // to learn the receipt from, and the capital would count the aToken in
    // the wallet on top of the position. Two sources answer instead: what the
    // adapter saw on this very position, stored with it, and the protocol's
    // static table, which also serves a device that only ever synced.
    let (_dir, mut vault, _wallet) = followed();
    known(&mut vault, USDC, "USDC");
    known(&mut vault, AUSDC, "AUSDC");
    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
    let live = PositionReading {
        receipts: vec![token(AUSDC)],
        ..reading(
            &key,
            PositionKind::Lending,
            vec![leg(CryptoDirection::In, token(USDC), "1000")],
            FeeReading::none_earned(),
        )
    };
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");

    let receipts = vault.crypto_position_receipts();
    assert!(
        receipts.contains(&token(AUSDC).key()),
        "the receipt the adapter itself saw is kept with the position"
    );
    assert!(
        !receipts.contains(&token(USDC).key()),
        "the underlying is what the position is worth, not a receipt for it"
    );
    // It survives a save and a reopen: the field is stored, not derived from
    // a reading this device happens to hold.
    let published = crate::cryptotrack::protocols::receipt_tokens("aave-v3", Chain::Ethereum);
    assert!(
        !published.is_empty(),
        "spec D3 filled the table of what Aave publishes in advance"
    );
    for asset in published {
        assert!(
            receipts.contains(&asset.key()),
            "a published receipt token is left out of the capital too"
        );
    }
    assert!(
        crate::cryptotrack::protocols::receipt_tokens("not-a-protocol", Chain::Ethereum).is_empty(),
        "a protocol nobody registered contributes nothing instead of failing"
    );
}

// -- what is stored -------------------------------------------------------------

#[test]
fn applying_a_reading_stores_the_legs_and_nothing_derived() {
    let (_dir, mut vault, _wallet) = followed();
    let key = PositionKey::new("uniswap-v3", Chain::Ethereum, "123456");
    let live = reading(
        &key,
        PositionKind::Pool,
        vec![leg(CryptoDirection::In, token(USDC), "2000")],
        FeeReading {
            collected: Some(Vec::new()),
            uncollected: Some(vec![AssetAmount {
                asset: token(USDC),
                amount: amount("30"),
            }]),
        },
    );
    let applied = vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");
    assert_eq!(applied.added, 1);
    assert_eq!(applied.rewritten, 0);
    assert_eq!(vault.doc.crypto_positions.len(), 1);
    assert_eq!(vault.doc.crypto_positions[0], live.stored());

    // A second pass of the identical position (only the moment differs)
    // changes nothing, so it saves nothing.
    let mut later = live.clone();
    later.read_at = "2026-05-13T09:15:00Z".into();
    let before = vault.doc.crypto_positions.clone();
    let again = vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&later), &[], &[])
        .expect("the reading is stored again");
    assert!(again.is_unchanged(), "{again:?}");
    assert_eq!(vault.doc.crypto_positions, before);

    // A reading that says something new rewrites the one record.
    let mut moved = later.clone();
    moved.legs = vec![leg(CryptoDirection::In, token(USDC), "2500")];
    let changed = vault
        .crypto_apply_positions(ACCOUNT, &[moved], &[], &[])
        .expect("the changed reading is stored");
    assert_eq!(changed.added, 0);
    assert_eq!(changed.rewritten, 1);
    assert_eq!(vault.doc.crypto_positions.len(), 1);
    assert_eq!(
        vault.doc.crypto_positions[0].updated_at,
        "2026-05-13T09:15:00Z"
    );
}

#[test]
fn a_reading_of_another_account_is_not_stored() {
    let (_dir, mut vault, _wallet) = followed();
    let key = PositionKey::new("lido", Chain::Ethereum, "steth");
    let stranger = PositionReading {
        account_id: "cacc-elsewhere".into(),
        ..reading(
            &key,
            PositionKind::Staking,
            vec![leg(CryptoDirection::In, ether(), "1")],
            FeeReading::none_earned(),
        )
    };
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[stranger], &[], &[])
        .expect("the pass stores nothing");
    assert_eq!(applied, CryptoPositionsApplied::default());
    assert!(vault.doc.crypto_positions.is_empty());
}

#[test]
fn a_position_survives_a_save_and_a_reopen() {
    let (dir, mut vault, _wallet) = followed();
    let key = PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve");
    let live = reading(
        &key,
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, token(USDC), "1000")],
        FeeReading::none_earned(),
    );
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the reading is stored");
    vault.flush_pending().expect("the write lands");
    let path = vault.path.clone();
    drop(vault);
    let reopened = UnlockedVault::open(path, "pw").expect("the vault reopens");
    assert_eq!(reopened.doc.crypto_positions, vec![live.stored()]);
    let _ = dir;
}

// -- a position the protocol no longer reports ----------------------------------

fn supply_reading() -> PositionReading {
    reading(
        &PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve"),
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, token(USDC), "1000")],
        FeeReading::none_earned(),
    )
}

/// What a successful discovery of `protocol` on `chain` left in `kept`.
fn review(protocol: &str, chain: Chain, kept: &[&PositionReading], at: &str) -> ProtocolReview {
    ProtocolReview {
        protocol: protocol.to_string(),
        chain,
        kept: kept.iter().map(|live| live.record_id()).collect(),
        read_at: at.to_string(),
    }
}

#[test]
fn a_position_the_protocol_no_longer_reports_is_closed_and_keeps_its_row() {
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");

    // The next pass: the protocol was asked, and lists nothing.
    let gone = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[gone])
        .expect("the withdrawal is stored");

    assert_eq!(applied.closed, 1);
    assert_eq!(vault.doc.crypto_positions.len(), 1, "history is kept");
    let stored = &vault.doc.crypto_positions[0];
    assert_eq!(stored.id, live.record_id());
    assert_eq!(stored.legs.len(), 1, "the legs stay, at zero");
    assert!(stored.legs[0].amount.is_zero());
    assert_eq!(stored.legs[0].asset, token(USDC));
    assert_eq!(stored.updated_at, "2026-05-14T09:00:00Z");
    let row = only(&vault, &[]);
    assert_eq!(row.status, PositionStatus::Closed);
}

#[test]
fn a_protocol_that_was_not_reviewed_leaves_its_positions_alone() {
    // Aave refused (it has no review), Uniswap answered with nothing: the
    // supply says nothing about either.
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");
    let other = review("uniswap-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[other])
        .expect("nothing to change");
    assert_eq!(applied, CryptoPositionsApplied::default());
    assert_eq!(vault.doc.crypto_positions, vec![live.stored()]);
}

#[test]
fn a_position_the_review_found_but_could_not_read_is_not_closed() {
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");
    let found = review("aave-v3", Chain::Ethereum, &[&live], "2026-05-14T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[found])
        .expect("nothing to change");
    assert_eq!(applied, CryptoPositionsApplied::default());
    assert_eq!(vault.doc.crypto_positions, vec![live.stored()]);
}

#[test]
fn a_review_closes_only_its_own_account_and_network() {
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    // Another followed address, another reserve of the same protocol: the
    // review of this account has no say over it.
    let mut elsewhere = reading(
        &PositionKey::new("aave-v3", Chain::Ethereum, "weth-reserve"),
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, ether(), "3")],
        FeeReading::none_earned(),
    )
    .stored();
    elsewhere.account_id = "cacc-elsewhere".into();
    vault.doc.crypto_positions.push(elsewhere.clone());
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");

    // Same protocol, another network: no say over an Ethereum position.
    let base = review("aave-v3", Chain::Base, &[], "2026-05-14T09:00:00Z");
    let none = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[base])
        .expect("nothing to change");
    assert_eq!(none, CryptoPositionsApplied::default());

    // The right network closes this account's position and not the other's.
    let right = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[right])
        .expect("the withdrawal is stored");
    assert_eq!(applied.closed, 1);
    let untouched = vault
        .doc
        .crypto_positions
        .iter()
        .find(|have| have.account_id == "cacc-elsewhere")
        .expect("the other account's row is still there");
    assert_eq!(untouched, &elsewhere);
}

#[test]
fn a_closed_position_is_not_rewritten_by_the_next_pass() {
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");
    let first = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[first])
        .expect("closed");
    let again = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:15:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[again])
        .expect("nothing to change");
    assert_eq!(applied, CryptoPositionsApplied::default());
    assert_eq!(
        vault.doc.crypto_positions[0].updated_at, "2026-05-14T09:00:00Z",
        "the close is stamped once, not at every pass"
    );
}

#[test]
fn a_closed_position_opens_again_when_the_protocol_reports_it() {
    let (_dir, mut vault, _wallet) = followed();
    let live = supply_reading();
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the supply is stored");
    let gone = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[gone])
        .expect("closed");

    let back = reading(
        &live.key,
        PositionKind::Lending,
        vec![leg(CryptoDirection::In, token(USDC), "250")],
        FeeReading::none_earned(),
    );
    let kept = review("aave-v3", Chain::Ethereum, &[&back], "2026-05-15T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&back), &[], &[kept])
        .expect("reopened");
    assert_eq!(applied.rewritten, 1);
    assert_eq!(applied.closed, 0);
    assert_eq!(vault.doc.crypto_positions, vec![back.stored()]);
    assert_eq!(only(&vault, &[]).status, PositionStatus::Open);
}

// -- one reserve, two followed accounts (review item 7.19) ----------------------

const SECOND: &str = "cacc-2";

fn follow_second(vault: &mut UnlockedVault) {
    vault.doc.crypto_accounts.push(CryptoAccount::new(
        SECOND,
        "Hot EVM",
        Chain::Ethereum,
        OUTSIDE,
        "2026-02-01T00:00:00Z",
    ));
}

fn usdc_supply_of(account: &str, quantity: &str) -> PositionReading {
    PositionReading {
        account_id: account.to_string(),
        ..reading(
            &PositionKey::new("aave-v3", Chain::Ethereum, "usdc-reserve"),
            PositionKind::Lending,
            vec![leg(CryptoDirection::In, token(USDC), quantity)],
            FeeReading::none_earned(),
        )
    }
}

#[test]
fn two_accounts_in_one_reserve_are_two_records_with_their_own_legs() {
    let (_dir, mut vault, _wallet) = followed();
    follow_second(&mut vault);
    let first = usdc_supply_of(ACCOUNT, "1000");
    let second = usdc_supply_of(SECOND, "250");
    vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&first), &[], &[])
        .expect("the first account is stored");
    vault
        .crypto_apply_positions(SECOND, std::slice::from_ref(&second), &[], &[])
        .expect("the second account is stored");

    assert_eq!(vault.doc.crypto_positions.len(), 2);
    let stored = |account: &str| {
        vault
            .doc
            .crypto_positions
            .iter()
            .find(|record| record.account_id == account)
            .expect("each account keeps its record")
    };
    assert_eq!(stored(ACCOUNT).legs[0].amount, amount("1000"));
    assert_eq!(stored(SECOND).legs[0].amount, amount("250"));
    assert_ne!(stored(ACCOUNT).id, stored(SECOND).id);
}

#[test]
fn closing_one_accounts_position_leaves_the_others_open() {
    let (_dir, mut vault, _wallet) = followed();
    follow_second(&mut vault);
    let first = usdc_supply_of(ACCOUNT, "1000");
    let second = usdc_supply_of(SECOND, "250");
    for (account, live) in [(ACCOUNT, &first), (SECOND, &second)] {
        vault
            .crypto_apply_positions(account, std::slice::from_ref(live), &[], &[])
            .expect("stored");
    }
    // The first account withdrew: its protocol review no longer keeps it.
    let gone = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    let applied = vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[gone])
        .expect("the withdrawal is stored");
    assert_eq!(applied.closed, 1);
    let open = vault
        .doc
        .crypto_positions
        .iter()
        .filter(|record| !is_closed(&record.legs))
        .collect::<Vec<_>>();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].account_id, SECOND);
}

/// What a build from before the account joined the key stored.
fn legacy_record(live: &PositionReading) -> CryptoPosition {
    CryptoPosition {
        id: live.key.legacy_record_id(),
        ..live.stored()
    }
}

#[test]
fn a_record_of_the_old_key_is_renamed_once_its_account_is_applied() {
    let (_dir, mut vault, _wallet) = followed();
    let live = usdc_supply_of(ACCOUNT, "1000");
    vault.doc.crypto_positions.push(legacy_record(&live));

    let applied = vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the pass stores the reading");
    assert_eq!(applied.migrated, 1);
    assert_eq!(applied.added, 0, "the old record became the new one");
    assert_eq!(vault.doc.crypto_positions.len(), 1);
    assert_eq!(vault.doc.crypto_positions[0].id, live.record_id());
}

#[test]
fn a_closed_record_of_the_old_key_stays_closed_under_the_new_one() {
    let (_dir, mut vault, _wallet) = followed();
    let live = usdc_supply_of(ACCOUNT, "1000");
    let mut closed = legacy_record(&live);
    closed.legs[0].amount = Amount::ZERO;
    vault.doc.crypto_positions.push(closed);

    // The protocol lists nothing: the row is neither duplicated nor revived,
    // and ADR-02 closing keeps working on it.
    let gone = review("aave-v3", Chain::Ethereum, &[], "2026-05-14T09:00:00Z");
    vault
        .crypto_apply_positions(ACCOUNT, &[], &[], &[gone])
        .expect("the pass is stored");
    assert_eq!(vault.doc.crypto_positions.len(), 1);
    assert_eq!(vault.doc.crypto_positions[0].id, live.record_id());
    assert!(is_closed(&vault.doc.crypto_positions[0].legs));
}

#[test]
fn an_old_key_record_written_back_by_an_older_build_is_not_counted_twice() {
    let (_dir, mut vault, _wallet) = followed();
    let live = usdc_supply_of(ACCOUNT, "1000");
    vault.doc.crypto_positions.push(live.stored());
    // An older build on another device synced the same position back under
    // the old key.
    vault.doc.crypto_positions.push(legacy_record(&live));

    assert_eq!(
        vault.crypto_list_positions(&[], LATER).len(),
        1,
        "the stale twin is not listed"
    );
    let applied = vault
        .crypto_apply_positions(ACCOUNT, std::slice::from_ref(&live), &[], &[])
        .expect("the pass is stored");
    assert_eq!(applied.migrated, 1);
    assert_eq!(vault.doc.crypto_positions, vec![live.stored()]);
}

#[test]
fn a_position_of_one_account_does_not_take_the_others_history() {
    let (_dir, mut vault, _wallet) = followed();
    follow_second(&mut vault);
    let first = usdc_supply_of(ACCOUNT, "1000");
    let second = usdc_supply_of(SECOND, "250");
    vault.doc.crypto_positions.push(first.stored());
    vault.doc.crypto_positions.push(second.stored());
    let rows = vault.crypto_list_positions(&[], LATER);
    assert_eq!(rows.len(), 2);
    assert_ne!(rows[0].id, rows[1].id);
    assert!(rows.iter().all(|row| row.id.starts_with(&row.account_id)));
}
