//! Test fixtures: a vault document with every persisted type filled in.

use crate::Amount;
use crate::model::category::CategoryGroup;
use crate::model::chain::Chain;
use crate::model::contact::{Contact, ContactAccount, ContactKind, DetailKind};
use crate::model::enrichment::{
    EnrichmentData, EnrichmentRecord, EnrichmentSource, EnrichmentStatus,
};
use crate::model::planning::{
    AlertCondition, AmountMode, AutomationPreset, AutomationSpec, Budget, BudgetPeriod,
    BudgetTarget, CategorizationRule, Goal, MeterReading, PriceAlert, RejectReason, Reminder,
    Schedule, TransactionDraft, UtilityItem, UtilityKind, WorkdayShift,
};
use crate::model::store::{AssetRef, LiabilityKind, LiabilityTerms, Store, StoreKind, StoreScope};
use crate::model::transaction::*;
use crate::model::vault::{ManualPrice, PendingImport, VaultDoc};
use crate::workflow::model::{RunOutcome, Workflow, WorkflowRun};

fn amount(raw: &str) -> Amount {
    Amount::parse(raw).unwrap()
}

fn usd() -> AssetRef {
    AssetRef::fiat("USD", "US Dollar")
}

fn stamp() -> PriceStamp {
    PriceStamp {
        price_usd: Some(amount("1")),
        price_base: Some(amount("90")),
        fx_override: None,
    }
}

fn fee() -> Option<Fee> {
    Some(Fee {
        asset: usd(),
        amount: amount("0.5"),
        price: stamp(),
    })
}

fn meta(comment: &str) -> TxMeta {
    let mut meta = TxMeta::new(comment);
    meta.flow_id = Some("flow".into());
    meta.contact_id = Some("contact".into());
    meta.origin = TxOrigin::Automation {
        automation_id: "auto".into(),
        planned_at: "2026-01-01T09:00:00Z".into(),
    };
    meta
}

fn topup() -> TopupTx {
    TopupTx {
        meta: meta("salary"),
        store_id: "bank".into(),
        asset: usd(),
        amount: amount("1000"),
        fee: fee(),
        from: Some(AccountRef::Contact {
            contact_id: "contact".into(),
            account_id: Some("acc".into()),
        }),
        details: KindDetails {
            cash_breakdown: Some(vec![CashNote {
                denomination: amount("100"),
                count: 10,
            }]),
            unit_price: Some(amount("1")),
            network: Some(Chain::Ethereum),
            tx_hash: Some("0xabc".into()),
            source_store_id: Some("card".into()),
            for_month: Some("2026-09".into()),
        },
        price: stamp(),
    }
}

fn payment() -> PaymentTx {
    PaymentTx {
        meta: meta("rent"),
        store_id: "bank".into(),
        asset: usd(),
        amount: amount("500"),
        fee: None,
        to: Some(AccountRef::Raw("landlord".into())),
        details: KindDetails::default(),
        price: PriceStamp::default(),
    }
}

fn transfer() -> TransferTx {
    TransferTx {
        meta: meta("lend"),
        from: AccountRef::Store("bank".into()),
        to: AccountRef::Contact {
            contact_id: "contact".into(),
            account_id: None,
        },
        asset: usd(),
        amount: amount("50"),
        fee: fee(),
        network: Some(Chain::Tron),
        tx_hash: Some("hash".into()),
        debt: Some(DebtEffect::Lend),
        price: stamp(),
    }
}

fn convert() -> ConvertTx {
    ConvertTx {
        meta: meta("fx"),
        from: ConvertSide {
            account: AccountRef::Store("bank".into()),
            asset: AssetRef::fiat("RUB", "Ruble"),
            amount: amount("9000"),
            price: stamp(),
        },
        to: ConvertSide {
            account: AccountRef::Store("usd".into()),
            asset: usd(),
            amount: amount("100"),
            price: stamp(),
        },
        fee: fee(),
        rate_override: Some(amount("90")),
    }
}

