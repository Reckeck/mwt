//! The scenarios of the stores area: a wallet created, edited, moved,
//! archived or deleted, and the cash wallet every vault has. The tree rules
//! come from `money::stores` and are checked before anything is written; every
//! write ends in one commit and reaches a `Store` through
//! `tx.collection::<Store>()`. The forms (`StoreExtras`, `OpeningBalance`)
//! belong to the service layer, so the steps that read them live here.

use crate::error::{Error, Result};
use crate::kernel::tx::Tx;
use crate::model::store::{Binding, ExchangeProfile, PropertyInfo, Store, StoreKind, SubType};
use crate::model::transaction::{KindDetails, PriceStamp, TopupTx, Transaction, TxMeta};
use crate::model::vault::VaultDoc;
use crate::money::portfolio::StoreTreeNode;
use crate::money::stores::{
    ExtraClaims, StoreMove, check_deletable, check_root_creatable, check_store_extras,
    check_sub_account, default_sub_account, edited_binding, ensure_unique_name, plan_archive,
    plan_move, validate_placement,
};
use crate::money::tree::find_store;
use crate::money::valuation::initial_rule;
use crate::planning::emergency::prune_emergency_fund;
use crate::service::{CASH_WALLET_ID, StoreExtras, UnlockedVault};
use mwt_money::stores::{FirstCard, OpeningBalance};

/// Allowed destinations for moving a sub-account.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct StoreMoveTargets {
    /// Flat (no `children`), in tree order; the current parent is left out.
    pub parents: Vec<StoreTreeNode>,
    /// Whether it may become a top-level wallet (no root has its name).
    pub can_detach: bool,
}

/// Rebuild the holdings from the operations and write once: the commit of
/// every change that books or removes something the holdings derive from.
pub(crate) fn rebase_and_commit(vault: &mut UnlockedVault) -> Result<()> {
    crate::usecases::money::portfolio::recompute_holdings(vault);
    vault.transact(|tx| {
        tx.touch();
        Ok(())
    })
}

fn claims(extras: &StoreExtras) -> ExtraClaims<'_> {
    ExtraClaims {
        liability: extras.liability.as_ref(),
        contact_id: extras.contact_id.as_deref(),
        secured_by: extras.secured_by.as_deref(),
        custody: extras.custody.is_some(),
        property: extras.property.is_some(),
        provider: extras.provider.is_some(),
    }
}

/// Sub-account defaults for [`create_store`]: the kind's first template.
pub(crate) fn default_sub_extras(kind: StoreKind) -> StoreExtras {
    let (sub_type, binding) = default_sub_account(kind);
    StoreExtras {
        sub_type,
        binding,
        ..StoreExtras::default()
    }
}

/// The top-up that records a new wallet's opening balance.
pub(crate) fn opening_topup(store_id: &str, opening: OpeningBalance) -> Transaction {
    let mut meta = TxMeta::new("Opening balance");
    meta.categories = vec!["opening_balance".to_string()];
    if let Some(at) = opening.at.filter(|at| !at.trim().is_empty()) {
        meta.timestamp = at;
    }
    Transaction::Topup(TopupTx {
        meta,
        store_id: store_id.to_string(),
        asset: opening.asset,
        amount: opening.amount,
        fee: None,
        from: None,
        details: KindDetails::default(),
        price: PriceStamp::default(),
    })
}

pub fn create_store(
    vault: &mut UnlockedVault,
    name: &str,
    kind: StoreKind,
    color: &str,
    note: &str,
    parent_id: Option<String>,
) -> Result<Store> {
    let extras = match parent_id
        .as_deref()
        .and_then(|id| find_store(&vault.doc, id))
    {
        Some(parent) => default_sub_extras(parent.kind),
        None => StoreExtras::default(),
    };
    create_store_with(vault, name, kind, color, note, parent_id, extras)
}

/// Where and as what a new wallet sits, after every rule agreed.
struct Placement {
    parent: Option<Store>,
    kind: StoreKind,
    sub_type: Option<SubType>,
    binding: Binding,
}

