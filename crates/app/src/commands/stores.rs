use tauri::State;

use trade_core::Amount;
use trade_core::calendar::{Calendar, now_secs};
use trade_core::ipc_contract::{
    CreateStoreInput, FundEnvelopeInput, FundGoalInput, MoveEnvelopeInput, MoveStoreInput,
    UpdateEnvelopeInput, UpdateStoreInput, VehicleMileageInput, VehicleServiceDoneInput,
    WalletCatalog,
};
use trade_core::model::ids::StoreId;
use trade_core::model::planning::Envelope;
use trade_core::model::store::{Store, StoreKind};
use trade_core::money::portfolio;
use trade_core::money::portfolio::{StoreOverview, StoreTreeNode};
use trade_core::money::store_change::{StoreChangeInput, StoreChangePlan};
use trade_core::planning::envelopes as envelope_reads;
use trade_core::planning::envelopes::{EnvelopeDraft, EnvelopeView};
use trade_core::service::StoreExtras;
use trade_core::usecases::household::{envelopes, vehicle};
use trade_core::usecases::money::rebase::{rebase_envelope_input, rebase_store_input};
use trade_core::usecases::money::stores::StoreMoveTargets;
use trade_core::usecases::money::{store_change, stores, valuation};

use crate::commands::AppState;
use crate::commands::app_settings::current_zone;
use crate::commands::{with_vault, with_vault_mut};
use crate::ipc::IpcError;
use crate::ipc::commands::ipc_commands;

/// What every wallet kind allows (sub-account types, limits, exchanges).
/// Pure: needs no vault.
#[tauri::command]
pub async fn get_wallet_catalog() -> Result<WalletCatalog, IpcError> {
    Ok(WalletCatalog::build())
}

#[tauri::command]
pub async fn list_stores(state: State<'_, AppState>) -> Result<Vec<Store>, IpcError> {
    with_vault(&state, |vault| Ok(vault.doc.stores.clone()))
}

#[tauri::command]
pub async fn create_store(
    input: CreateStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, IpcError> {
    let extras = StoreExtras {
        purpose: input.purpose,
        liability: input.liability,
        contact_id: input.contact_id,
        sub_type: input.sub_type,
        binding: input.binding,
        custody: input.custody,
        property: input.property,
        secured_by: input.secured_by,
        provider: input.provider,
        opening: input.opening,
        margin: input.margin,
    };
    with_vault_mut(&state, |vault| match input.card {
        Some(card) if input.kind == StoreKind::Bank && input.parent_id.is_none() => {
            stores::create_bank_with_card(
                vault,
                &input.name,
                &input.color,
                &input.note,
                extras,
                card,
            )
        }
        _ => stores::create_store_with(
            vault,
            &input.name,
            input.kind,
            &input.color,
            &input.note,
            input.parent_id,
            extras,
        ),
    })
}

#[tauri::command]
pub async fn update_store(
    input: UpdateStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase_store_input(&vault.doc, input)?;
        stores::update_store(
            vault,
            &input.id,
            &input.name,
            &input.color,
            &input.note,
            StoreExtras {
                purpose: input.purpose,
                liability: input.liability,
                contact_id: input.contact_id,
                sub_type: None,
                binding: input.binding,
                custody: input.custody,
                property: input.property,
                secured_by: input.secured_by,
                provider: input.provider,
                opening: None,
                margin: None,
            },
        )
    })
}

/// The wallet tree with own and rolled-up values (sidebar, wallets page).
#[tauri::command]
pub async fn get_store_tree(state: State<'_, AppState>) -> Result<Vec<StoreTreeNode>, IpcError> {
    with_vault(&state, |vault| Ok(portfolio::store_tree(&vault.books())))
}

#[tauri::command]
pub async fn move_store(
    input: MoveStoreInput,
    state: State<'_, AppState>,
) -> Result<Store, IpcError> {
    with_vault_mut(&state, |vault| {
        stores::move_store(vault, &input.id, input.parent_id.as_deref())
    })
}

/// Where a sub-account may be moved to.
#[tauri::command]
pub async fn store_move_targets(
    id: String,
    state: State<'_, AppState>,
) -> Result<StoreMoveTargets, IpcError> {
    with_vault(&state, |vault| stores::store_move_targets(vault, &id))
}

#[tauri::command]
pub async fn archive_store(
    id: String,
    archived: bool,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| stores::archive_store(vault, &id, archived))
}

#[tauri::command]
pub async fn delete_store(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| stores::delete_store(vault, &id))
}

/// One round trip for the whole wallet page header: store card, totals,
/// positions and realized PnL. `include_children` (default on) widens PnL and
/// transaction stats to the subtree.
#[tauri::command]
pub async fn get_store_overview(
    store_id: String,
    include_children: Option<bool>,
    state: State<'_, AppState>,
) -> Result<StoreOverview, IpcError> {
    with_vault(&state, |vault| {
        portfolio::store_overview(&vault.books(), &store_id, include_children.unwrap_or(true))
    })
}