/// One of each transaction variant.
pub fn every_transaction() -> Vec<Transaction> {
    vec![
        Transaction::Topup(topup()),
        Transaction::Payment(payment()),
        Transaction::Transfer(transfer()),
        Transaction::Convert(convert()),
        Transaction::TradeSpot(TradeSpotTx {
            meta: meta("dip"),
            store_id: "ex".into(),
            sold: TradeSide {
                asset: usd(),
                qty: amount("1000"),
                price: stamp(),
            },
            bought: TradeSide {
                asset: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
                qty: amount("0.01"),
                price: stamp(),
            },
            fee: fee(),
            liquidity: Some(Liquidity::Maker),
            order_id: Some("ord".into()),
        }),
        Transaction::TradeFutures(TradeFuturesTx {
            meta: meta("perp"),
            store_id: "ex".into(),
            contract: AssetRef::perp("bybit", "BTCUSDT"),
            settle: AssetRef::fiat("USDT", "Tether"),
            direction: FuturesDirection::Short,
            action: FuturesAction::Close,
            quantity: amount("0.1"),
            fill_price: amount("60000"),
            leverage: Some(amount("5")),
            fee: fee(),
            closed_pnl: Some(amount("-12.5")),
            order_id: None,
            category: FuturesCategory::Inverse,
            price: stamp(),
        }),
        Transaction::Defi(DefiTx {
            meta: meta("stake"),
            store_id: "hot".into(),
            action: DefiAction::AddLiquidity,
            asset: usd(),
            amount: amount("10"),
            protocol: Some("uni".into()),
            pool_id: Some("pool".into()),
            network: Some(Chain::Base),
            tx_hash: None,
            fee: None,
            price: stamp(),
        }),
        Transaction::AssetSell(AssetSellTx {
            meta: meta("car"),
            store_id: "things".into(),
            asset: AssetRef::fiat("CAR", "Car"),
            unit_price: amount("15000"),
            quantity: amount("1"),
            currency: usd(),
            proceeds_to: Some("bank".into()),
            fee: fee(),
            price: stamp(),
        }),
        Transaction::Adjustment(AdjustmentTx {
            meta: meta("counted"),
            store_id: "bank".into(),
            lines: vec![AdjustmentLine {
                asset: usd(),
                before: amount("100"),
                after: amount("95.5"),
                price: stamp(),
            }],
            fee: None,
        }),
    ]
}

