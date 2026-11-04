//! Catalog 96: the references of a removed duplicate move to the kept copy.

use super::tests::{settle, two_devices};
use super::*;
use crate::model::enrichment::{
    EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus,
};
use crate::model::entity_ref::{EntityKind, EntityRef};
use crate::model::shared_event::{EventItem, Purchase, Settlement, SharedEvent};
use crate::model::transaction::Transaction;
use crate::model::vault::{ImportRun, TxProperty};
use crate::service::NewFile;
use crate::service::test_support::income;

fn pause() {
    std::thread::sleep(std::time::Duration::from_millis(5));
}

fn cash_id(vault: &UnlockedVault) -> String {
    vault
        .doc
        .stores
        .iter()
        .find(|store| store.parent_id.is_none())
        .unwrap()
        .id
        .clone()
}

/// The monthly rent automation booked for the same planned moment.
fn rent(vault: &mut UnlockedVault) -> String {
    let cash = cash_id(vault);
    let mut tx = income(&cash, "100", "rent");
    if let Transaction::Topup(topup) = &mut tx {
        topup.meta.origin = TxOrigin::Automation {
            automation_id: "rent".into(),
            planned_at: "2026-11-01".into(),
        };
    }
    vault.add_transaction(tx).unwrap().meta().id.clone()
}

fn enrichment(tx_id: &str) -> EnrichmentRecord {
    EnrichmentRecord {
        id: "enr-1".into(),
        tx_id: tx_id.into(),
        source: EnrichmentSource::TokenList,
        found: EnrichmentData::default(),
        status: EnrichmentStatus::Pending,
        created_at: "2026-11-01T00:00:00Z".into(),
    }
}

fn tx_ids(vault: &UnlockedVault) -> Vec<String> {
    vault
        .doc
        .transactions
        .iter()
        .map(|tx| tx.meta().id.clone())
        .collect()
}

fn reopen(vault: &UnlockedVault) -> UnlockedVault {
    UnlockedVault::open(vault.path.clone(), "pw").unwrap()
}

#[test]
fn repoint_tx_moves_every_known_reference() {
    let (_dir, mut vault, _right) = two_devices();
    let owner = EntityRef::new(EntityKind::Tx, "old");
    let goal = EntityRef::new(EntityKind::Goal, "old");
    vault
        .doc
        .attachments
        .push(crate::model::attachment::Attachment {
            id: "a1".into(),
            owner: owner.clone(),
            kind: crate::model::attachment::AttachmentKind::Link,
            title: "x".into(),
            url: Some("https://x.io".into()),
            mime: None,
            size: 0,
            sha256: None,
            created_at: "2026-11-01T00:00:00Z".into(),
            removed_at: None,
        });
    let mut goal_note = vault.doc.attachments[0].clone();
    goal_note.id = "a2".into();
    goal_note.owner = goal.clone();
    vault.doc.attachments.push(goal_note);
    vault.doc.tx_properties.push(TxProperty {
        tx_id: "old".into(),
        property_id: "flat".into(),
    });
    vault.doc.enrichments.push(enrichment("old"));
    vault.doc.shared_events.push(SharedEvent {
        id: "ev".into(),
        items: vec![EventItem {
            purchases: vec![Purchase {
                id: "p1".into(),
                tx_id: Some("old".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        settlements: vec![Settlement {
            id: "s1".into(),
            tx_id: Some("old".into()),
            ..settlement()
        }],
        ..Default::default()
    });
    vault.doc.import_runs.push(ImportRun {
        reconcile_tx_id: Some("old".into()),
        ..Default::default()
    });

    assert!(vault.repoint_tx("old", "new"));

    assert_eq!(vault.doc.attachments[0].owner.id, "new");
    assert_eq!(
        vault.doc.attachments[1].owner, goal,
        "a goal with the same id is not an operation"
    );
    assert_eq!(vault.doc.tx_properties[0].tx_id, "new");
    assert_eq!(vault.doc.enrichments[0].tx_id, "new");
    let event = &vault.doc.shared_events[0];
    assert_eq!(event.items[0].purchases[0].tx_id.as_deref(), Some("new"));
    assert_eq!(event.settlements[0].tx_id.as_deref(), Some("new"));
    assert_eq!(
        vault.doc.import_runs[0].reconcile_tx_id.as_deref(),
        Some("new")
    );
    assert!(!vault.repoint_tx("old", "new"), "nothing left to move");
}

fn settlement() -> Settlement {
    serde_json::from_value(serde_json::json!({
        "id": "s", "kind": "transfer", "participant_id": "p", "amount": "1", "at": "2026-11-01T00:00:00Z"
    }))
    .unwrap()
}

#[test]
fn a_linked_property_of_the_kept_copy_wins() {
    let (_dir, mut vault, _right) = two_devices();
    for (tx_id, property_id) in [("old", "flat"), ("new", "car")] {
        vault.doc.tx_properties.push(TxProperty {
            tx_id: tx_id.into(),
            property_id: property_id.into(),
        });
    }
    assert!(vault.repoint_tx("old", "new"));
    assert_eq!(
        vault.doc.tx_properties,
        vec![TxProperty {
            tx_id: "new".into(),
            property_id: "car".into()
        }]
    );
}

#[test]
fn dedupe_hands_the_losers_attachment_and_links_to_the_winner() {
    let (_dir, mut left, mut right) = two_devices();
    let winner = rent(&mut left);
    pause();
    let loser = rent(&mut right);
    let item = right
        .add_attachment_file(
            EntityRef::new(EntityKind::Tx, loser.clone()),
            NewFile {
                name: "rent.pdf",
                mime: None,
                bytes: b"%PDF-1.7 rent",
            },
        )
        .unwrap();
    right.doc.tx_properties.push(TxProperty {
        tx_id: loser.clone(),
        property_id: "flat".into(),
    });
    right.doc.enrichments.push(enrichment(&loser));
    right.persist().unwrap();

    settle(&mut left, &mut right);

    assert_eq!(tx_ids(&left), vec![winner.clone()]);
    assert_eq!(tx_ids(&right), vec![winner.clone()]);
    for vault in [&left, &right] {
        let found = vault
            .doc
            .attachments
            .iter()
            .find(|found| found.id == item.id)
            .unwrap();
        assert_eq!(found.owner.id, winner, "the attachment follows the winner");
    }
    assert_eq!(right.doc.tx_properties[0].tx_id, winner);
    assert_eq!(right.doc.enrichments[0].tx_id, winner);

    // The next open must not delete it nor its bytes.
    let file = right.attachments_dir().join(&item.id);
    let right = reopen(&right);
    assert!(
        right
            .doc
            .attachments
            .iter()
            .any(|found| found.id == item.id)
    );
    assert!(file.exists(), "the bytes survived the open");
    let (_, bytes) = right.read_attachment(&item.id).unwrap();
    assert_eq!(bytes, b"%PDF-1.7 rent");
}
