//! Who an account dealt with (spec E15): the user's other addresses, the
//! counterparties the user named an exchanger, the protocols met and the fiat
//! records paired with its operations.
//!
//! Computed from the events on every call. Nothing is stored, nothing is
//! written, and a row carries the operation filter that lists exactly what it
//! counted, so the screen opens the operations without building one.

use std::collections::BTreeMap;

use super::super::UnlockedVault;
use crate::cryptotrack::engine::{ids, overrides};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::links::CryptoPairsInput;
use crate::ipc_contract::cryptotrack::operations::CryptoOpFilter;
use crate::ipc_contract::cryptotrack::relations::{
    CryptoFiatGroup, CryptoFiatRelation, CryptoRelationGroup, CryptoRelationRow, CryptoRelations,
    CryptoRelationsInput,
};
use crate::model::chain::Chain;
use crate::model::contact::{ContactKind, DetailKind};
use crate::model::cryptotrack::{TrackingRef, normalize_address};

/// The most rows of one group.
const ROW_CAP: usize = 50;

/// What is counted for one row while the events are walked.
struct Tally {
    label: String,
    address: Option<String>,
    count: u32,
    last_time: String,
}

impl Tally {
    fn add(&mut self, time: &str) {
        self.count += 1;
        if time > self.last_time.as_str() {
            self.last_time = time.to_string();
        }
    }
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

fn rows(
    group: BTreeMap<String, Tally>,
    filter_of: impl Fn(&Tally, &str) -> CryptoOpFilter,
) -> CryptoRelationGroup {
    let mut out: Vec<CryptoRelationRow> = group
        .into_iter()
        .map(|(key, tally)| CryptoRelationRow {
            filter: filter_of(&tally, &key),
            key,
            label: tally.label,
            address: tally.address,
            count: tally.count,
            last_time: tally.last_time,
        })
        .collect();
    out.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
            .then_with(|| left.key.cmp(&right.key))
    });
    let total = u32::try_from(out.len()).unwrap_or(u32::MAX);
    out.truncate(ROW_CAP);
    CryptoRelationGroup { items: out, total }
}

