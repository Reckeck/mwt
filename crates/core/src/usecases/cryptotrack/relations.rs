//! The read of who an account dealt with (spec E15). The tallies are made in
//! `cryptotrack::service::relations`; this turns them into the rows the screen
//! shows, each carrying the operation filter that lists exactly what it
//! counted, and adds the fiat records paired with the account's operations.

use std::collections::BTreeMap;

use crate::cryptotrack::service::relations::{Tally, tally_relations};
use crate::error::{Error, Result};
use crate::ipc_contract::cryptotrack::links::CryptoPairsInput;
use crate::ipc_contract::cryptotrack::operations::CryptoOpFilter;
use crate::ipc_contract::cryptotrack::relations::{
    CryptoFiatGroup, CryptoFiatRelation, CryptoRelationGroup, CryptoRelationRow, CryptoRelations,
    CryptoRelationsInput,
};
use crate::money::books::Books;
use crate::usecases::cryptotrack::links::crypto_pairs;

/// The most rows of one group.
const ROW_CAP: usize = 50;

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

/// Everything the account dealt with, read from its events. An unknown
/// account is `NOT_FOUND`; the hidden operations and the module's own
/// entries are left out, as the list leaves them out by default.
pub fn crypto_relations(
    books: &Books<'_>,
    input: &CryptoRelationsInput,
) -> Result<CryptoRelations> {
    let doc = books.doc();
    let account = doc
        .crypto_accounts
        .iter()
        .find(|account| account.id == input.account_id)
        .ok_or_else(|| Error::NotFound(format!("account {}", input.account_id)))?;
    let account_filter = CryptoOpFilter {
        account_ids: vec![account.id.clone()],
        ..Default::default()
    };
    let tallies = tally_relations(doc, account);

    let by_address = |tally: &Tally, _key: &str| CryptoOpFilter {
        address: tally.address.clone(),
        ..account_filter.clone()
    };
    let by_protocol = |_tally: &Tally, key: &str| CryptoOpFilter {
        protocol: Some(key.to_string()),
        ..account_filter.clone()
    };

    let mut fiat: Vec<CryptoFiatRelation> = crypto_pairs(books, &CryptoPairsInput::default())
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
        own_addresses: rows(tallies.addresses, by_address),
        exchanges: rows(tallies.exchanges, by_address),
        protocols: rows(tallies.protocols, by_protocol),
        fiat: CryptoFiatGroup {
            items: fiat,
            total: fiat_total,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc_contract::cryptotrack::links::CryptoLinkInput;
    use crate::ipc_contract::cryptotrack::operations::CryptoListOperationsInput;
    use crate::model::chain::Chain;
    use crate::model::contact::{Contact, ContactAccount};
    use crate::model::contact::{ContactKind, DetailKind};
    use crate::model::cryptotrack::TrackingRef;
    use crate::model::cryptotrack::{
        CryptoAccount, CryptoAsset, CryptoDirection, CryptoLeg, CryptoOverride, CryptoTx,
        CryptoTxKind,
    };
    use crate::model::store::StoreKind;
    use crate::service::UnlockedVault;
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
