use crate::household::contacts::ContactDraft;
use crate::household::shared_events::view::PersonKind;
use crate::ipc_contract::{
    SetSharedItemSplitInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
};
use crate::model::contact::{ContactAccount, DetailKind};
use crate::model::shared_event::{GuestDetails, ItemSplit, ME, SharedEventStatus};
use crate::model::vault::VaultDoc;
use crate::service::UnlockedVault;
use crate::service::test_support::*;
use crate::storage::{SnapshotFormat, VaultFile, decode, encode};

fn draft(title: &str) -> SharedEventDraft {
    SharedEventDraft {
        title: title.into(),
        currency: "rub".into(),
        ..SharedEventDraft::default()
    }
}

fn guest(name: &str) -> SharedParticipantDraft {
    SharedParticipantDraft {
        name: name.into(),
        ..SharedParticipantDraft::default()
    }
}

#[test]
fn an_event_is_created_renamed_and_deleted() {
    let (_dir, mut vault) = empty_vault();
    let view = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        SharedEventDraft {
            starts_on: Some("2026-11-02".into()),
            ends_on: Some("2026-11-04".into()),
            participants: vec![guest("Оля")],
            ..draft("  Шашлыки ")
        },
    )
    .unwrap();
    assert_eq!(view.event.title, "Шашлыки");
    assert_eq!(view.event.currency, "rub");
    assert_eq!(view.event.organizer, ME);
    assert!(view.event.i_participate);
    assert_eq!(view.event.rounding_step, amount("1"));
    assert_eq!(view.status, SharedEventStatus::Planning);
    // me and the guest
    assert_eq!(view.people.len(), 2);

    let id = view.event.id;
    let renamed = crate::usecases::household::shared_events::update_shared_event(
        &mut vault,
        &id,
        SharedEventDraft {
            starts_on: Some("2026-11-02".into()),
            ends_on: Some("2026-11-02".into()),
            ..draft("Мангал")
        },
    )
    .unwrap();
    assert_eq!(renamed.event.title, "Мангал");
    // an end equal to the start is one day
    assert_eq!(renamed.event.ends_on, None);
    assert_eq!(
        crate::household::shared_events::view::list_shared_events(&vault.doc, false).len(),
        1
    );
    crate::usecases::household::shared_events::delete_shared_event(&mut vault, &id).unwrap();
    assert!(crate::household::shared_events::view::list_shared_events(&vault.doc, true).is_empty());
    assert!(
        crate::usecases::household::shared_events::delete_shared_event(&mut vault, &id).is_err()
    );
}

#[test]
fn dates_are_checked() {
    let (_dir, mut vault) = empty_vault();
    let bad = |starts: Option<&str>, ends: Option<&str>| SharedEventDraft {
        starts_on: starts.map(str::to_string),
        ends_on: ends.map(str::to_string),
        ..draft("Trip")
    };
    let before = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        bad(Some("2026-11-04"), Some("2026-11-02")),
    )
    .unwrap_err();
    assert_eq!(before.code(), "INVALID_INPUT");
    assert!(
        crate::usecases::household::shared_events::create_shared_event(
            &mut vault,
            bad(None, Some("2026-11-02"))
        )
        .is_err()
    );
    assert!(
        crate::usecases::household::shared_events::create_shared_event(
            &mut vault,
            bad(Some("2.11.2026"), None)
        )
        .is_err()
    );
    assert!(vault.doc.shared_events.is_empty());
    // dates are optional
    assert!(
        crate::usecases::household::shared_events::create_shared_event(&mut vault, bad(None, None))
            .is_ok()
    );
}

#[test]
fn the_list_is_ordered_by_the_event_date() {
    let (_dir, mut vault) = empty_vault();
    for (title, day) in [
        ("A", Some("2026-11-02")),
        ("B", Some("2026-12-01")),
        ("C", None),
    ] {
        crate::usecases::household::shared_events::create_shared_event(
            &mut vault,
            SharedEventDraft {
                starts_on: day.map(str::to_string),
                ..draft(title)
            },
        )
        .unwrap();
    }
    // «C» has no date: it sorts by the day it was made
    vault.doc.shared_events[2].created_at = "2026-11-15T10:00:00Z".into();
    let titles: Vec<String> =
        crate::household::shared_events::view::list_shared_events(&vault.doc, false)
            .into_iter()
            .map(|row| row.title)
            .collect();
    assert_eq!(titles, ["B", "C", "A"]);
}

