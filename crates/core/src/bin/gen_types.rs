//! Regenerates `src/shared/api/ipc.gen.d.ts` from the core IPC contract.
//!
//! Run via: `mise run gen-types`
//!
//! Every type enumerated in `collect!` below is exported with ts-rs
//! (`#[ts(export)]` respecting `TS_RS_EXPORT_DIR`) into the frontend's
//! `src/shared/api` directory, then flattened into a single ambient `.d.ts`.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = match std::env::var_os("TS_RS_EXPORT_DIR") {
        Some(raw) => PathBuf::from(raw),
        None => PathBuf::from("src/shared/api/.ts-tmp"),
    };
    fs::create_dir_all(&out_dir).expect("create export dir");

    let mut ok = true;
    macro_rules! export {
        ($($ty:ty),* $(,)?) => {
            $(ok = export_ty::<$ty>(&out_dir) && ok;)*
        };
    }

    export!(
        trade_core::model::store::StoreKind,
        trade_core::model::store::Store,
        trade_core::model::store::Holding,
        trade_core::model::transaction::Transaction,
        trade_core::model::transaction::TxKind,
        trade_core::model::transaction::TxMeta,
        trade_core::model::transaction::TxOrigin,
        trade_core::model::transaction::PriceStamp,
        trade_core::model::transaction::Fee,
        trade_core::model::transaction::AccountRef,
        trade_core::model::transaction::LegRole,
        trade_core::model::transaction::CashNote,
        trade_core::model::transaction::KindDetails,
        trade_core::model::transaction::TopupTx,
        trade_core::model::transaction::PaymentTx,
        trade_core::model::transaction::DebtEffect,
        trade_core::model::transaction::TransferTx,
        trade_core::model::transaction::ConvertSide,
        trade_core::model::transaction::ConvertTx,
        trade_core::model::transaction::TradeSide,
        trade_core::model::transaction::Liquidity,
        trade_core::model::transaction::TradeSpotTx,
        trade_core::model::transaction::FuturesDirection,
        trade_core::model::transaction::FuturesAction,
        trade_core::model::transaction::FuturesCategory,
        trade_core::model::transaction::TradeFuturesTx,
        trade_core::model::transaction::DefiAction,
        trade_core::model::transaction::DefiTx,
        trade_core::model::transaction::AssetSellTx,
        trade_core::model::store::LiabilityKind,
        trade_core::model::store::LiabilityTerms,
        trade_core::model::contact::ContactKind,
        trade_core::model::contact::DetailKind,
        trade_core::model::contact::ContactAccount,
        trade_core::model::contact::Contact,
        trade_core::model::category::CategoryGroup,
        trade_core::model::category::Category,
        trade_core::model::planning::Schedule,
        trade_core::model::planning::TransactionDraft,
        trade_core::model::planning::Automation,
        trade_core::model::planning::RejectReason,
        trade_core::model::planning::AutomationOutcome,
        trade_core::model::planning::AutomationRun,
        trade_core::model::planning::BudgetTarget,
        trade_core::model::planning::BudgetPeriod,
        trade_core::model::planning::Budget,
        trade_core::model::planning::Goal,
        trade_core::model::planning::Reminder,
        trade_core::model::planning::AlertCondition,
        trade_core::model::planning::PriceAlert,
        trade_core::model::planning::CategorizationRule,
        trade_core::model::planning::UtilityKind,
        trade_core::model::planning::UtilityItem,
        trade_core::model::planning::MeterReading,
        trade_core::model::enrichment::EnrichmentSource,
        trade_core::model::enrichment::EnrichmentData,
        trade_core::model::enrichment::EnrichmentStatus,
        trade_core::model::enrichment::EnrichmentRecord,
        trade_core::model::vault::DailyBudgetMode,
        trade_core::model::vault::ManualPrice,
        trade_core::model::vault::Settings,
        trade_core::appconfig::Theme,
        trade_core::appconfig::Locale,
        trade_core::appconfig::AppSettings,
        trade_core::appconfig::UpdateAppSettingsInput,
        trade_core::appconfig::SupportedCurrencies,
        trade_core::ipc_contract::CurrencyCatalogOutput,
        trade_core::model::vault::PricePoint,
        trade_core::model::vault::FxPoint,
        trade_core::model::vault::VaultDoc,
        trade_core::ipc_contract::VaultStatus,
        trade_core::ipc_contract::UnlockInput,
        trade_core::ipc_contract::CreateVaultInput,
        trade_core::ipc_contract::ChangePasswordInput,
        trade_core::ipc_contract::VaultInfo,
        trade_core::ipc_contract::CreateStoreInput,
        trade_core::ipc_contract::UpdateStoreInput,
        trade_core::ipc_contract::MoveStoreInput,
        trade_core::model::store::StoreScope,
        trade_core::ipc_contract::TransactionKinds,
        trade_core::ipc_contract::AssetIdent,
        trade_core::ipc_contract::TxCommonInput,
        trade_core::ipc_contract::FeeInput,
        trade_core::ipc_contract::ConvertSideInput,
        trade_core::ipc_contract::TradeSideInput,
        trade_core::ipc_contract::TransactionInput,
        trade_core::ipc_contract::UpdateTransactionInput,
        trade_core::ipc_contract::UpdateContactInput,
        trade_core::ipc_contract::ContactTurnoverInput,
        trade_core::ipc_contract::CreateCategoryInput,
        trade_core::ipc_contract::UpdateCategoryInput,
        trade_core::ipc_contract::CreateRuleInput,
        trade_core::ipc_contract::UpdateRuleInput,
        trade_core::ipc_contract::SetManualPriceInput,
        trade_core::ipc_contract::SetExchangeTargetsInput,
        trade_core::ipc_contract::RefreshResult,
        trade_core::ipc_contract::AssetPriceInput,
        trade_core::ipc_contract::CachedPrice,
        trade_core::ipc_contract::ConvertAmountInput,
        trade_core::ipc_contract::ConvertedAmount,
        trade_core::ipc_contract::AutomationInput,
        trade_core::ipc_contract::UpdateAutomationInput,
        trade_core::ipc_contract::BudgetInput,
        trade_core::ipc_contract::UpdateBudgetInput,
        trade_core::ipc_contract::GoalInput,
        trade_core::ipc_contract::UpdateGoalInput,
        trade_core::ipc_contract::ReminderInput,
        trade_core::ipc_contract::UpdateReminderInput,
        trade_core::ipc_contract::PriceAlertInput,
        trade_core::ipc_contract::UpdatePriceAlertInput,
        trade_core::ipc_contract::AutomationRejectedEvent,
        trade_core::service::AutomationPass,
        trade_core::service::BudgetStatus,
        trade_core::service::DailyBudget,
        trade_core::service::MonthSummary,
        trade_core::service::GoalProgress,
        trade_core::service::RealismIssue,
        trade_core::service::GoalCalendar,
        trade_core::service::GoalCalendarMonth,
        trade_core::service::UpcomingItem,
        trade_core::service::UpcomingKind,
        trade_core::service::UtilityMonth,
        trade_core::service::UtilityTrend,
        trade_core::service::UtilityPaymentDraft,
        trade_core::ipc_contract::UtilityInput,
        trade_core::ipc_contract::UpdateUtilityInput,
        trade_core::ipc_contract::MeterReadingInput,
        trade_core::service::TxLoss,
        trade_core::service::LossRow,
        trade_core::service::LossBucket,
        trade_core::service::LossReport,
        trade_core::service::FlowAmount,
        trade_core::service::FlowLink,
        trade_core::service::FlowSummary,
        trade_core::service::FxRow,
        trade_core::service::FxTable,
        trade_core::ipc_contract::LinkTransactionsInput,
        trade_core::service::ReportKind,
        trade_core::service::ReportFilter,
        trade_core::service::ReportMeta,
        trade_core::service::Report,
        trade_core::service::CashflowReport,
        trade_core::service::CashflowRow,
        trade_core::service::YearReport,
        trade_core::service::YearRow,
        trade_core::service::CategoriesReport,
        trade_core::service::CategoryGroupBlock,
        trade_core::service::CategoryRow,
        trade_core::service::NetworthReport,
        trade_core::service::NetworthPoint,
        trade_core::service::StoresReport,
        trade_core::service::StoreReportRow,
        trade_core::service::PnlReport,
        trade_core::service::PnlRow,
        trade_core::service::PortfolioReport,
        trade_core::service::PortfolioPoint,
        trade_core::service::InflationReport,
        trade_core::service::InflationPoint,
        trade_core::service::CalendarMonth,
        trade_core::service::CalendarDay,
        trade_core::service::DayStatus,
        trade_core::ipc_contract::UpdateSettingsInput,
        trade_core::calc::CalcMode,
        trade_core::calc::CalcInput,
        trade_core::calc::CalcResult,
        trade_core::pricing::AssetSearchHit,
        trade_core::service::AssetStat,
        trade_core::service::NamedValue,
        trade_core::service::StoreValue,
        trade_core::service::StorePosition,
        trade_core::service::StoreOverview,
        trade_core::service::StoreChildRow,
        trade_core::service::StoreCrumb,
        trade_core::service::StoreTreeNode,
        trade_core::service::StoreMoveTargets,
        trade_core::service::PortfolioSummary,
        trade_core::service::HistoryPoint,
        trade_core::service::PnlLeg,
        trade_core::service::RealizedPnl,
        trade_core::service::NetWorth,
        trade_core::service::FlowClass,
        trade_core::service::ContactDraft,
        trade_core::service::ContactTurnover,
        trade_core::service::TurnoverRow,
        trade_core::service::ContactBalance,
        trade_core::txinfo::TxInfo,
        trade_core::txinfo::Chain,
        trade_core::ipc_contract::InspectTxInput,
        trade_core::ipc_contract::InspectTxResult,
        trade_core::ipc_contract::TxFieldHint,
        trade_core::exchange::Category,
        trade_core::exchange::TradePrefill,
        trade_core::ipc_contract::BybitExecution,
        trade_core::ipc_contract::BybitExecutionsInput,
        trade_core::ipc_contract::BybitExecutionsResult,
        trade_core::ipc_contract::ExchangeKeyStatus,
        trade_core::ipc_contract::SetExchangeKeyInput,
        trade_core::ipc_contract::RecentVault,
        trade_core::ipc_contract::SelectVaultInput,
        trade_core::ipc_contract::ClearExchangeKeyInput,
        trade_core::model::vault::PendingImport,
        trade_core::ipc_contract::ImportBybitHistoryInput,
        trade_core::ipc_contract::ImportBybitHistoryOutput,
        trade_core::ipc_contract::ListPendingImportsOutput,
        trade_core::ipc_contract::ConfirmPendingImportsInput,
        trade_core::ipc_contract::ConfirmPendingImportsOutput,
        trade_core::ipc_contract::DiscardPendingImportsInput,
        trade_core::ipc_contract::DiscardPendingImportsOutput,
        trade_core::ipc_contract::DevDumpVaultOutput,
        trade_core::ipc_contract::GetCandlesInput,
        trade_core::ipc_contract::GetCandlesOutput,
        trade_core::ipc_contract::CandlePoint,
    );

    if !ok {
        eprintln!("gen-types: one or more types failed to export");
        std::process::exit(1);
    }

    let mut merged =
        String::from("// AUTO-GENERATED by `mise run gen-types`. Do not edit manually.\n");
    merged.push_str("type Amount = string; // rust_decimal money — always a decimal string\n");
    let mut files: Vec<OsString> = fs::read_dir(&out_dir)
        .expect("read export dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name())
        .filter(|name| name.to_string_lossy().ends_with(".ts"))
        .collect();
    files.sort();
    for file in files {
        let content = fs::read_to_string(out_dir.join(&file)).expect("read export file");
        for line in content.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("import") || trimmed.starts_with("// This file was generated") {
                continue;
            }
            let exported = trimmed
                .strip_prefix("export ")
                .map(str::to_string)
                .unwrap_or_else(|| line.to_string());
            merged.push_str(&exported);
            merged.push('\n');
        }
    }

    let path = Path::new("src")
        .join("shared")
        .join("api")
        .join("ipc.gen.d.ts");
    fs::create_dir_all(path.parent().expect("ipc.gen.d.ts parent")).expect("create src/shared/api");
    fs::write(&path, merged).expect("write ipc.gen.d.ts");
    let _ = fs::remove_dir_all(&out_dir);
    println!("gen-types: wrote {}", path.display());
}

fn export_ty<T: ts_rs::TS + 'static>(out_dir: &Path) -> bool {
    match T::export_all_to(out_dir) {
        Ok(()) => true,
        Err(err) => {
            eprintln!("gen-types: failed for {}: {err}", T::name());
            false
        }
    }
}
