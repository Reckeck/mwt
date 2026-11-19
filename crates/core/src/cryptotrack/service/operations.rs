//! The module's operations as the screens read them, and the corrections the
//! user lays over them (spec E10): what is read off the document alone, the
//! symbol of a coin, the name of an account, the contact behind an address, and
//! the three small writes a correction makes (joining and leaving a chain,
//! remembering an address). The rows, the pages, the candidates and the
//! correction itself build the screens' types and are in
//! `usecases::cryptotrack::operations`.
//!
//! **Nothing here creates or deletes an operation.** Events come from the
//! sources and are rewritten by a refetch; the one write in this file is a
//! [`CryptoOverride`], which carries classification and attributes (kind,
//! comment, categories, contact, hidden, chain) and never an amount. The
//! chain write also sets `flow_id` on a fiat operation the user chose, which
//! is the same id the fiat `link_transactions` writes.
//!
//! Reading builds no URL: a row only says whether a link exists
//! (`linkable`). The address of the page is made in the detail, when the user
//! opens the operation, and the frontend opens it only on a click.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use chrono::{DateTime, NaiveDate};

use crate::Amount;
use crate::cryptotrack::chains::evm_chain;
use crate::cryptotrack::engine::{ids, math, overrides};
use crate::cryptotrack::prices;
use crate::error::{Error, Result};
use crate::model::chain::Chain;
use crate::model::contact::{ContactAccount, DetailKind};
use crate::model::cryptotrack::{
    CryptoAsset, CryptoDirection, CryptoOverride, CryptoTx, normalize_address,
};
use crate::model::vault::VaultDoc;

/// The coin a network pays gas in.
pub fn native_symbol(chain: Chain) -> &'static str {
    if let Some(row) = evm_chain(chain) {
        return row.native.symbol;
    }
    match chain {
        Chain::Bitcoin => "BTC",
        Chain::Solana => "SOL",
        Chain::Tron => "TRX",
        Chain::Litecoin => "LTC",
        Chain::Dogecoin => "DOGE",
        Chain::Cardano => "ADA",
        Chain::Xrp => "XRP",
        Chain::Polkadot => "DOT",
        Chain::Cosmos => "ATOM",
        Chain::Near => "NEAR",
        Chain::Ton => "TON",
        Chain::Algorand => "ALGO",
        _ => "",
    }
}

pub fn short(text: &str) -> String {
    let count = text.chars().count();
    if count <= 12 {
        return text.to_string();
    }
    let head: String = text.chars().take(6).collect();
    let tail: String = text.chars().skip(count - 4).collect();
    format!("{head}…{tail}")
}

pub fn secs(time: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(time)
        .ok()
        .map(|moment| moment.timestamp())
}

pub fn day_of(time: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(time.get(..10)?, "%Y-%m-%d").ok()
}

/// A rule for the contract of `event`, if the user set one.
pub fn rule_for<'doc>(
    rules: &BTreeMap<&str, &'doc CryptoOverride>,
    event: &CryptoTx,
) -> Option<&'doc CryptoOverride> {
    overrides::contract_key(event).and_then(|key| rules.get(key.as_str()).copied())
}

/// The other side of an operation, from the followed account's view.
pub fn other_side(event: &CryptoTx) -> &str {
    match event.legs.first().map(|leg| leg.direction) {
        Some(CryptoDirection::In) => &event.from,
        _ => &event.to,
    }
}

pub fn crypto_symbol(doc: &VaultDoc, asset: &CryptoAsset) -> String {
    match asset {
        CryptoAsset::Native { chain } => native_symbol(*chain).to_string(),
        // A coin on an exchange is `Token(Fiat, "<TICKER>")`.
        CryptoAsset::Token(token) if token.chain == Chain::Fiat => token.contract.to_uppercase(),
        CryptoAsset::Token(token) => doc
            .crypto_tokens
            .iter()
            .find(|record| record.id() == *token)
            .map_or_else(|| short(&token.contract), |record| record.symbol.clone()),
    }
}

pub fn crypto_account_name(doc: &VaultDoc, account_id: &str) -> String {
    if let Some(store) = doc
        .stores
        .iter()
        .find(|store| store.crypto_account_id.as_deref() == Some(account_id))
    {
        return store.name.clone();
    }
    doc.crypto_accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| account.name.clone())
        .unwrap_or_default()
}