#[test]
fn participants_are_contacts_or_guests() {
    let (_dir, mut vault) = empty_vault();
    let petya = crate::usecases::household::contacts::create_contact(
        &mut vault,
        ContactDraft {
            name: "Петя".into(),
            ..ContactDraft::default()
        },
    )
    .unwrap();
    let view =
        crate::usecases::household::shared_events::create_shared_event(&mut vault, draft("Event"))
            .unwrap();
    let id = view.event.id;
    crate::usecases::household::shared_events::add_shared_participant(
        &mut vault,
        &id,
        SharedParticipantDraft {
            contact_id: Some(petya.id.clone()),
            ..SharedParticipantDraft::default()
        },
    )
    .unwrap();
    // the same contact twice, and a nameless guest
    assert!(
        crate::usecases::household::shared_events::add_shared_participant(
            &mut vault,
            &id,
            SharedParticipantDraft {
                contact_id: Some(petya.id.clone()),
                ..SharedParticipantDraft::default()
            }
        )
        .is_err()
    );
    assert!(
        crate::usecases::household::shared_events::add_shared_participant(
            &mut vault,
            &id,
            guest("  ")
        )
        .is_err()
    );
    let view = crate::usecases::household::shared_events::add_shared_participant(
        &mut vault,
        &id,
        guest("Оля"),
    )
    .unwrap();
    let kinds: Vec<PersonKind> = view.people.iter().map(|row| row.kind).collect();
    assert_eq!(
        kinds,
        [PersonKind::Me, PersonKind::Contact, PersonKind::Guest]
    );
    assert_eq!(view.people[1].name, "Петя");
}

#[test]
fn a_guest_with_details_becomes_a_contact_with_them() {
    let (_dir, mut vault) = empty_vault();
    let view =
        crate::usecases::household::shared_events::create_shared_event(&mut vault, draft("Event"))
            .unwrap();
    let id = view.event.id;
    let view = crate::usecases::household::shared_events::add_shared_participant(
        &mut vault,
        &id,
        SharedParticipantDraft {
            name: "Оля".into(),
            details: Some(GuestDetails {
                phone: "+7 912 345-67-89".into(),
                messenger: "@olya".into(),
                requisites: vec![ContactAccount {
                    label: "Карта".into(),
                    kind: DetailKind::Card,
                    value: "2200 0000 0000 4411".into(),
                    ..ContactAccount::default()
                }],
                note: "с работы".into(),
            }),
            ..SharedParticipantDraft::default()
        },
    )
    .unwrap();
    let guest_id = view.people[1].id.clone();
    assert!(view.people[1].has_details);
    let contact = crate::usecases::household::shared_events::make_shared_participant_contact(
        &mut vault, &id, &guest_id,
    )
    .unwrap();
    assert_eq!(contact.name, "Оля");
    assert_eq!(contact.note, "с работы");
    assert_eq!(contact.accounts.len(), 3);
    assert!(contact.accounts.iter().all(|account| account.active));
    assert!(
        contact
            .accounts
            .iter()
            .any(|account| account.kind == DetailKind::Card)
    );
    let view = crate::household::shared_events::view::shared_event_view(&vault.doc, &id).unwrap();
    assert_eq!(view.people[1].kind, PersonKind::Contact);
    assert_eq!(
        view.people[1].contact_id.as_deref(),
        Some(contact.id.as_str())
    );
    assert!(!view.people[1].has_details);
    // already a contact
    assert!(
        crate::usecases::household::shared_events::make_shared_participant_contact(
            &mut vault, &id, &guest_id
        )
        .is_err()
    );
}

#[test]
fn a_person_with_money_in_the_event_cannot_be_removed() {
    let (_dir, mut vault) = empty_vault();
    let view = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        SharedEventDraft {
            participants: vec![guest("Оля"), guest("Вася")],
            organizer: String::new(),
            ..draft("Event")
        },
    )
    .unwrap();
    let id = view.event.id.clone();
    let olya = view.people[1].id.clone();
    let vasya = view.people[2].id.clone();
    crate::usecases::household::shared_events::add_shared_item(
        &mut vault,
        &id,
        SharedItemDraft {
            title: "Хлеб".into(),
            ..SharedItemDraft::default()
        },
    )
    .unwrap();
    vault.find_shared_event_mut(&id).unwrap().items[0]
        .purchases
        .push(crate::model::shared_event::Purchase {
            id: "p".into(),
            buyer: vasya.clone(),
            qty: amount("1"),
            amount: amount("100"),
            currency: "rub".into(),
            price_mode: crate::model::shared_event::PriceMode::Total,
            rate_to_event: amount("1"),
            ..Default::default()
        });
    assert!(
        crate::usecases::household::shared_events::remove_shared_participant(
            &mut vault, &id, &vasya
        )
        .is_err()
    );
    crate::usecases::household::shared_events::remove_shared_participant(&mut vault, &id, &olya)
        .unwrap();
}