// -- envelopes ---------------------------------------------------------------

/// Envelopes of one store, or of every store when `store_id` is absent.
#[tauri::command]
pub async fn list_envelopes(
    store_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<EnvelopeView>, IpcError> {
    with_vault(&state, |vault| {
        Ok(envelope_reads::envelopes(
            &vault.doc,
            store_id.map(StoreId::from),
        ))
    })
}

#[tauri::command]
pub async fn create_envelope(
    input: EnvelopeDraft,
    state: State<'_, AppState>,
) -> Result<Envelope, IpcError> {
    with_vault_mut(&state, |vault| envelopes::create_envelope(vault, input))
}

#[tauri::command]
pub async fn update_envelope(
    input: UpdateEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<Envelope, IpcError> {
    with_vault_mut(&state, |vault| {
        let input = rebase_envelope_input(&vault.doc, input)?;
        envelopes::update_envelope(vault, &input.id, input.draft)
    })
}

/// Earmark more (`amount > 0`) or release (`amount < 0`).
#[tauri::command]
pub async fn fund_envelope(
    input: FundEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<Envelope, IpcError> {
    with_vault_mut(&state, |vault| {
        envelopes::fund_envelope(vault, &input.id, input.amount)
    })
}

/// Earmark up to `amount` for a goal in a wallet; returns what was taken.
#[tauri::command]
pub async fn fund_goal(
    input: FundGoalInput,
    state: State<'_, AppState>,
) -> Result<Amount, IpcError> {
    with_vault_mut(&state, |vault| {
        envelopes::fund_goal(
            vault,
            &input.goal_id,
            &StoreId::from(input.store_id),
            input.amount,
        )
    })
}

#[tauri::command]
pub async fn move_between_envelopes(
    input: MoveEnvelopeInput,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| {
        envelopes::move_between_envelopes(vault, &input.from, &input.to, input.amount)
    })
}

#[tauri::command]
pub async fn delete_envelope(id: String, state: State<'_, AppState>) -> Result<(), IpcError> {
    with_vault_mut(&state, |vault| envelopes::delete_envelope(vault, &id))
}

/// What changing a wallet's kind, moving, detaching or merging an account
/// would do: what moves along, what is split off, which operations would no
/// longer fit. Writes nothing.
#[tauri::command]
pub async fn plan_store_change(
    input: StoreChangeInput,
    state: State<'_, AppState>,
) -> Result<StoreChangePlan, IpcError> {
    with_vault(&state, |vault| {
        Ok(store_change::plan_store_change(vault, &input))
    })
}

/// Make that change: refused while the plan has an error or a conflict. The
/// vault file is copied to `.bak` first.
#[tauri::command]
pub async fn apply_store_change(
    input: StoreChangeInput,
    state: State<'_, AppState>,
) -> Result<StoreChangePlan, IpcError> {
    with_vault_mut(&state, |vault| {
        store_change::apply_store_change(vault, &input)
    })
}

// -- cars ---------------------------------------------------------------------------

/// The odometer of a car, read today.
#[tauri::command]
pub async fn vehicle_update_mileage(
    input: VehicleMileageInput,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Store, IpcError> {
    let today = Calendar::new(&current_zone(&app)).date_of(now_secs());
    with_vault_mut(&state, |vault| {
        vehicle::vehicle_update_mileage(vault, &StoreId::from(input.store_id), input.km, today)
    })
}

/// A car's service done: the next one moves by its intervals.
#[tauri::command]
pub async fn vehicle_service_done(
    input: VehicleServiceDoneInput,
    state: State<'_, AppState>,
) -> Result<Store, IpcError> {
    let date = chrono::NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
        .map_err(|_| IpcError::from(trade_core::Error::InvalidInput("date".into())))?;
    with_vault_mut(&state, |vault| {
        vehicle::vehicle_service_done(vault, &StoreId::from(input.store_id), date, input.km)
    })
}

/// Set or switch off the automatic revaluation of a property.
#[tauri::command]
pub async fn property_set_revalue(
    store_id: String,
    rule: Option<trade_core::model::store::RevalueRule>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), IpcError> {
    let zone = current_zone(&app);
    with_vault_mut(&state, |vault| {
        valuation::set_revalue_rule(vault, &StoreId::from(store_id), rule, &zone)
    })
}

ipc_commands!(stores_commands, STORES_NAMES, commands::stores: [get_wallet_catalog, list_stores, create_store, update_store, get_store_tree, move_store, store_move_targets, archive_store, delete_store, get_store_overview, list_envelopes, create_envelope, update_envelope, fund_envelope, fund_goal, move_between_envelopes, delete_envelope, plan_store_change, apply_store_change, vehicle_update_mileage, vehicle_service_done, property_set_revalue]);
