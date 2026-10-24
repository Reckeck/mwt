//! The demo wallets: cash, a bank with a card and a savings account, and a
//! crypto wallet with a stablecoin. Ids are fixed, so two seeds give the same
//! document.

use crate::Result;
use crate::model::chain::Chain;
use crate::model::store::{AssetRef, Binding, Custody, Store, StoreKind, SubType};
use crate::service::{StoreExtras, UnlockedVault};

use super::ids;

/// Where the demo money lives.
pub struct Wallets {
    pub cash: String,
    pub card: String,
    pub savings: String,
    pub token: String,
}

pub fn usd() -> AssetRef {
    AssetRef::fiat("USD", "Доллар США")
}

pub fn usdt() -> AssetRef {
    AssetRef::crypto("USDT", "Tether", Some("tether"), Chain::Ethereum)
}

/// Point the store `old` (and the children that name it) at the id `new`.
fn assign_id(vault: &mut UnlockedVault, old: &str, new: &str) {
    for store in &mut vault.doc.stores {
        if store.id == old {
            store.id = new.to_string();
        }
        if store.parent_id.as_deref() == Some(old) {
            store.parent_id = Some(new.to_string());
        }
    }
}

fn create(
    vault: &mut UnlockedVault,
    (name, kind, color): (&str, StoreKind, &str),
    parent: Option<&Store>,
    extras: StoreExtras,
    id: String,
) -> Result<Store> {
    let made = vault.create_store_with(
        name,
        kind,
        color,
        "",
        parent.map(|owner| owner.id.clone()),
        extras,
    )?;
    assign_id(vault, &made.id, &id);
    Ok(Store { id, ..made })
}

pub fn create_all(vault: &mut UnlockedVault) -> Result<Wallets> {
    vault.ensure_cash_wallet("Наличные")?;
    let cash = vault
        .doc
        .stores
        .iter()
        .find(|store| store.kind == StoreKind::Cash && store.parent_id.is_none())
        .map(|store| store.id.clone())
        .unwrap_or_default();
    let cash_id = ids::store(0);
    assign_id(vault, &cash, &cash_id);

    let bank = create(
        vault,
        ("Основной банк", StoreKind::Bank, "#facc15"),
        None,
        StoreExtras::default(),
        ids::store(1),
    )?;
    let card_extras = StoreExtras {
        sub_type: Some(SubType::Card),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    let card = create(
        vault,
        ("Карта", StoreKind::Bank, "#facc15"),
        Some(&bank),
        card_extras,
        ids::store(2),
    )?;
    let savings_extras = StoreExtras {
        sub_type: Some(SubType::Savings),
        binding: Some(Binding::Asset(usd())),
        ..StoreExtras::default()
    };
    let savings = create(
        vault,
        ("Накопления", StoreKind::Bank, "#22c55e"),
        Some(&bank),
        savings_extras,
        ids::store(3),
    )?;
    let ledger_extras = StoreExtras {
        custody: Some(Custody::Cold),
        ..StoreExtras::default()
    };
    let ledger = create(
        vault,
        ("Ledger", StoreKind::Crypto, "#3b82f6"),
        None,
        ledger_extras,
        ids::store(4),
    )?;
    let token_extras = StoreExtras {
        sub_type: Some(SubType::Token),
        binding: Some(Binding::Asset(usdt())),
        ..StoreExtras::default()
    };
    let token = create(
        vault,
        ("USDT", StoreKind::Crypto, "#3b82f6"),
        Some(&ledger),
        token_extras,
        ids::store(5),
    )?;
    Ok(Wallets {
        cash: cash_id,
        card: card.id,
        savings: savings.id,
        token: token.id,
    })
}
