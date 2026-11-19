//! Who an account dealt with (spec E15): the user's other addresses, the
//! counterparties the user named an exchanger and the protocols met.
//!
//! Computed from the events on every call. Nothing is stored, nothing is
//! written. The rows, the operation filter each one carries and the fiat
//! records paired with the operations are made from these tallies by
//! `usecases::cryptotrack::relations`.

use std::collections::BTreeMap;

use super::accounts::wallet_of;
use crate::cryptotrack::engine::{ids, overrides};
use crate::model::chain::Chain;
use crate::model::contact::{ContactKind, DetailKind};
use crate::model::cryptotrack::{CryptoAccount, TrackingRef, normalize_address};
use crate::model::vault::VaultDoc;

/// What is counted for one row while the events are walked.
pub struct Tally {
    pub label: String,
    pub address: Option<String>,
    pub count: u32,
    pub last_time: String,
}

impl Tally {
    fn add(&mut self, time: &str) {
        self.count += 1;
        if time > self.last_time.as_str() {
            self.last_time = time.to_string();
        }
    }
}

/// The three groups of rows, each by its key (an address or a protocol).
#[derive(Default)]
pub struct Tallies {
    pub addresses: BTreeMap<String, Tally>,
    pub exchanges: BTreeMap<String, Tally>,
    pub protocols: BTreeMap<String, Tally>,
}

fn bump(
    group: &mut BTreeMap<String, Tally>,
    key: String,
    label: &str,
    address: Option<&str>,
    time: &str,
) {
    group
        .entry(key)
        .or_insert_with(|| Tally {
            label: label.to_string(),
            address: address.map(str::to_string),
            count: 0,
            last_time: String::new(),
        })
        .add(time);
}

/// The wallet's name when a wallet follows the account, else the account's own.
fn account_name(doc: &VaultDoc, account: &CryptoAccount) -> String {
    wallet_of(doc, &account.id).map_or_else(|| account.name.clone(), |store| store.name.clone())
}

/// Everything the account dealt with, read from its events: the hidden
/// operations and the module's own entries are left out, as the list leaves
/// them out by default.
pub fn tally_relations(doc: &VaultDoc, account: &CryptoAccount) -> Tallies {
    let own = normalize_address(account.chain, &account.address);
    let fixes = overrides::index(&doc.crypto_overrides);
    let mut out = Tallies::default();

    for event in &doc.crypto_events {
        let fix = fixes.get(event.id.as_str()).copied();
        if event.account_id != account.id
            || ids::adjustment(&event.id).is_some()
            || fix.is_some_and(|fix| fix.hidden)
        {
            continue;
        }
        if let Some(protocol) = &event.protocol {
            bump(
                &mut out.protocols,
                protocol.clone(),
                protocol,
                None,
                &event.time,
            );
        }
        if matches!(event.tracking, TrackingRef::ExchangeHistory { .. })
            || event.chain == Chain::Fiat
        {
            continue;
        }
        for side in [&event.from, &event.to] {
            let address = normalize_address(event.chain, side);
            if address.is_empty() || address == own {
                continue;
            }
            if let Some(other) = doc.crypto_accounts.iter().find(|other| {
                other.id != account.id
                    && other.chain == event.chain
                    && normalize_address(other.chain, &other.address) == address
            }) {
                let label = account_name(doc, other);
                bump(
                    &mut out.addresses,
                    address.clone(),
                    &label,
                    Some(&address),
                    &event.time,
                );
                continue;
            }
            let contact = fix
                .and_then(|fix| fix.contact_id.as_deref())
                .and_then(|id| doc.contacts.iter().find(|contact| contact.id == id))
                .or_else(|| {
                    doc.contacts.iter().find(|contact| {
                        !contact.archived
                            && contact.accounts.iter().any(|saved| {
                                saved.kind == DetailKind::CryptoAddress
                                    && saved.active
                                    && saved.network.is_none_or(|network| network == event.chain)
                                    && normalize_address(event.chain, &saved.value) == address
                            })
                    })
                });
            if let Some(contact) = contact.filter(|contact| contact.kind == ContactKind::Exchanger)
            {
                bump(
                    &mut out.exchanges,
                    address.clone(),
                    &contact.name,
                    Some(&address),
                    &event.time,
                );
            }
        }
    }
    out
}