impl UnlockedVault {
    /// Everything the account dealt with, read from its events. An unknown
    /// account is `NOT_FOUND`; the hidden operations and the module's own
    /// entries are left out, as the list leaves them out by default.
    pub fn crypto_relations(&self, input: &CryptoRelationsInput) -> Result<CryptoRelations> {
        let account = self
            .doc
            .crypto_accounts
            .iter()
            .find(|account| account.id == input.account_id)
            .ok_or_else(|| Error::NotFound(format!("account {}", input.account_id)))?;
        let own = normalize_address(account.chain, &account.address);
        let fixes = overrides::index(&self.doc.crypto_overrides);
        let account_filter = CryptoOpFilter {
            account_ids: vec![account.id.clone()],
            ..Default::default()
        };

        let mut addresses: BTreeMap<String, Tally> = BTreeMap::new();
        let mut exchanges: BTreeMap<String, Tally> = BTreeMap::new();
        let mut protocols: BTreeMap<String, Tally> = BTreeMap::new();

        for event in &self.doc.crypto_events {
            let fix = fixes.get(event.id.as_str()).copied();
            if event.account_id != account.id
                || ids::adjustment(&event.id).is_some()
                || fix.is_some_and(|fix| fix.hidden)
            {
                continue;
            }
            if let Some(protocol) = &event.protocol {
                bump(
                    &mut protocols,
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
                if let Some(other) = self.doc.crypto_accounts.iter().find(|other| {
                    other.id != account.id
                        && other.chain == event.chain
                        && normalize_address(other.chain, &other.address) == address
                }) {
                    let label = self.relation_account_name(&other.id, &other.name);
                    bump(
                        &mut addresses,
                        address.clone(),
                        &label,
                        Some(&address),
                        &event.time,
                    );
                    continue;
                }
                let contact = fix
                    .and_then(|fix| fix.contact_id.as_deref())
                    .and_then(|id| self.doc.contacts.iter().find(|contact| contact.id == id))
                    .or_else(|| {
                        self.doc.contacts.iter().find(|contact| {
                            !contact.archived
                                && contact.accounts.iter().any(|saved| {
                                    saved.kind == DetailKind::CryptoAddress
                                        && saved.active
                                        && saved
                                            .network
                                            .is_none_or(|network| network == event.chain)
                                        && normalize_address(event.chain, &saved.value) == address
                                })
                        })
                    });
                if let Some(contact) =
                    contact.filter(|contact| contact.kind == ContactKind::Exchanger)
                {
                    bump(
                        &mut exchanges,
                        address.clone(),
                        &contact.name,
                        Some(&address),
                        &event.time,
                    );
                }
            }
        }

        let by_address = |tally: &Tally, _key: &str| CryptoOpFilter {
            address: tally.address.clone(),
            ..account_filter.clone()
        };
        let by_protocol = |_tally: &Tally, key: &str| CryptoOpFilter {
            protocol: Some(key.to_string()),
            ..account_filter.clone()
        };

        let mut fiat: Vec<CryptoFiatRelation> = self
            .crypto_pairs(&CryptoPairsInput::default())
            .into_iter()
            .filter_map(|pair| {
                let (event, fiat) = (pair.event?, pair.fiat?);
                (event.account_id == account.id).then(|| CryptoFiatRelation {
                    event_id: pair.event_id,
                    fiat,
                    filter: account_filter.clone(),
                })
            })
            .collect();
        fiat.sort_by(|left, right| {
            right
                .fiat
                .time
                .cmp(&left.fiat.time)
                .then_with(|| left.event_id.cmp(&right.event_id))
        });
        let fiat_total = u32::try_from(fiat.len()).unwrap_or(u32::MAX);
        fiat.truncate(ROW_CAP);

        Ok(CryptoRelations {
            own_addresses: rows(addresses, by_address),
            exchanges: rows(exchanges, by_address),
            protocols: rows(protocols, by_protocol),
            fiat: CryptoFiatGroup {
                items: fiat,
                total: fiat_total,
            },
        })
    }

    /// The wallet's name when a wallet follows the account, else the account's own.
    pub(crate) fn relation_account_name(&self, account_id: &str, fallback: &str) -> String {
        self.doc
            .stores
            .iter()
            .find(|store| store.crypto_account_id.as_deref() == Some(account_id))
            .map_or_else(|| fallback.to_string(), |store| store.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contract::cryptotrack::links::CryptoLinkInput;
    use crate::ipc_contract::cryptotrack::operations::CryptoListOperationsInput;
    use crate::model::contact::{Contact, ContactAccount};
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoOverride, CryptoTx,
        CryptoTxKind,
    };
    use crate::model::store::StoreKind;
    use crate::service::test_support::{amount, at, empty_vault, payment, store};

    const MINE: &str = "0x00000000000000000000000000000000000000a1";
    const OTHER: &str = "0x00000000000000000000000000000000000000a2";
    const SHOP: &str = "0x00000000000000000000000000000000000000b1";
    const ROUTER: &str = "0x00000000000000000000000000000000000000c1";

    fn event(id: &str, from: &str, to: &str, protocol: Option<&str>) -> CryptoTx {
        CryptoTx {
            id: id.into(),
            chain: Chain::Ethereum,
            account_id: "acc-1".into(),
            time: format!("2026-11-1{}T10:00:00Z", id.len() % 9),
            block: Some(1),
            from: from.into(),
            to: to.into(),
            legs: vec![CryptoLeg {
                direction: CryptoDirection::Out,
                asset: CryptoAsset::Native {
                    chain: Chain::Ethereum,
                },
                amount: amount("1"),
                counterparty: None,
            }],
            gas: None,
            protocol: protocol.map(str::to_string),
            method: None,
            kind: CryptoTxKind::Transfer,
            raw: None,
            tracking: TrackingRef::ChainTx {
                chain: Chain::Ethereum,
                hash: format!("0x{}", id.len()),
            },
        }
    }

    fn setup() -> (tempfile::TempDir, UnlockedVault) {
        let (dir, mut vault) = empty_vault();
        for (id, name, address) in [("acc-1", "Main", MINE), ("acc-2", "Savings", OTHER)] {
            vault.doc.crypto_accounts.push(CryptoAccount::new(
                id,
                name,
                Chain::Ethereum,
                address,
                "2026-01-01T00:00:00Z",
            ));
        }
        vault.doc.contacts.push(Contact {
            id: "c-ex".into(),
            name: "Exchanger".into(),
            kind: ContactKind::Exchanger,
            accounts: vec![ContactAccount {
                id: "a1".into(),
                kind: DetailKind::CryptoAddress,
                value: SHOP.into(),
                network: Some(Chain::Ethereum),
                active: true,
                ..Default::default()
            }],
            ..Default::default()
        });
        vault.doc.crypto_events = vec![
            event("a", MINE, OTHER, None),
            event("bb", OTHER, MINE, None),
            event("ccc", MINE, SHOP, None),
            event("dddd", MINE, ROUTER, Some("uniswap")),
            event("eeeee", MINE, ROUTER, Some("uniswap")),
        ];
        (dir, vault)
    }

    fn relations(vault: &UnlockedVault) -> CryptoRelations {
        vault
            .crypto_relations(&CryptoRelationsInput {
                account_id: "acc-1".into(),
            })
            .unwrap()
    }

    /// A wallet with years of history must not be answered with a list that
    /// was cut in silence: the group reports how many rows there are in all,
    /// and the screen says «shown N of M» from it.
    #[test]
    fn a_cut_group_still_reports_how_many_there_are() {
        let (_dir, mut vault) = setup();
        for index in 0..ROW_CAP + 7 {
            vault.doc.crypto_events.push(event(
                &format!("p{index}"),
                MINE,
                ROUTER,
                Some(&format!("protocol-{index:03}")),
            ));
        }
        let found = relations(&vault);
        assert_eq!(
            found.protocols.items.len(),
            ROW_CAP,
            "the answer stays bounded"
        );
        assert_eq!(
            found.protocols.total as usize,
            ROW_CAP + 8,
            "and it still says how many were found, uniswap among them"
        );
        assert_eq!(
            found.own_addresses.total as usize,
            found.own_addresses.items.len(),
            "a group that was not cut reports its own length"
        );
    }

    #[test]
    fn it_names_own_addresses_exchangers_and_protocols() {
        let (_dir, vault) = setup();
        let found = relations(&vault);
        assert_eq!(found.own_addresses.items.len(), 1);
        assert_eq!(found.own_addresses.items[0].label, "Savings");
        assert_eq!(found.own_addresses.items[0].count, 2);
        assert_eq!(found.exchanges.items.len(), 1);
        assert_eq!(found.exchanges.items[0].label, "Exchanger");
        assert_eq!(found.protocols.items.len(), 1);
        assert_eq!(found.protocols.items[0].label, "uniswap");
        assert_eq!(found.protocols.items[0].count, 2);
        assert!(found.fiat.items.is_empty());
    }

    /// The filter of a row lists exactly the operations the row counted.
    #[test]
    fn every_row_filter_lists_what_it_counted() {
        let (_dir, vault) = setup();
        let found = relations(&vault);
        for row in found
            .own_addresses
            .items
            .iter()
            .chain(&found.exchanges.items)
            .chain(&found.protocols.items)
        {
            let page = vault.crypto_list_operations(&CryptoListOperationsInput {
                filter: row.filter.clone(),
                ..Default::default()
            });
            assert_eq!(page.total, row.count, "row {}", row.key);
        }
    }

    #[test]
    fn hidden_operations_and_foreign_accounts_are_not_counted() {
        let (_dir, mut vault) = setup();
        vault.doc.crypto_overrides.push(CryptoOverride {
            id: "cov:dddd".into(),
            event_id: "dddd".into(),
            kind: None,
            comment: None,
            categories: Vec::new(),
            flow_id: None,
            contact_id: None,
            not_exchange: false,
            hidden: true,
            fiat_tx_id: None,
            updated_at: "2026-11-12T00:00:00Z".into(),
        });
        let found = relations(&vault);
        assert_eq!(found.protocols.items[0].count, 1);
        let other = vault
            .crypto_relations(&CryptoRelationsInput {
                account_id: "acc-2".into(),
            })
            .unwrap();
        assert!(other.own_addresses.items.is_empty() && other.protocols.items.is_empty());
    }

    #[test]
    fn fiat_records_come_from_the_pointers_and_nothing_is_written() {
        let (_dir, mut vault) = setup();
        let bank = store(&mut vault, "Bank", StoreKind::Cash, None);
        let mut spend = payment(&bank, "500", "to the exchanger");
        at(&mut spend, "2026-11-11T09:00:00Z");
        let id = vault.add_transaction(spend).unwrap().meta().id.clone();
        vault
            .crypto_link_fiat(&CryptoLinkInput {
                event_id: "ccc".into(),
                fiat_tx_id: id.clone(),
            })
            .unwrap();
        let before = serde_json::to_string(&vault.doc.crypto_overrides).unwrap();
        let found = relations(&vault);
        assert_eq!(found.fiat.items.len(), 1);
        assert_eq!(found.fiat.items[0].event_id, "ccc");
        assert_eq!(found.fiat.items[0].fiat.id, id);
        assert_eq!(
            found.fiat.items[0].filter.account_ids,
            vec!["acc-1".to_string()]
        );
        assert_eq!(
            serde_json::to_string(&vault.doc.crypto_overrides).unwrap(),
            before
        );
        // A deleted fiat record leaves no row, and no error.
        vault.delete_transaction(&id).unwrap();
        assert!(relations(&vault).fiat.items.is_empty());
    }

    #[test]
    fn an_unknown_account_is_not_found() {
        let (_dir, vault) = setup();
        assert!(
            vault
                .crypto_relations(&CryptoRelationsInput {
                    account_id: "nope".into()
                })
                .is_err()
        );
    }
}