/// A vault document with every collection non-empty and every enum variant
/// that carries data used at least once.
pub fn full_doc() -> VaultDoc {
    let mut doc = VaultDoc::default();
    let mut loan = Store::new("Card", StoreKind::Liability, "#000");
    loan.liability = Some(LiabilityTerms {
        kind: LiabilityKind::CreditCard,
        rate: Some(amount("24.9")),
        limit: Some(amount("100000")),
        due_day: Some(25),
        monthly_payment: Some(amount("5000")),
    });
    loan.contact_id = Some("contact".into());
    let mut child = Store::new("Spot", StoreKind::Exchange, "#111");
    child.parent_id = Some(loan.id.clone());
    doc.stores = vec![loan, child, Store::new("Flat", StoreKind::Property, "#222")];
    doc.transactions = every_transaction();
    doc.settings.budget_currency = "thb".into();
    doc.contacts = vec![Contact {
        id: "contact".into(),
        name: "skycapital".into(),
        kind: ContactKind::Exchanger,
        accounts: vec![ContactAccount {
            id: "acc".into(),
            group: Some("Bank".into()),
            label: "Main".into(),
            kind: DetailKind::CryptoAddress,
            value: "T123".into(),
            currency: Some("usdt".into()),
            network: Some(Chain::Tron),
            active: true,
        }],
        note: "note".into(),
        archived: false,
        created_at: "2026-01-01T00:00:00Z".into(),
        group: "P2P".into(),
    }];
    let schedules = [
        Schedule::Monthly { day: 31 },
        Schedule::Weekly { weekday: 0 },
        Schedule::EveryNDays { days: 14 },
    ];
    let drafts = [
        TransactionDraft::Topup(topup()),
        TransactionDraft::Payment(payment()),
        TransactionDraft::Transfer(transfer()),
        TransactionDraft::Convert(convert()),
    ];
    let shifts = [
        WorkdayShift::Back,
        WorkdayShift::None,
        WorkdayShift::Forward,
    ];
    // Automations are workflows built from their form.
    doc.workflows = drafts
        .into_iter()
        .zip(schedules.iter().cycle())
        .zip(shifts.iter().cycle())
        .enumerate()
        .map(|(index, ((template, schedule), shift))| {
            let spec = AutomationSpec {
                time: None,
                asks: false,
                for_month_offset: None,
                preset: (index == 0).then_some(AutomationPreset::Salary),
                schedule: *schedule,
                starts_at: "2026-01-01T09:00:00Z".into(),
                amount_mode: if matches!(template, TransactionDraft::Convert(_)) {
                    AmountMode::Fixed
                } else {
                    AmountMode::Varies
                },
                template,
                workday_shift: *shift,
                property_id: None,
            };
            Workflow {
                id: format!("auto-{index}"),
                name: "Rent".into(),
                description: String::new(),
                active: true,
                graph: serde_json::to_string(&crate::workflow::automation::automation_graph(&spec))
                    .unwrap(),
                created_at: "2026-01-01T00:00:00Z".into(),
                updated_at: "2026-01-01T00:00:00Z".into(),
                react_to_workflows: false,
                max_amount: None,
                versions: Vec::new(),
                tests: Vec::new(),
                draft: None,
                automation: Some(spec),
            }
        })
        .collect();
    let outcomes = [
        RunOutcome::Done,
        RunOutcome::Rejected {
            reason: RejectReason::InsufficientFunds {
                store_id: "bank".into(),
                asset: usd(),
                required: amount("25000"),
                available: amount("8000"),
            },
            trigger_node: "t".into(),
            planned_at: Some("2026-01-15T09:00:00Z".into()),
        },
        RunOutcome::Rejected {
            reason: RejectReason::CreditLimitExceeded {
                store_id: "card".into(),
                limit: amount("1000"),
                required: amount("1200"),
            },
            trigger_node: "t".into(),
            planned_at: None,
        },
        RunOutcome::Rejected {
            reason: RejectReason::Invalid {
                message: "archived".into(),
            },
            trigger_node: "t".into(),
            planned_at: Some("2026-01-15T09:00:00Z".into()),
        },
        RunOutcome::Dropped {
            reason: "skipped".into(),
        },
    ];
    doc.workflow_runs = outcomes
        .into_iter()
        .enumerate()
        .map(|(index, outcome)| WorkflowRun {
            id: format!("run-{index}"),
            workflow_id: "auto-0".into(),
            trigger: "schedule 2026-01-15T09:00:00Z".into(),
            started_at: "2026-01-15T09:00:05Z".into(),
            created: if outcome == RunOutcome::Done {
                vec!["tx".into()]
            } else {
                Vec::new()
            },
            outcome,
            queued: 0,
            trace: Vec::new(),
            dry_run: false,
        })
        .collect();
    let targets = [
        BudgetTarget::Total,
        BudgetTarget::Category("groceries".into()),
        BudgetTarget::Store(StoreScope::store("bank", true)),
    ];
    let periods = [BudgetPeriod::Day, BudgetPeriod::Week, BudgetPeriod::Month];
    doc.budgets = targets
        .into_iter()
        .zip(periods)
        .enumerate()
        .map(|(index, (target, period))| Budget {
            id: format!("budget-{index}"),
            active: true,
            target,
            period,
            limit: amount("1500"),
            currency: "rub".into(),
            warn: true,
        })
        .collect();
    doc.goals = vec![Goal {
        id: "goal".into(),
        name: "Car".into(),
        active: true,
        target_amount: amount("1000000"),
        currency: "rub".into(),
        starts_at: "2026-01-01T00:00:00Z".into(),
        deadline: Some("2027-01-01T00:00:00Z".into()),
        priority: 5,
        monthly_commitment: Some(amount("50000")),
        scope: StoreScope::store("bank", false),
        created_at: "2026-01-01T00:00:00Z".into(),
        scenario_id: None,
        expected_return: None,
    }];
    doc.reminders = vec![Reminder {
        id: "rem".into(),
        text: "Call the bank".into(),
        due_at: "2026-03-01T09:00:00Z".into(),
        done: false,
        repeat: Some(crate::model::planning::ReminderRepeat {
            every: 1,
            unit: crate::model::planning::RepeatUnit::Month,
        }),
        source: Some(crate::model::planning::ReminderSource {
            kind: crate::model::planning::ReminderSourceKind::LoanPayment,
            id: "loan".into(),
        }),
        muted: true,
        user_edited: true,
        pay: None,
        snoozed_until: Some("2026-03-01T10:00:00Z".into()),
        source_key: "m25|1|09:30".into(),
        keep: false,
    }];
    doc.price_alerts = vec![PriceAlert {
        id: "alert".into(),
        asset: usd(),
        condition: AlertCondition::Below,
        price: amount("0.9"),
        currency: "eur".into(),
        active: true,
        triggered_at: None,
        triggered_for: None,
        store_id: None,
        repeats: false,
    }];
    doc.categorization_rules = vec![CategorizationRule {
        id: "rule".into(),
        pattern: "такси".into(),
        category: "taxi".into(),
        active: true,
        order: 0,
    }];
    let kinds = [
        UtilityKind::Metered {
            unit: "kWh".into(),
            tariff: amount("6.5"),
            formula: Some("расход_день * 6.5 + расход_ночь * 3".into()),
            zones: vec!["день".into(), "ночь".into()],
        },
        UtilityKind::Static {
            amount: amount("700"),
        },
        UtilityKind::Housing {
            amount: amount("30000"),
            deposit: Some(amount("30000")),
            due_day: 5,
            automation_id: Some("auto-0".into()),
        },
    ];
    doc.utility_items = kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| UtilityItem {
            id: format!("util-{index}"),
            name: "Power".into(),
            active: true,
            kind,
            currency: "rub".into(),
            property_id: "flat".into(),
            store_id: Some("bank".into()),
            category: "utilities".into(),
        })
        .collect();
    doc.meter_readings = vec![MeterReading {
        id: "reading".into(),
        item_id: "util-0".into(),
        value: amount("1234.5"),
        period: "2026-01".into(),
        recorded_at: "2026-01-31T00:00:00Z".into(),
        day: Some(15),
        zone: Some("день".into()),
    }];
    doc.notification_log = vec![crate::model::notify::NoticeRecord {
        key: "reminder:rem:2026-03-01T09:00:00Z".into(),
        category: crate::model::notify::NoticeCategory::Reminder,
        title: "Reminder".into(),
        body: "Call the bank".into(),
        link: Some("/automations".into()),
        at: "2026-03-01T09:00:00Z".into(),
        delivered_at: "2026-03-01T09:00:10Z".into(),
        read: true,
    }];
    doc.settings.notifications = crate::model::notify::NotificationSettings {
        system: false,
        hide_details_when_locked: true,
        default_time: "09:30".into(),
        categories: vec![crate::model::notify::NoticeCategoryCfg {
            category: crate::model::notify::NoticeCategory::Tax,
            enabled: false,
            lead_days: 30,
            auto_reminders: false,
        }],
    };
    doc.enrichments = [
        EnrichmentSource::Explorer(Chain::Bitcoin),
        EnrichmentSource::TokenList,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, source)| EnrichmentRecord {
        id: format!("enr-{index}"),
        tx_id: "tx".into(),
        source,
        found: EnrichmentData {
            fee: fee(),
            confirmed: Some(true),
            block_time: Some("2026-01-01T00:00:00Z".into()),
            from_address: Some("a".into()),
            to_address: Some("b".into()),
            token_name: Some("Tether".into()),
            token_decimals: Some(6),
            contract: Some("0x1".into()),
        },
        status: EnrichmentStatus::Pending,
        created_at: "2026-01-01T00:00:00Z".into(),
    })
    .collect();
    doc.manual_prices = vec![ManualPrice {
        asset_key: "asset:FLAT".into(),
        unit_price: amount("9000000"),
        currency: "rub".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }];
    doc.pending_imports = vec![PendingImport {
        exec_id: "exec".into(),
        is_maker: Some(true),
        closed_size: Some(amount("0.1")),
        closed_pnl: Some(amount("5")),
        ..PendingImport::default()
    }];
    doc.categories.push(crate::model::category::Category {
        key: "custom".into(),
        name: Some("Custom".into()),
        group: CategoryGroup::Savings,
        archived: true,
        essential: Some(false),
        passive: Some(true),
    });
    doc.emergency_fund = crate::model::planning::EmergencyFund {
        scope: crate::model::store::StoreScope {
            store_ids: vec!["reserve".into()],
            include_children: false,
        },
        target_months: 3,
        avg_months: 12,
    };
    doc.health_last = Some(crate::model::planning::HealthSnapshot {
        month: "2026-03".into(),
        color: crate::model::planning::HealthColor::Yellow,
        reasons: vec![crate::model::planning::HealthReasonCode::FundBelowThree],
    });
    doc.asset_classes = vec![crate::model::planning::AssetClass {
        id: "local".into(),
        name: "Local".into(),
        target_pct: amount("100"),
        cap: false,
    }];
    doc.asset_class_of = vec![crate::model::planning::AssetClassLink {
        asset_key: "fiat:usd".into(),
        class_id: "local".into(),
    }];
    doc.valuations = vec![crate::model::vault::Valuation {
        id: "val1".into(),
        store_id: "flat".into(),
        asset_key: "asset:FLAT".into(),
        date: "2025-06-01".into(),
        unit_price: Amount::new(rust_decimal::Decimal::from(9_000_000)),
        currency: "rub".into(),
    }];
    doc.tx_properties = vec![crate::model::vault::TxProperty {
        tx_id: "tx1".into(),
        property_id: "flat".into(),
    }];
    // Sync bookkeeping, including a stored record value (MessagePack must
    // carry arbitrary JSON in a batch for undo).
    let stamp = crate::sync::Stamp::new(1_700_000_000_000, 2, "devaaaaa");
    doc.sync.stamps.insert(
        "stores:x".into(),
        crate::sync::RecordStamps {
            created: stamp.clone(),
            fields: [("name".to_string(), stamp.clone())].into(),
        },
    );
    doc.sync
        .tombstones
        .insert("contacts:gone".into(), stamp.clone());
    doc.sync
        .tombstones_seen
        .insert("contacts:gone".into(), 1_700_000_500_000);
    doc.sync.peers.push(crate::sync::TrustedPeer {
        device_id: "dev-b".into(),
        name: "Phone".into(),
        public_key: "ab".repeat(32),
        paired_at: "2026-10-01T00:00:00Z".into(),
        last_synced_at: Some("2026-10-02T00:00:00Z".into()),
        revoked: None,
        stamp: stamp.clone(),
    });
    doc.sync.batches.push(crate::sync::SyncBatch {
        id: "b1".into(),
        peer_device: "dev-b".into(),
        peer_name: "Phone".into(),
        applied_at: "2026-10-02T00:00:00Z".into(),
        summary: crate::sync::SyncSummary::default(),
        entries: vec![crate::sync::BatchEntry {
            key: "stores:x".into(),
            before: crate::sync::RecordState {
                value: Some(serde_json::json!({"id": "x", "n": 1.5, "tags": [null, true]})),
                stamps: None,
                tombstone: None,
            },
            after: crate::sync::RecordState {
                value: None,
                stamps: None,
                tombstone: Some(stamp),
            },
        }],
        undone_at: None,
    });
    doc
}