fn place(
    doc: &VaultDoc,
    name: &str,
    kind: StoreKind,
    parent_id: Option<&str>,
    extras: &StoreExtras,
) -> Result<Placement> {
    validate_placement(doc, None, parent_id, name)?;
    let parent = parent_id.and_then(|id| find_store(doc, id)).cloned();
    // A sub-account is always of its wallet's kind.
    let kind = parent.as_ref().map_or(kind, |parent| parent.kind);
    let (sub_type, binding) = match &parent {
        Some(_) => {
            let (sub_type, binding) =
                check_sub_account(kind, extras.sub_type, extras.binding.as_ref())?;
            (Some(sub_type), binding)
        }
        None => {
            check_root_creatable(doc, kind)?;
            (None, Binding::Any)
        }
    };
    check_store_extras(doc, kind, sub_type, parent.is_none(), &claims(extras))?;
    Ok(Placement {
        parent,
        kind,
        sub_type,
        binding,
    })
}

/// The record of a new wallet from its form.
fn build_store(
    placed: Placement,
    (name, color, note): (&str, &str, &str),
    parent_id: Option<String>,
    extras: StoreExtras,
) -> Store {
    let Placement {
        parent,
        kind,
        sub_type,
        binding,
    } = placed;
    let mut store = Store::new(name, kind, color);
    store.note = note.trim().to_string();
    store.purpose = extras.purpose.unwrap_or_else(|| {
        sub_type
            .and_then(SubType::default_purpose)
            .or_else(|| parent.as_ref().map(|parent| parent.purpose))
            .unwrap_or_else(|| kind.default_purpose())
    });
    store.sub_type = sub_type;
    store.binding = binding;
    store.liability = extras.liability;
    store.contact_id = extras.contact_id;
    if parent.is_none() {
        store.custody = extras.custody.or(store.custody);
        store.property = extras
            .property
            .or_else(|| (kind == StoreKind::Property).then(PropertyInfo::default));
        // New objects start with the rule of their type (plan §11.4).
        if let Some(info) = store
            .property
            .as_mut()
            .filter(|_| kind == StoreKind::Property)
            && info.revalue.is_none()
        {
            info.revalue = initial_rule(info.category);
        }
        store.exchange = match (kind, extras.provider) {
            (StoreKind::Exchange, Some(provider)) => Some(ExchangeProfile {
                provider,
                import_interval_sec: 900,
                terminal: Default::default(),
                import: Default::default(),
            }),
            _ => None,
        };
    } else {
        store.custody = None;
    }
    store.secured_by = extras.secured_by;
    // Margin only means something on an exchange's futures account.
    store.margin = extras
        .margin
        .filter(|_| kind == StoreKind::Exchange && sub_type == Some(SubType::Futures));
    store.parent_id = parent_id;
    store
}

/// The new wallet and the opening balance to book on it, checked as a whole
/// before anything is written.
fn plan_store(
    doc: &VaultDoc,
    form: (&str, &str, &str),
    kind: StoreKind,
    parent_id: Option<String>,
    mut extras: StoreExtras,
) -> Result<(Store, Option<OpeningBalance>)> {
    let placed = place(doc, form.0, kind, parent_id.as_deref(), &extras)?;
    let opening = extras
        .opening
        .take()
        .filter(|opening| !opening.amount.is_zero());
    if opening.is_some() && matches!(placed.kind, StoreKind::Property | StoreKind::Liability) {
        return Err(Error::WalletRule(crate::error::WalletRule::NotApplicable));
    }
    Ok((build_store(placed, form, parent_id, extras), opening))
}

pub fn create_store_with(
    vault: &mut UnlockedVault,
    name: &str,
    kind: StoreKind,
    color: &str,
    note: &str,
    parent_id: Option<String>,
    extras: StoreExtras,
) -> Result<Store> {
    let (store, opening) = plan_store(&vault.doc, (name, color, note), kind, parent_id, extras)?;
    // The opening top-up is checked against the document that already has the
    // wallet, so the wallet goes in first and comes out again if the top-up
    // is refused: a bad opening balance leaves no half-created wallet behind.
    vault.run(|tx| tx.collection::<Store>().insert(store.clone()))?;
    if let Some(opening) = opening {
        let mut topup = opening_topup(&store.id, opening);
        if let Err(err) =
            crate::usecases::money::transactions::prepare_transaction(vault, &mut topup)
        {
            vault.run(|tx| tx.collection::<Store>().remove(&store.id).map(drop))?;
            return Err(err);
        }
        vault.run(|tx| {
            tx.doc_mut().transactions.push(topup);
            Ok(())
        })?;
    }
    rebase_and_commit(vault)?;
    Ok(store)
}