/// The contact whose saved crypto address is `address` on `chain`.
pub fn crypto_contact_by_address<'doc>(
    doc: &'doc VaultDoc,
    chain: Chain,
    address: &str,
) -> Option<&'doc str> {
    let wanted = normalize_address(chain, address);
    if wanted.is_empty() {
        return None;
    }
    doc.contacts
        .iter()
        .filter(|contact| !contact.archived)
        .find(|contact| {
            contact.accounts.iter().any(|account| {
                account.kind == DetailKind::CryptoAddress
                    && account.active
                    && account.network.is_none_or(|network| network == chain)
                    && normalize_address(chain, &account.value) == wanted
            })
        })
        .map(|contact| contact.id.as_str())
}

pub fn crypto_signed_value(doc: &VaultDoc, event: &CryptoTx) -> Option<Amount> {
    let day = day_of(&event.time)?;
    // Quantities and prices come from a chain, where a token may carry
    // 1e24 units: `Amount`'s own operators panic on overflow and the
    // release profile aborts, so every step goes through `math`, which
    // answers `None` instead (the figure then shows as a dash).
    let mut total = Amount::ZERO;
    for leg in &event.legs {
        let price = prices::lookup(doc, &leg.asset, day).value?;
        let value = math::mul(leg.amount, price)?;
        total = match leg.direction {
            CryptoDirection::In => math::add(total, value)?,
            CryptoDirection::Out => math::sub(total, value)?,
        };
    }
    (!event.legs.is_empty()).then_some(total)
}

pub fn crypto_event_ref<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc CryptoTx> {
    doc.crypto_events
        .iter()
        .find(|event| event.id == id && ids::adjustment(&event.id).is_none())
        .ok_or_else(|| Error::NotFound(format!("operation {id}")))
}

/// Put the fiat operation `tx_id` and a crypto one into one chain: the
/// crypto one's own, else the fiat one's, else a new one.
pub fn crypto_join_chain(doc: &mut VaultDoc, tx_id: &str, current: Option<&str>) -> Result<String> {
    let tx = doc
        .transactions
        .iter_mut()
        .find(|tx| tx.meta().id == tx_id)
        .ok_or_else(|| Error::NotFound(format!("transaction {tx_id}")))?;
    let flow = current
        .map(str::to_string)
        .or_else(|| tx.meta().flow_id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    tx.meta_mut().flow_id = Some(flow.clone());
    Ok(flow)
}

/// After a crypto operation left chain `flow`: a chain that is left with
/// one fiat operation and nothing else was made by this link, so it goes.
pub fn crypto_release_chain(doc: &mut VaultDoc, flow: &str, leaving: &str) {
    let coins = doc
        .crypto_overrides
        .iter()
        .filter(|fix| fix.event_id != leaving && fix.flow_id.as_deref() == Some(flow))
        .count();
    let fiat = doc
        .transactions
        .iter()
        .filter(|tx| tx.meta().flow_id.as_deref() == Some(flow))
        .count();
    if coins == 0 && fiat <= 1 {
        for tx in doc
            .transactions
            .iter_mut()
            .filter(|tx| tx.meta().flow_id.as_deref() == Some(flow))
        {
            tx.meta_mut().flow_id = None;
        }
    }
}

/// «Through whom» also teaches the contact the address, so the next
/// operation with it is named on its own.
pub fn crypto_remember_address(doc: &mut VaultDoc, contact_id: &str, event: &CryptoTx) {
    let address = normalize_address(event.chain, other_side(event));
    if address.is_empty() || event.chain == Chain::Fiat {
        return;
    }
    let own: BTreeSet<String> = doc
        .crypto_accounts
        .iter()
        .map(|account| normalize_address(account.chain, &account.address))
        .collect();
    if own.contains(&address) {
        return;
    }
    let chain = event.chain;
    let Some(contact) = doc.contacts.iter_mut().find(|known| known.id == contact_id) else {
        return;
    };
    let known = contact.accounts.iter().any(|account| {
        account.kind == DetailKind::CryptoAddress
            && normalize_address(chain, &account.value) == address
    });
    if known {
        return;
    }
    contact.accounts.push(ContactAccount {
        id: uuid::Uuid::new_v4().to_string(),
        group: None,
        label: chain.label().to_string(),
        kind: DetailKind::CryptoAddress,
        value: address,
        currency: None,
        network: Some(chain),
        active: true,
    });
}
