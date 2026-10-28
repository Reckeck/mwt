//! Scenarios the tests of shared events share. The first is the «Шашлыки»
//! board of the design (shared-expenses 2): five people, 7 100 ₽, beer split
//! by units; the same numbers sit in the browser mock.

use std::collections::{BTreeMap, HashMap};

use crate::Amount;
use crate::ipc_contract::{
    SetSharedItemSplitInput, SharedEventDraft, SharedItemDraft, SharedParticipantDraft,
    SharedPurchaseDraft,
};
use crate::model::contact::Contact;
use crate::model::shared_event::{ItemPlan, ItemSplit, PriceMode};
use crate::model::store::{AssetRef, StoreKind};
use crate::service::UnlockedVault;
use crate::service::test_support;

pub use crate::model::shared_event::ME;
pub use crate::service::test_support::amount;

pub struct Scenario {
    pub _dir: tempfile::TempDir,
    pub vault: UnlockedVault,
    pub wallet: String,
    pub event: String,
    pub contacts: HashMap<String, String>,
}

impl Scenario {
    /// A participant's id in the event (the keys the scenario picked).
    pub fn person(&self, key: &str) -> String {
        key.to_string()
    }

    /// The contact behind a participant.
    pub fn contact(&self, key: &str) -> String {
        self.contacts[key].clone()
    }

    pub fn balance(&self) -> Amount {
        test_support::holding_of(&self.vault, &self.wallet, &rub())
    }
}

pub fn rub() -> AssetRef {
    AssetRef::fiat("RUB", "RUB")
}

fn new_vault() -> (
    tempfile::TempDir,
    UnlockedVault,
    String,
    HashMap<String, String>,
) {
    let (dir, mut vault) = test_support::empty_vault();
    vault.set_base_currency("rub");
    let wallet = test_support::store(&mut vault, "Тинькофф", StoreKind::Bank, None);
    vault
        .add_transaction(test_support::income_of(
            &wallet,
            rub(),
            "50000",
            "0.0109",
            "salary",
        ))
        .unwrap();
    let mut contacts = HashMap::new();
    for (key, name) in [("petya", "Петя"), ("vasya", "Вася"), ("anya", "Аня")] {
        let contact: Contact = vault
            .create_contact(crate::service::ContactDraft {
                name: name.into(),
                ..Default::default()
            })
            .unwrap();
        contacts.insert(key.to_string(), contact.id);
    }
    (dir, vault, wallet, contacts)
}

fn contact_person(contacts: &HashMap<String, String>, key: &str) -> SharedParticipantDraft {
    SharedParticipantDraft {
        id: Some(key.to_string()),
        contact_id: Some(contacts[key].clone()),
        ..Default::default()
    }
}

fn buy(
    wallet: &str,
    item: &str,
    buyer: &str,
    qty: &str,
    total: &str,
    new_title: Option<&str>,
) -> SharedPurchaseDraft {
    SharedPurchaseDraft {
        item_id: (!item.is_empty()).then(|| item.to_string()),
        new_item_title: new_title.map(str::to_string),
        buyer: buyer.into(),
        qty: amount(qty),
        amount: amount(total),
        currency: "rub".into(),
        price_mode: PriceMode::Total,
        store_id: Some(wallet.to_string()),
        at: Some("2026-10-03".into()),
        ..Default::default()
    }
}

fn planned(title: &str, unit: &str, qty: &str, total: &str) -> SharedItemDraft {
    SharedItemDraft {
        title: title.into(),
        unit: unit.into(),
        plan: Some(ItemPlan {
            qty: amount(qty),
            price: amount(total),
            price_mode: PriceMode::Total,
        }),
        ..Default::default()
    }
}

/// «Шашлыки · 2–4 ноя»: I organize; Petya, Vasya, Anya and the guest Olya.
/// I bought the meat (4 800) and the beer (8 for 1 800), Vasya the coal (500).
pub fn skewers_scenario() -> Scenario {
    let (dir, mut vault, wallet, contacts) = new_vault();
    let view = vault
        .create_shared_event(SharedEventDraft {
            title: "Шашлыки".into(),
            currency: "rub".into(),
            starts_on: Some("2026-11-02".into()),
            ends_on: Some("2026-11-04".into()),
            participants: vec![
                contact_person(&contacts, "petya"),
                contact_person(&contacts, "vasya"),
                contact_person(&contacts, "anya"),
                SharedParticipantDraft {
                    id: Some("olya".into()),
                    name: "Оля".into(),
                    ..Default::default()
                },
            ],
            ..SharedEventDraft::default()
        })
        .unwrap();
    let event = view.event.id;
    let mut items = HashMap::new();
    for draft in [
        planned("Мясо", "кг", "4", "4800"),
        planned("Пиво", "шт", "6", "1200"),
        planned("Овощи", "кг", "3", "900"),
        planned("Лаваш", "шт", "4", "300"),
        planned("Соус", "шт", "2", "400"),
        planned("Сок", "л", "6", "1800"),
    ] {
        let title = draft.title.clone();
        let view = vault.add_shared_item(&event, draft).unwrap();
        items.insert(title, view.event.items.last().unwrap().id.clone());
    }
    vault
        .add_shared_purchase(&event, buy(&wallet, &items["Мясо"], ME, "4", "4800", None))
        .unwrap();
    vault
        .add_shared_purchase(&event, buy(&wallet, &items["Пиво"], ME, "8", "1800", None))
        .unwrap();
    vault
        .add_shared_purchase(&event, buy(&wallet, "", "vasya", "1", "500", Some("Уголь")))
        .unwrap();
    vault
        .set_shared_item_split(SetSharedItemSplitInput {
            event_id: event.clone(),
            item_id: items["Пиво"].clone(),
            split: ItemSplit::ByUnits {
                units: BTreeMap::from([
                    ("petya".to_string(), amount("3")),
                    ("vasya".to_string(), amount("5")),
                ]),
            },
        })
        .unwrap();
    Scenario {
        _dir: dir,
        vault,
        wallet,
        event,
        contacts,
    }
}

/// «Подарок Маше · 5 ноя»: Anya organizes and bought the 7 400 gift; four of
/// us share it, my part is 1 850.
pub fn gift_scenario() -> Scenario {
    let (dir, mut vault, wallet, contacts) = new_vault();
    let view = vault
        .create_shared_event(SharedEventDraft {
            title: "Подарок Маше".into(),
            currency: "rub".into(),
            starts_on: Some("2026-11-05".into()),
            organizer: "anya".into(),
            participants: vec![
                contact_person(&contacts, "anya"),
                contact_person(&contacts, "petya"),
                contact_person(&contacts, "vasya"),
            ],
            ..SharedEventDraft::default()
        })
        .unwrap();
    let event = view.event.id;
    vault
        .add_shared_purchase(
            &event,
            buy(&wallet, "", "anya", "1", "7400", Some("Подарок")),
        )
        .unwrap();
    Scenario {
        _dir: dir,
        vault,
        wallet,
        event,
        contacts,
    }
}