/// A new bank together with its first card: a card is a bank account, so a
/// bank is never left without one. The opening balance goes on the card. All
/// or nothing.
pub fn create_bank_with_card(
    vault: &mut UnlockedVault,
    name: &str,
    color: &str,
    note: &str,
    mut extras: StoreExtras,
    card: FirstCard,
) -> Result<Store> {
    let currency = card.currency.trim().to_uppercase();
    if currency.is_empty() {
        return Err(Error::InvalidInput("the card needs a currency".into()));
    }
    let opening = extras.opening.take();
    let backup = vault.doc.clone();
    let result = (|| -> Result<Store> {
        let bank = create_store_with(vault, name, StoreKind::Bank, color, note, None, extras)?;
        let card_name = if card.name.trim().is_empty() {
            format!("{name} {currency}")
        } else {
            card.name.trim().to_string()
        };
        let asset = crate::model::store::AssetRef::fiat(&currency, &currency);
        create_store_with(
            vault,
            &card_name,
            StoreKind::Bank,
            color,
            "",
            Some(bank.id.clone()),
            StoreExtras {
                sub_type: Some(SubType::Card),
                binding: Some(Binding::Asset(asset)),
                opening,
                ..StoreExtras::default()
            },
        )?;
        Ok(bank)
    })();
    if result.is_err() {
        vault.doc = backup;
        rebase_and_commit(vault)?;
    }
    result
}

/// The vault always has exactly one cash wallet; called on create and open.
/// Returns whether one had to be added.
pub fn ensure_cash_wallet(vault: &mut UnlockedVault, name: &str) -> Result<bool> {
    let has_cash = vault
        .doc
        .stores
        .iter()
        .any(|store| store.kind == StoreKind::Cash && store.parent_id.is_none());
    if has_cash {
        return Ok(false);
    }
    let name = if name.trim().is_empty() { "Cash" } else { name };
    let mut store = Store::new(name, StoreKind::Cash, "#16a34a");
    // A fixed id: two vaults made apart meet in one record, and an old
    // random-id wallet is folded into this one by the repair pass (R1).
    store.id = CASH_WALLET_ID.to_string();
    if ensure_unique_name(&vault.doc, None, None, &store.name).is_err() {
        store.name = format!("{} 2", store.name);
    }
    vault.transact(|tx| {
        tx.collection::<Store>().insert(store)?;
        // The cash wallet is listed first.
        tx.doc_mut().stores.rotate_right(1);
        Ok(true)
    })
}

/// What an edit changes on the store: the trimmed form and the checked binding.
struct StoreEdit {
    name: Option<String>,
    color: String,
    note: String,
    binding: Option<Binding>,
    extras: StoreExtras,
}

impl StoreEdit {
    fn apply(self, store: &mut Store) {
        let extras = self.extras;
        if let Some(name) = self.name {
            store.name = name;
        }
        store.color = self.color;
        store.note = self.note;
        if let Some(purpose) = extras.purpose {
            store.purpose = purpose;
        }
        if let Some(binding) = self.binding {
            store.binding = binding;
        }
        store.liability = extras.liability;
        store.contact_id = extras.contact_id;
        if store.parent_id.is_none() {
            if extras.custody.is_some() {
                store.custody = extras.custody;
            }
            if let Some(mut next) = extras.property {
                // The rule is set by its own command only.
                next.revalue = store
                    .property
                    .as_ref()
                    .and_then(|info| info.revalue.clone());
                store.property = Some(next);
            }
            if let Some(provider) = extras.provider {
                let profile = store.exchange.get_or_insert_with(|| ExchangeProfile {
                    provider,
                    import_interval_sec: 900,
                    terminal: Default::default(),
                    import: Default::default(),
                });
                profile.provider = provider;
            }
        }
        store.secured_by = extras.secured_by;
    }
}

fn plan_edit(
    doc: &VaultDoc,
    id: &str,
    (name, color, note): (&str, &str, &str),
    extras: StoreExtras,
) -> Result<StoreEdit> {
    let current = find_store(doc, id).ok_or_else(|| Error::Missing("store".into()))?;
    let is_root = current.parent_id.is_none();
    let name = (!name.trim().is_empty()).then(|| name.trim().to_string());
    if name.is_some() {
        ensure_unique_name(
            doc,
            Some(id),
            current.parent_id.as_deref(),
            name.as_deref().unwrap_or_default(),
        )?;
    }
    check_store_extras(
        doc,
        current.kind,
        current.sub_type,
        is_root,
        &claims(&extras),
    )?;
    let binding = edited_binding(
        doc,
        id,
        current.kind,
        current.sub_type,
        extras.binding.as_ref(),
    )?;
    Ok(StoreEdit {
        name,
        color: color.trim().to_string(),
        note: note.trim().to_string(),
        binding,
        extras,
    })
}