#[test]
fn items_are_planned_cancelled_and_deleted() {
    let (_dir, mut vault) = empty_vault();
    let id =
        crate::usecases::household::shared_events::create_shared_event(&mut vault, draft("Event"))
            .unwrap()
            .event
            .id;
    let plan = crate::model::shared_event::ItemPlan {
        qty: amount("6"),
        price: amount("200"),
        price_mode: crate::model::shared_event::PriceMode::PerUnit,
    };
    let view = crate::usecases::household::shared_events::add_shared_item(
        &mut vault,
        &id,
        SharedItemDraft {
            title: "Пиво".into(),
            plan: Some(plan.clone()),
            ..SharedItemDraft::default()
        },
    )
    .unwrap();
    assert_eq!(view.totals.planned, amount("1200"));
    assert_eq!(view.event.items[0].unit, "шт");
    let item_id = view.event.items[0].id.clone();
    let view = crate::usecases::household::shared_events::update_shared_item(
        &mut vault,
        &id,
        &item_id,
        SharedItemDraft {
            title: "Пиво".into(),
            cancelled: true,
            plan: Some(plan),
            ..SharedItemDraft::default()
        },
    )
    .unwrap();
    // a cancelled item is out of the totals
    assert_eq!(view.totals.planned, amount("0"));
    assert!(
        crate::usecases::household::shared_events::add_shared_item(
            &mut vault,
            &id,
            SharedItemDraft::default()
        )
        .is_err()
    );
    crate::usecases::household::shared_events::delete_shared_item(&mut vault, &id, &item_id)
        .unwrap();
    assert!(
        crate::household::shared_events::view::shared_event_view(&vault.doc, &id)
            .unwrap()
            .event
            .items
            .is_empty()
    );
}

#[test]
fn a_split_names_only_people_of_the_event() {
    let (_dir, mut vault) = empty_vault();
    let view = crate::usecases::household::shared_events::create_shared_event(
        &mut vault,
        SharedEventDraft {
            participants: vec![guest("Оля")],
            ..draft("Event")
        },
    )
    .unwrap();
    let id = view.event.id.clone();
    let olya = view.people[1].id.clone();
    let item_id = crate::usecases::household::shared_events::add_shared_item(
        &mut vault,
        &id,
        SharedItemDraft {
            title: "Пиво".into(),
            ..SharedItemDraft::default()
        },
    )
    .unwrap()
    .event
    .items[0]
        .id
        .clone();
    let set = |vault: &mut UnlockedVault, split: ItemSplit| {
        crate::usecases::household::shared_events::set_shared_item_split(
            vault,
            SetSharedItemSplitInput {
                event_id: id.clone(),
                item_id: item_id.clone(),
                split,
            },
        )
    };
    assert!(
        set(
            &mut vault,
            ItemSplit::Equal {
                among: vec!["ghost".into()]
            }
        )
        .is_err()
    );
    set(
        &mut vault,
        ItemSplit::Equal {
            among: vec![olya.clone()],
        },
    )
    .unwrap();
    let units = std::collections::BTreeMap::from([(olya, amount("3"))]);
    set(&mut vault, ItemSplit::ByUnits { units }).unwrap();
}

/// The vault of a build before shared events: the document without the field.
fn old_document_json() -> Vec<u8> {
    let mut value = serde_json::to_value(VaultDoc::default()).unwrap();
    value.as_object_mut().unwrap().remove("shared_events");
    value["transactions"] = serde_json::json!([]);
    serde_json::to_vec(&value).unwrap()
}

#[test]
fn an_old_vault_without_shared_events_opens_keeps_its_data_and_reads_again() {
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let json: serde_json::Value = serde_json::from_slice(&old_document_json()).unwrap();
        assert!(json.get("shared_events").is_none());
        let old_bytes = match fmt {
            SnapshotFormat::Json => old_document_json(),
            _ => rmp_serde::to_vec_named(&json).unwrap(),
        };
        let doc = decode(&old_bytes, fmt).unwrap();
        assert!(doc.shared_events.is_empty());
        assert_eq!(doc.version, crate::VAULT_FORMAT_VERSION);

        // through a real file: write, open, add an event, save, open again
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.wallet");
        VaultFile::save(&path, "pw", &doc, fmt).unwrap();
        let mut vault = UnlockedVault::open(path.clone(), "pw").unwrap();
        let stores = vault.doc.stores.len();
        let id = crate::usecases::household::shared_events::create_shared_event(
            &mut vault,
            draft("После"),
        )
        .unwrap()
        .event
        .id;
        drop(vault);
        let reopened = UnlockedVault::open(path, "pw").unwrap();
        assert_eq!(reopened.doc.stores.len(), stores);
        assert_eq!(reopened.doc.shared_events.len(), 1);
        assert_eq!(reopened.doc.shared_events[0].id, id);
        // and the encoded form round-trips
        let bytes = encode(&reopened.doc, fmt).unwrap();
        assert_eq!(decode(&bytes, fmt).unwrap().shared_events.len(), 1);
    }
}