/// Edit a store's own fields. Its place in the tree changes only through
/// [`move_store`].
pub fn update_store(
    vault: &mut UnlockedVault,
    id: &str,
    name: &str,
    color: &str,
    note: &str,
    extras: StoreExtras,
) -> Result<Store> {
    let edit = plan_edit(&vault.doc, id, (name, color, note), extras)?;
    vault.transact(|tx| {
        let out = tx.collection::<Store>().update(id, |store| {
            edit.apply(store);
            store.clone()
        })?;
        prune_emergency_fund(tx.doc_mut());
        Ok(out)
    })
}

/// Re-hang a sub-account under another node, or make it a top-level wallet
/// with `parent_id = None`. A top-level wallet never moves (R6).
pub fn move_store(vault: &mut UnlockedVault, id: &str, parent_id: Option<&str>) -> Result<Store> {
    match plan_move(&vault.doc, id, parent_id)? {
        StoreMove::Stay(store) => Ok(*store),
        StoreMove::Reparent => vault.transact(|tx| {
            tx.collection::<Store>().update(id, |store| {
                store.parent_id = parent_id.map(str::to_string);
                store.clone()
            })
        }),
    }
}

/// Depth-first flat order without `children`: parent, then its subtree.
fn flatten_tree(nodes: Vec<StoreTreeNode>) -> Vec<StoreTreeNode> {
    let mut out = Vec::new();
    for mut node in nodes {
        let children = std::mem::take(&mut node.children);
        out.push(node);
        out.extend(flatten_tree(children));
    }
    out
}

/// Where `id` may be moved: every allowed parent (flat, without the current
/// one) and whether it may become a top-level wallet.
pub fn store_move_targets(vault: &UnlockedVault, id: &str) -> Result<StoreMoveTargets> {
    let store = find_store(&vault.doc, id).ok_or_else(|| Error::Missing("store".into()))?;
    if store.parent_id.is_none() {
        return Ok(StoreMoveTargets::default());
    }
    let parents = flatten_tree(crate::money::portfolio::store_tree(&vault.books()))
        .into_iter()
        .filter(|node| store.parent_id.as_deref() != Some(node.store.id.as_str()))
        .filter(|node| {
            validate_placement(&vault.doc, Some(id), Some(&node.store.id), &store.name).is_ok()
        })
        .collect();
    Ok(StoreMoveTargets {
        parents,
        can_detach: false,
    })
}

/// Archive (or restore) a store together with its whole subtree.
pub fn archive_store(vault: &mut UnlockedVault, id: &str, archived: bool) -> Result<()> {
    let affected = plan_archive(&vault.doc, id, archived)?;
    vault.transact(|tx| {
        for store_id in &affected {
            tx.collection::<Store>()
                .update(store_id, |store| store.archived = archived)?;
        }
        prune_emergency_fund(tx.doc_mut());
        Ok(())
    })
}

pub fn delete_store(vault: &mut UnlockedVault, id: &str) -> Result<()> {
    check_deletable(&vault.doc, id)?;
    vault.run(|tx: &mut Tx<'_>| {
        tx.collection::<Store>().remove(id)?;
        let doc = tx.doc_mut();
        doc.holdings.retain(|holding| holding.store_id != id);
        prune_emergency_fund(doc);
        Ok(())
    })?;
    rebase_and_commit(vault)
}

/// Merge one account into another: every operation moves to `target_id`, the
/// corrections keep their deltas, and the account goes. The same change as
/// `apply_store_change` with `MergeInto`.
pub fn merge(
    vault: &mut UnlockedVault,
    source_id: &str,
    target_id: &str,
) -> Result<crate::money::store_change::StoreChangePlan> {
    let input = crate::money::store_change::StoreChangeInput {
        store_id: source_id.to_string(),
        change: crate::money::store_change::StoreChange::MergeInto {
            target_id: target_id.to_string(),
        },
    };
    crate::usecases::money::store_change::apply_store_change(vault, &input)
}
