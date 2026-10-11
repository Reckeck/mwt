//! Domain services: everything the app does on top of the [`VaultDoc`].
//! This module is pure Rust (no Tauri); the `app` crate only maps commands.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::Amount;
use crate::error::{Error, Result, StoreTreeViolation, WalletRule};
use crate::model::chain::Chain;
use crate::model::store::{
    AssetRef, Binding, BindingShape, Custody, ExchangeProfile, ExchangeProvider, Holding,
    IndicatorCfg, LiabilityTerms, MAX_STORE_DEPTH, PropertyInfo, Purpose, Store, StoreKind,
    StoreScope, SubType, TerminalLayout, TerminalState,
};
use crate::model::transaction::{
    AccountRef, Fee, FuturesAction, FuturesCategory, FuturesDirection, KindDetails, Leg, LegRole,
    Liquidity, PriceStamp, TopupTx, TradeFuturesTx, TradeSide, TradeSpotTx, Transaction,
    TransferTx, TxMeta, TxOrigin,
};
use crate::model::vault::{PendingImport, Settings, VaultDoc, iso_to_millis, now_iso};
use crate::pricing::history::secs_or_now;
use crate::storage::{SnapshotFormat, VaultFile};

mod adjustments;
mod agenda;
mod automations;
mod budget;
mod categories;
mod contacts;
mod enrichment;
mod envelopes;
mod goals;
mod interest;
mod ledger;
mod losses;
mod loyalty;
mod preview;
mod rates;
mod reports;
#[cfg(test)]
mod test_support;
mod utilities;
mod validate;
mod worth;

pub use adjustments::AdjustmentDraft;
pub use agenda::{PriceAlertDraft, UpcomingItem, UpcomingKind};
pub use automations::{
    AutomationDraft, AutomationPass, MAX_CATCH_UP, describe_reject, planned_dates,
};
pub use budget::{BudgetDraft, BudgetStatus, DailyBudget, MonthSummary};
pub use contacts::{ContactBalance, ContactDraft, ContactTurnover, DebtTermsDraft, TurnoverRow};
pub use envelopes::{EnvelopeDraft, EnvelopeView};
pub use goals::{GoalCalendar, GoalCalendarMonth, GoalDraft, GoalProgress, RealismIssue};
pub use interest::{DepositReport, InterestTaxReport, PdsReport};
pub use ledger::{
    GRAPH_NODE_CAP, GraphEdge, GraphEdgeKind, GraphLane, GraphNode, TxFilter, TxGraph,
};
pub use losses::{
    FlowAmount, FlowLink, FlowSummary, FxRow, FxTable, LossBucket, LossReport, LossRow, TxLoss,
};
pub use loyalty::{CashbackReport, CashbackSource, LoyaltyBalance};
pub use preview::{PositionChange, PreviewWarning, TxPreview};
pub use rates::RateBook;
pub use reports::{
    CalendarDay, CalendarMonth, CashflowReport, CashflowRow, CategoriesReport, CategoryGroupBlock,
    CategoryRow, DayStatus, InflationPoint, InflationReport, NetworthPoint, NetworthReport,
    PnlReport, PnlRow, PortfolioPoint, PortfolioReport, Report, ReportFilter, ReportKind,
    ReportMeta, StoreReportRow, StoresReport, YearReport, YearRow, report_csv,
};
pub use utilities::{
    PayUtilityBillInput, PayUtilityLine, PropertySummary, UtilityBill, UtilityBillLine,
    UtilityDraft, UtilityMonth, UtilityPaymentDraft, UtilityTrend,
};
pub use worth::{FlowClass, NetWorth};

/// Store fields beyond name, colour and note. On update, `None` in an
/// `Option<Option>`-like field keeps the current value where noted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoreExtras {
    /// `None` on create = the default (sub-account type, else the kind's);
    /// `None` on update keeps the current purpose.
    pub purpose: Option<Purpose>,
    pub liability: Option<LiabilityTerms>,
    pub contact_id: Option<String>,
    /// Sub-accounts only; fixed after creation.
    pub sub_type: Option<SubType>,
    /// Sub-accounts only. `None` on update keeps the current binding.
    pub binding: Option<Binding>,
    pub custody: Option<Custody>,
    pub property: Option<PropertyInfo>,
    pub secured_by: Option<String>,
    /// Exchange wallets; `None` on update keeps the current exchange.
    pub provider: Option<ExchangeProvider>,
    /// Create only: money already there, booked as one top-up.
    pub opening: Option<OpeningBalance>,
}

/// What a new wallet already holds. Booked as a top-up in the service
/// category `opening_balance`, so it is neither income nor spending.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct OpeningBalance {
    pub asset: AssetRef,
    pub amount: Amount,
}

/// A partial update of an exchange terminal; `None` keeps the field.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct TerminalPatch {
    #[serde(default)]
    pub watchlist: Option<Vec<String>>,
    #[serde(default)]
    pub active_symbol: Option<String>,
    #[serde(default)]
    pub timeframe: Option<String>,
    #[serde(default)]
    pub indicators: Option<Vec<IndicatorCfg>>,
    #[serde(default)]
    pub layout: Option<TerminalLayout>,
}

/// Upper bound on staged imports held in the vault. A background poll must not
/// be able to grow the encrypted file without limit.
pub const MAX_PENDING_IMPORTS: usize = 10_000;

/// An unlocked vault held in memory for the duration of a session.
/// The master password lives only here (zeroized on drop) — it is never
/// written to disk and is required again on the next unlock.
pub struct UnlockedVault {
    pub path: PathBuf,
    pub fmt: SnapshotFormat,
    pub doc: VaultDoc,
    /// Portfolio base currency for this session.
    ///
    /// Deliberately *not* part of `doc`: the base currency is an app-level
    /// preference (see [`crate::appconfig::AppSettings`]), so opening a
    /// different vault must not change it. The app layer pushes the current
    /// value in right after the vault is opened.
    base_currency: String,
    password: Zeroizing<String>,
    /// Released when the vault is locked (dropped).
    _lock: crate::storage::VaultLock,
}

impl UnlockedVault {
    pub fn create(path: PathBuf, password: &str) -> Result<Self> {
        VaultFile::create(&path, password)?;
        let lock = crate::storage::VaultLock::acquire(&path)?;
        let doc = VaultDoc::default();
        let fmt = SnapshotFormat::for_profile();
        let mut vault = Self {
            path,
            fmt,
            doc,
            base_currency: crate::appconfig::DEFAULT_BASE_CURRENCY.to_string(),
            password: Zeroizing::new(password.to_string()),
            _lock: lock,
        };
        vault.ensure_cash_wallet("Cash")?;
        Ok(vault)
    }

    pub fn open(path: PathBuf, password: &str) -> Result<Self> {
        let (decoded, fmt) = VaultFile::open_versioned(&path, password)?;
        // After the password check: a wrong password must not hint the vault is in use.
        let lock = crate::storage::VaultLock::acquire(&path)?;
        let migrated = decoded.from_version < crate::VAULT_FORMAT_VERSION;
        if migrated {
            // The file as it was stays next to the vault before anything rewrites it.
            VaultFile::backup_version(&path, decoded.from_version)?;
        }
        let mut vault = Self {
            path,
            fmt: fmt.for_writing(),
            doc: decoded.doc,
            base_currency: crate::appconfig::DEFAULT_BASE_CURRENCY.to_string(),
            password: Zeroizing::new(password.to_string()),
            _lock: lock,
        };
        // A vault from before the single cash wallet gets one on first open.
        vault.ensure_cash_wallet("Cash")?;
        if migrated || fmt != vault.fmt {
            vault.persist()?;
        }
        Ok(vault)
    }

    /// Point the vault at the app-level base currency.
    ///
    /// Holdings, average costs and every "in base" figure are re-derived, so an
    /// existing vault keeps reporting the right numbers after the switch. The
    /// document itself is not rewritten — only the document's *history* values
    /// (`price_base` stamps) stay as they were written.
    pub fn set_base_currency(&mut self, base: &str) {
        let base = base.trim().to_lowercase();
        if base.is_empty() || base == self.base_currency {
            return;
        }
        self.base_currency = base;
        self.recompute_holdings();
    }

    /// Current base currency, never empty.
    pub fn base_currency(&self) -> &str {
        &self.base_currency
    }

    /// Persist the current document (encrypt + atomic write).
    pub fn persist(&self) -> Result<()> {
        VaultFile::save(&self.path, &self.password, &self.doc, self.fmt)
    }

    /// Replace the whole document with an externally supplied one and persist
    /// it (raw JSON editor).
    ///
    /// Holdings are re-derived from the transactions rather than trusted from
    /// the payload, so an edited `holdings` array can never disagree with the
    /// transactions it summarizes. `id` and `created_at` are pinned to the
    /// current document: a replace edits *content*, it does not re-create the
    /// vault, and a changed id would orphan every store in the recents list.
    pub fn replace_doc(&mut self, mut doc: VaultDoc) -> Result<()> {
        doc.id = self.doc.id.clone();
        doc.created_at = self.doc.created_at.clone();
        doc.version = crate::VAULT_FORMAT_VERSION;
        self.doc = doc;
        self.recompute_holdings();
        self.persist()
    }

    /// Try opening with a possibly-wrong password (used by unlock screen).
    pub fn try_open(path: PathBuf, password: &str) -> Result<Self> {
        Self::open(path, password)
    }

    /// Re-encrypt the vault under a new password.
    ///
    /// The current password is checked against the file on disk (decryption is
    /// the verifier), so an unlocked but unattended session cannot be re-keyed
    /// by whoever sits down at it. No `.bak` is written on purpose: a copy that
    /// still opens with the old password would defeat changing a leaked one.
    pub fn change_password(&mut self, current_password: &str, new_password: &str) -> Result<()> {
        if new_password.is_empty() {
            return Err(Error::InvalidInput(
                "the new password must not be empty".into(),
            ));
        }
        VaultFile::open(&self.path, current_password)?;
        self.password = Zeroizing::new(new_password.to_string());
        self.persist()
    }

    /// Re-encode on disk (dev JSON <-> prod binary) — mostly used by tests.
    /// Legacy bincode is never written: asking for it writes MessagePack.
    pub fn switch_format(&mut self, fmt: SnapshotFormat) -> Result<()> {
        self.fmt = fmt.for_writing();
        self.persist()
    }

    // -- settings -----------------------------------------------------------

    /// Apply a partial update of the vault-scoped settings: every field that is
    /// present is written (clamped to a sane range), the rest is kept.
    pub fn update_settings(
        &mut self,
        input: crate::ipc_contract::UpdateSettingsInput,
    ) -> Result<Settings> {
        if let Some(rate) = input.savings_rate
            && (rate.is_negative() || rate > Amount::new(rust_decimal::Decimal::ONE))
        {
            return Err(Error::InvalidInput(
                "the savings rate must be between 0 and 100%".into(),
            ));
        }
        let settings = &mut self.doc.settings;
        if let Some(decimals) = input.display_decimals {
            settings.display_decimals = decimals.clamp(0, 18);
        }
        if let Some(widgets) = input.dashboard_widgets {
            settings.dashboard_widgets = widgets;
        }
        if let Some(interval) = input.fx_refresh_interval_sec {
            settings.fx_refresh_interval_sec = interval.clamp(30, 3600);
        }
        if let Some(minimize) = input.minimize_to_tray {
            settings.minimize_to_tray = minimize;
        }
        if let Some(rate) = input.savings_rate {
            settings.savings_rate = rate;
        }
        if let Some(mode) = input.daily_budget_mode {
            settings.daily_budget_mode = mode;
        }
        if let Some(currency) = input.budget_currency {
            settings.budget_currency = currency.trim().to_lowercase();
        }
        let out = settings.clone();
        self.persist()?;
        Ok(out)
    }

    // -- stores -------------------------------------------------------------
    //
    // Stores form a tree of at most `MAX_STORE_DEPTH + 1` levels. Every write
    // that places a node goes through `validate_placement`, so the tree rules
    // live in one spot; reads stay lenient so a hand-edited vault still opens.

    pub(crate) fn find_store(&self, id: &str) -> Option<&Store> {
        self.doc.stores.iter().find(|store| store.id == id)
    }

    /// Parent chain from the root down to `id`'s parent. Stops at a missing
    /// parent or at a cycle left behind by a hand-edited vault.
    pub(crate) fn ancestors_of(&self, id: &str) -> Vec<&Store> {
        let mut chain = Vec::new();
        let mut seen = HashSet::from([id.to_string()]);
        let mut parent = self
            .find_store(id)
            .and_then(|store| store.parent_id.as_deref());
        while let Some(parent_id) = parent {
            if !seen.insert(parent_id.to_string()) {
                break;
            }
            let Some(store) = self.find_store(parent_id) else {
                break;
            };
            chain.push(store);
            parent = store.parent_id.as_deref();
        }
        chain.reverse();
        chain
    }

    /// 0 for a top-level wallet.
    pub fn depth(&self, id: &str) -> usize {
        self.ancestors_of(id).len()
    }

    /// Top-level wallet that `id` belongs to (itself for a root).
    pub fn root_of(&self, id: &str) -> String {
        self.ancestors_of(id)
            .first()
            .map(|store| store.id.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// Levels below `id`: 0 for a leaf.
    fn subtree_height(&self, id: &str) -> usize {
        let mut seen = HashSet::from([id.to_string()]);
        let mut level = vec![id.to_string()];
        let mut height = 0;
        loop {
            let next: Vec<String> = level
                .iter()
                .flat_map(|parent| self.children_of(parent))
                .filter(|child| seen.insert(child.id.clone()))
                .map(|child| child.id.clone())
                .collect();
            if next.is_empty() {
                return height;
            }
            height += 1;
            level = next;
        }
    }

    /// R5: names are unique among siblings (roots among roots), ignoring case
    /// and surrounding spaces. `node` is skipped so a rename to itself passes.
    fn ensure_unique_name(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        let wanted = name.trim().to_lowercase();
        let taken = self.doc.stores.iter().any(|store| {
            node != Some(store.id.as_str())
                && store.parent_id.as_deref() == parent
                && store.name.trim().to_lowercase() == wanted
        });
        if taken {
            return Err(Error::StoreTree(StoreTreeViolation::NameTaken));
        }
        Ok(())
    }

    /// Whether `node` (an existing store, or `None` for a new one) may sit
    /// under `parent` with `name`. Rules R1–R6.
    fn validate_placement(
        &self,
        node: Option<&str>,
        parent: Option<&str>,
        name: &str,
    ) -> Result<()> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("store name must not be empty".into()));
        }
        if let Some(parent_id) = parent {
            let parent_store = self
                .find_store(parent_id)
                .ok_or_else(|| Error::Missing("parent store".into()))?;
            if let Some(node_id) = node {
                if self
                    .find_store(node_id)
                    .is_some_and(|store| store.parent_id.is_none())
                {
                    return Err(Error::StoreTree(StoreTreeViolation::RootMove));
                }
                if self
                    .find_store(node_id)
                    .is_some_and(|store| store.kind != parent_store.kind)
                {
                    return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
                }
                if parent_id == node_id
                    || self.subtree_ids(node_id).iter().any(|id| id == parent_id)
                {
                    return Err(Error::StoreTree(StoreTreeViolation::Cycle));
                }
            }
            let height = node.map_or(0, |id| self.subtree_height(id));
            if self.depth(parent_id) + 1 + height > MAX_STORE_DEPTH {
                return Err(Error::StoreTree(StoreTreeViolation::TooDeep));
            }
            if parent_store.archived {
                return Err(Error::StoreTree(StoreTreeViolation::ParentArchived));
            }
        }
        // A typed sub-account cannot become a top-level wallet.
        if parent.is_none()
            && node
                .and_then(|id| self.find_store(id))
                .is_some_and(|store| store.parent_id.is_some())
        {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        self.ensure_unique_name(node, parent, name)
    }

    pub fn create_store(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
    ) -> Result<Store> {
        let extras = match parent_id.as_deref().and_then(|id| self.find_store(id)) {
            Some(parent) => default_sub_extras(parent.kind),
            None => StoreExtras::default(),
        };
        self.create_store_with(name, kind, color, note, parent_id, extras)
    }

    /// Which extras a wallet of this kind and level may carry, shared by
    /// create and update.
    fn check_store_extras(
        &self,
        kind: StoreKind,
        sub_type: Option<SubType>,
        is_root: bool,
        extras: &StoreExtras,
    ) -> Result<()> {
        let credit = sub_type.is_some_and(SubType::is_credit);
        if extras.liability.is_some() && kind != StoreKind::Liability && !credit {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        if let Some(terms) = &extras.liability
            && terms.due_day.is_some_and(|day| !(1..=31).contains(&day))
        {
            return Err(Error::InvalidInput("the payment day must be 1–31".into()));
        }
        let misplaced = (extras.custody.is_some() && !(is_root && kind == StoreKind::Crypto))
            || (extras.property.is_some() && !(is_root && kind == StoreKind::Property))
            || (extras.provider.is_some() && !(is_root && kind == StoreKind::Exchange))
            || (extras.secured_by.is_some() && kind != StoreKind::Liability);
        if misplaced {
            return Err(Error::WalletRule(WalletRule::NotApplicable));
        }
        if let Some(target) = &extras.secured_by
            && !self
                .find_store(target)
                .is_some_and(|store| store.kind == StoreKind::Property && store.parent_id.is_none())
        {
            return Err(Error::Missing(format!("property {target}")));
        }
        if let Some(contact_id) = &extras.contact_id {
            if !is_root {
                return Err(Error::InvalidInput(
                    "a contact is set on the top-level wallet; sub-accounts share it".into(),
                ));
            }
            if !self
                .doc
                .contacts
                .iter()
                .any(|contact| &contact.id == contact_id)
            {
                return Err(Error::Missing(format!("contact {contact_id}")));
            }
        }
        Ok(())
    }

    /// A sub-account's type must be offered by its wallet kind and its
    /// binding must have the shape the type expects.
    fn check_sub_account(
        kind: StoreKind,
        sub_type: Option<SubType>,
        binding: Option<&Binding>,
    ) -> Result<(SubType, Binding)> {
        let sub_type = sub_type.ok_or(Error::WalletRule(WalletRule::BindingRequired))?;
        if !kind.admits_sub_type(sub_type) {
            return Err(Error::WalletRule(WalletRule::SubTypeNotAllowed));
        }
        let binding = match (sub_type.binding_shape(), binding) {
            (BindingShape::Market(market), None | Some(Binding::Any)) => Binding::Market(market),
            (_, None | Some(Binding::Any)) => {
                return Err(Error::WalletRule(WalletRule::BindingRequired));
            }
            (shape, Some(binding)) if binding.shape() == Some(shape) => binding.clone(),
            _ => return Err(Error::WalletRule(WalletRule::BindingMismatch)),
        };
        if let Binding::Pair { base, quote } = &binding
            && base.key() == quote.key()
        {
            return Err(Error::InvalidInput(
                "a pair needs two different assets".into(),
            ));
        }
        Ok((sub_type, binding))
    }

    /// The vault always has exactly one cash wallet; called on create and
    /// open. Returns whether one had to be added.
    pub fn ensure_cash_wallet(&mut self, name: &str) -> Result<bool> {
        let has_cash = self
            .doc
            .stores
            .iter()
            .any(|store| store.kind == StoreKind::Cash && store.parent_id.is_none());
        if has_cash {
            return Ok(false);
        }
        let name = if name.trim().is_empty() { "Cash" } else { name };
        let mut store = Store::new(name, StoreKind::Cash, "#16a34a");
        if self.ensure_unique_name(None, None, &store.name).is_err() {
            store.name = format!("{} 2", store.name);
        }
        self.doc.stores.insert(0, store);
        self.persist()?;
        Ok(true)
    }

    pub fn create_store_with(
        &mut self,
        name: &str,
        kind: StoreKind,
        color: &str,
        note: &str,
        parent_id: Option<String>,
        extras: StoreExtras,
    ) -> Result<Store> {
        self.validate_placement(None, parent_id.as_deref(), name)?;
        let parent = parent_id
            .as_deref()
            .and_then(|id| self.find_store(id))
            .cloned();
        // A sub-account is always of its wallet's kind.
        let kind = parent.as_ref().map_or(kind, |parent| parent.kind);
        let (sub_type, binding) = match &parent {
            Some(_) => {
                let (sub_type, binding) =
                    Self::check_sub_account(kind, extras.sub_type, extras.binding.as_ref())?;
                (Some(sub_type), binding)
            }
            None => {
                if !kind.user_creatable() {
                    return Err(Error::WalletRule(WalletRule::NotCreatable));
                }
                (None, Binding::Any)
            }
        };
        if parent.is_none()
            && let Some(max) = kind.max_roots()
            && self
                .doc
                .stores
                .iter()
                .filter(|store| store.kind == kind && store.parent_id.is_none())
                .count()
                >= max
        {
            return Err(Error::WalletRule(WalletRule::CashSingleton));
        }
        self.check_store_extras(kind, sub_type, parent.is_none(), &extras)?;
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
            store.exchange = match (kind, extras.provider) {
                (StoreKind::Exchange, Some(provider)) => Some(ExchangeProfile {
                    provider,
                    import_interval_sec: 900,
                    terminal: Default::default(),
                }),
                _ => None,
            };
        } else {
            store.custody = None;
        }
        store.secured_by = extras.secured_by;
        store.parent_id = parent_id;
        self.doc.stores.push(store.clone());
        // Validated before anything is written, so a bad opening balance
        // leaves no half-created wallet behind.
        if let Some(opening) = extras.opening.filter(|opening| !opening.amount.is_zero()) {
            if matches!(kind, StoreKind::Property | StoreKind::Liability) {
                self.doc.stores.pop();
                return Err(Error::WalletRule(WalletRule::NotApplicable));
            }
            let mut tx = opening_topup(&store.id, opening);
            if let Err(err) = self.prepare_transaction(&mut tx) {
                self.doc.stores.pop();
                return Err(err);
            }
            self.doc.transactions.push(tx);
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(store)
    }

    /// Edit a store's own fields. Its place in the tree changes only through
    /// [`Self::move_store`].
    pub fn update_store(
        &mut self,
        id: &str,
        name: &str,
        color: &str,
        note: &str,
        extras: StoreExtras,
    ) -> Result<Store> {
        let current = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        let parent = current.parent_id.clone();
        let kind = current.kind;
        let sub_type = current.sub_type;
        if !name.trim().is_empty() {
            self.ensure_unique_name(Some(id), parent.as_deref(), name)?;
        }
        self.check_store_extras(kind, sub_type, parent.is_none(), &extras)?;
        let binding = match (&extras.binding, parent.is_some()) {
            (Some(binding), true) => {
                let (_, binding) = Self::check_sub_account(kind, sub_type, Some(binding))?;
                // Everything already booked here must still fit.
                let fits = self
                    .doc
                    .transactions
                    .iter()
                    .flat_map(Transaction::legs)
                    .filter(|leg| leg.store_id == id)
                    .all(|leg| binding.admits(&leg.asset));
                if !fits {
                    return Err(Error::WalletRule(WalletRule::BindingMismatch));
                }
                Some(binding)
            }
            (Some(_), false) => return Err(Error::WalletRule(WalletRule::NotApplicable)),
            (None, _) => None,
        };
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            if !name.trim().is_empty() {
                store.name = name.trim().to_string();
            }
            store.color = color.trim().to_string();
            store.note = note.trim().to_string();
            if let Some(purpose) = extras.purpose {
                store.purpose = purpose;
            }
            if let Some(binding) = binding {
                store.binding = binding;
            }
            store.liability = extras.liability;
            store.contact_id = extras.contact_id;
            if store.parent_id.is_none() {
                if extras.custody.is_some() {
                    store.custody = extras.custody;
                }
                if extras.property.is_some() {
                    store.property = extras.property;
                }
                if let Some(provider) = extras.provider {
                    let profile = store.exchange.get_or_insert_with(|| ExchangeProfile {
                        provider,
                        import_interval_sec: 900,
                        terminal: Default::default(),
                    });
                    profile.provider = provider;
                }
            }
            store.secured_by = extras.secured_by;
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    /// Re-hang a sub-account under another node, or make it a top-level
    /// wallet with `parent_id = None`. A top-level wallet never moves (R6).
    pub fn move_store(&mut self, id: &str, parent_id: Option<&str>) -> Result<Store> {
        let store = self
            .find_store(id)
            .cloned()
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.as_deref() == parent_id {
            return Ok(store);
        }
        self.validate_placement(Some(id), parent_id, &store.name)?;
        let out = {
            let store = self
                .doc
                .stores
                .iter_mut()
                .find(|store| store.id == id)
                .ok_or_else(|| Error::Missing("store".into()))?;
            store.parent_id = parent_id.map(str::to_string);
            store.clone()
        };
        self.persist()?;
        Ok(out)
    }

    /// Where `id` may be moved: every allowed parent (flat, without the
    /// current one) and whether it may become a top-level wallet.
    pub fn store_move_targets(&self, id: &str) -> Result<StoreMoveTargets> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if store.parent_id.is_none() {
            return Ok(StoreMoveTargets::default());
        }
        let parents = flatten_tree(self.store_tree())
            .into_iter()
            .filter(|node| store.parent_id.as_deref() != Some(node.store.id.as_str()))
            .filter(|node| {
                self.validate_placement(Some(id), Some(&node.store.id), &store.name)
                    .is_ok()
            })
            .collect();
        Ok(StoreMoveTargets {
            parents,
            can_detach: false,
        })
    }

    /// Archive (or restore) a store together with its whole subtree. A node
    /// cannot be restored while an ancestor is still archived (R8).
    pub fn archive_store(&mut self, id: &str, archived: bool) -> Result<()> {
        let store = self
            .find_store(id)
            .ok_or_else(|| Error::Missing("store".into()))?;
        if archived && store.kind == StoreKind::Cash && store.parent_id.is_none() {
            return Err(Error::WalletRule(WalletRule::CashLocked));
        }
        if archived && store.archived {
            return Err(Error::InvalidInput("already archived".into()));
        }
        if !archived && self.ancestors_of(id).iter().any(|parent| parent.archived) {
            return Err(Error::StoreTree(StoreTreeViolation::RestoreParentFirst));
        }
        let mut affected: HashSet<String> = self.subtree_ids(id).into_iter().collect();
        affected.insert(id.to_string());
        for store in &mut self.doc.stores {
            if affected.contains(&store.id) {
                store.archived = archived;
            }
        }
        self.persist()
    }

    pub fn delete_store(&mut self, id: &str) -> Result<()> {
        if self
            .find_store(id)
            .is_some_and(|store| store.kind == StoreKind::Cash && store.parent_id.is_none())
        {
            return Err(Error::WalletRule(WalletRule::CashLocked));
        }
        if self.doc.transactions.iter().any(|tx| tx.touches(id)) {
            return Err(Error::InvalidInput(
                "store has transactions; archive instead".into(),
            ));
        }
        if !self.children_of(id).is_empty() {
            return Err(Error::InvalidInput(
                "store has sub-accounts; move or delete them first".into(),
            ));
        }
        let before = self.doc.stores.len();
        self.doc.stores.retain(|store| store.id != id);
        self.doc.holdings.retain(|holding| holding.store_id != id);
        if self.doc.stores.len() == before {
            return Err(Error::Missing("store".into()));
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }

    // -- transactions -------------------------------------------------------
    //
    // Holdings are DERIVED from transactions (single source of truth):
    // `recompute_holdings()` re-derives every position and average cost basis
    // from the transaction stream, guaranteeing consistency.
    //
    // `Transaction` is a closed enum whose variants carry their own fields, so
    // there is no kind tag to dispatch on here: one entry point validates
    // whichever variant it is handed. Adding a variant does not mean adding a
    // branch here, only a `legs()` impl on the variant itself.

    /// Transaction history, newest first, limited to a scope.
    pub fn transactions(&self, scope: &StoreScope) -> Vec<Transaction> {
        let ids = (!scope.store_ids.is_empty()).then(|| self.resolve_scope(scope));
        let mut out: Vec<Transaction> = match ids {
            Some(ids) => self.scoped_txs(&ids),
            None => self.doc.transactions.clone(),
        };
        out.sort_by(|left, right| right.meta().timestamp.cmp(&left.meta().timestamp));
        out
    }

    /// Fill what the user does not type: the purchase price of property
    /// (its unit price is in the base currency at the time of buying).
    fn complete_transaction(&self, tx: &mut Transaction) {
        if let Transaction::Topup(inner) = tx
            && let Some(unit_price) = inner.details.unit_price
            && inner.price.is_empty()
            && self
                .find_store(&inner.store_id)
                .is_some_and(|store| store.kind == StoreKind::Property)
        {
            let (base, _) = self.base_and_fx();
            inner.price.price_base = Some(unit_price);
            inner.price.price_usd = self
                .fx_at(&base, &inner.meta.timestamp)
                .filter(|rate| !rate.is_zero())
                .map(|rate| unit_price / rate);
        }
    }

    /// Everything a write does before storing: categories normalized and
    /// auto-filled, property prices completed, unpriced sides stamped, every
    /// rule checked.
    pub(crate) fn prepare_transaction(&self, tx: &mut Transaction) -> Result<()> {
        Self::normalize_categories(tx);
        self.apply_categorization_rules(tx);
        self.complete_transaction(tx);
        self.complete_adjustment(tx)?;
        Self::stamp_prices(&self.rate_book(), tx);
        self.validate_transaction(tx)
    }

    /// Validate and append a transaction of any variant (plan §4.5, §8.1):
    /// categories are normalized and auto-filled, then every rule is checked.
    pub fn add_transaction(&mut self, mut tx: Transaction) -> Result<Transaction> {
        self.prepare_transaction(&mut tx)?;
        if tx.meta().id.trim().is_empty()
            || self
                .doc
                .transactions
                .iter()
                .any(|known| known.meta().id == tx.meta().id)
        {
            tx.meta_mut().id = uuid::Uuid::new_v4().to_string();
        }
        self.doc.transactions.push(tx.clone());
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    /// Replace a transaction entirely. Its id and origin are kept: an edit
    /// changes what happened, not where the record came from.
    pub fn update_transaction(&mut self, id: &str, mut tx: Transaction) -> Result<Transaction> {
        let index = self
            .doc
            .transactions
            .iter()
            .position(|known| known.meta().id == id)
            .ok_or_else(|| Error::Missing("transaction".into()))?;
        let origin = self.doc.transactions[index].meta().origin.clone();
        tx.meta_mut().id = id.to_string();
        tx.meta_mut().origin = origin;
        self.prepare_transaction(&mut tx)?;
        self.doc.transactions[index] = tx.clone();
        self.recompute_holdings();
        self.persist()?;
        Ok(tx)
    }

    pub fn delete_transaction(&mut self, id: &str) -> Result<()> {
        let before = self.doc.transactions.len();
        self.doc.transactions.retain(|tx| tx.meta().id != id);
        if self.doc.transactions.len() == before {
            return Err(Error::Missing("transaction".into()));
        }
        self.recompute_holdings();
        self.persist()?;
        Ok(())
    }

    // -- holdings recomputation ---------------------------------------------

    fn recompute_holdings(&mut self) {
        // key: (store_id, asset_key) -> (quantity, avg_cost)
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let base = self.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            crate::appconfig::DEFAULT_BASE_CURRENCY.to_string()
        } else {
            base
        };

        // Replay the whole stream leg by leg in timestamp order. Sorting on the
        // transaction is enough because every leg of a transaction shares its
        // timestamp, and within one timestamp the legs are independent
        // per (store, asset) key.
        let mut txs: Vec<Transaction> = self.doc.transactions.clone();
        txs.sort_by(|left, right| left.meta().timestamp.cmp(&right.meta().timestamp));
        let book = self.rate_book();

        for tx in &txs {
            let secs = secs_or_now(&tx.meta().timestamp);
            for leg in tx.legs() {
                let key = (leg.store_id.clone(), leg.asset.key());
                let delta = leg.amount;
                if delta.is_zero() {
                    continue;
                }
                let price = book.leg_price_in_base(&leg, &base, secs);
                let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
                let new_qty = cur + delta;

                if delta.is_positive() && leg.role != LegRole::Fee && !price.is_zero() {
                    // weighted average cost
                    let new_cost = {
                        let old = cost.get(&key).copied().unwrap_or(Amount::ZERO);
                        let total_cost = old * cur + price * delta;
                        if new_qty == Amount::ZERO {
                            Amount::ZERO
                        } else {
                            total_cost / new_qty
                        }
                    };
                    cost.insert(key.clone(), new_cost);
                }
                // sells keep avg_cost unchanged (realized PnL measured against
                // it is computed on demand in analytics)
                qty.insert(key, new_qty);
            }
        }

        // Recover the full AssetRef for each surviving position from the legs
        // that produced it, rather than reconstructing an identity from the
        // cache key (which would lose the name, quote and chain).
        let mut identity: HashMap<(String, String), AssetRef> = HashMap::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                identity
                    .entry((leg.store_id.clone(), leg.asset.key()))
                    .or_insert_with(|| leg.asset.clone());
            }
        }

        self.doc.holdings = qty
            .into_iter()
            .filter(|(_, quantity)| !quantity.is_zero())
            .map(|((store_id, asset_key), quantity)| {
                let asset = identity
                    .remove(&(store_id.clone(), asset_key.clone()))
                    .unwrap_or_else(|| AssetRef::fiat("UNKNOWN", "Unknown"));
                let avg_cost = cost.get(&(store_id.clone(), asset_key.clone())).copied();
                Holding {
                    id: uuid::Uuid::new_v4().to_string(),
                    store_id,
                    asset,
                    quantity,
                    avg_cost,
                }
            })
            .collect();
    }

    // -- analytics ----------------------------------------------------------

    /// Base currency code (never empty) and its **latest** cached FX rate.
    ///
    /// The cache keeps history sorted oldest first, so the first match used to
    /// be the oldest rate ever seen. With no rate at all the factor is 1; the
    /// summary reports that through `base_fx_as_of: None`.
    fn base_and_fx(&self) -> (String, Amount) {
        let base = self.base_currency.to_ascii_lowercase();
        let base = if base.is_empty() {
            crate::appconfig::DEFAULT_BASE_CURRENCY.to_string()
        } else {
            base
        };
        let fx = self
            .per_usd(&base)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        (base, fx)
    }

    /// Timestamp of the base currency's latest rate; `None` when it was never
    /// fetched (USD needs no rate and reports `Some("")`).
    pub(crate) fn base_fx_as_of(&self) -> Option<String> {
        let base = self.base_and_fx().0;
        if base == "usd" {
            return Some(String::new());
        }
        self.doc
            .fx_cache
            .iter()
            .filter(|point| point.currency == base && !point.per_usd.is_zero())
            .map(|point| point.ts.clone())
            .max()
    }

    /// Current USD price of one unit: a hand-set price (property), fiat
    /// through its latest FX rate, else the newest cached quote, else the last price a transaction recorded for
    /// the asset, else zero. Derivatives are never valued.
    fn price_usd_for(&self, asset_key: &str) -> Amount {
        self.manual_price_usd(asset_key)
            .or_else(|| {
                let code = asset_key.strip_prefix("fiat:")?;
                self.per_usd(code)
                    .filter(|rate| !rate.is_zero())
                    .map(|rate| Amount::new(rust_decimal::Decimal::ONE) / rate)
            })
            .or_else(|| {
                self.doc
                    .price_cache
                    .iter()
                    .filter(|point| point.asset_key == asset_key)
                    .max_by(|left, right| left.ts.cmp(&right.ts))
                    .map(|point| point.value_usd)
            })
            .or_else(|| self.last_stamped_price(asset_key))
            .unwrap_or(Amount::ZERO)
    }

    /// Every store id a scope covers. An empty id list means the whole vault.
    pub fn resolve_scope(&self, scope: &StoreScope) -> HashSet<String> {
        if scope.store_ids.is_empty() {
            return self
                .doc
                .stores
                .iter()
                .map(|store| store.id.clone())
                .collect();
        }
        let mut out = HashSet::new();
        for id in &scope.store_ids {
            out.insert(id.clone());
            if scope.include_children {
                out.extend(self.subtree_ids(id));
            }
        }
        out
    }

    /// Transactions with at least one leg on a store in `ids`.
    fn scoped_txs(&self, ids: &HashSet<String>) -> Vec<Transaction> {
        self.doc
            .transactions
            .iter()
            .filter(|tx| ids.iter().any(|id| tx.touches(id)))
            .cloned()
            .collect()
    }

    /// A store's positions, sorted by value descending. `derivatives` picks
    /// the open futures positions instead of the holdings that have value.
    fn store_positions_of(
        &self,
        store_id: &str,
        fx: Amount,
        derivatives: bool,
    ) -> Vec<StorePosition> {
        let mut out: Vec<StorePosition> = self
            .doc
            .holdings
            .iter()
            .filter(|holding| {
                holding.store_id == store_id && holding.asset.is_derivative() == derivatives
            })
            .map(|holding| {
                let price_usd = if derivatives {
                    Amount::ZERO
                } else {
                    self.price_usd_for(&holding.asset.key())
                };
                let value_usd = holding.quantity * price_usd;
                let price_base = price_usd * fx;
                let unrealized_base = match holding.avg_cost {
                    Some(cost) => (price_base - cost) * holding.quantity.abs(),
                    None => Amount::ZERO,
                };
                StorePosition {
                    asset: holding.asset.clone(),
                    quantity: holding.quantity,
                    avg_cost: holding.avg_cost,
                    price_usd,
                    price_base: price_usd * fx,
                    value_usd,
                    value_base: value_usd * fx,
                    unrealized_base,
                }
            })
            .collect();
        out.sort_by_key(|position| std::cmp::Reverse(position.value_base));
        out
    }

    fn store_positions(&self, store_id: &str, fx: Amount) -> Vec<StorePosition> {
        self.store_positions_of(store_id, fx, false)
    }

    /// One row per asset across several stores. The average cost is weighted
    /// by quantity and only kept when every store knows its own.
    fn merged_positions(&self, ids: &[String], fx: Amount) -> Vec<StorePosition> {
        let mut merged: Vec<StorePosition> = Vec::new();
        let mut cost_known: HashMap<String, bool> = HashMap::new();
        for id in ids {
            for position in self.store_positions(id, fx) {
                let key = position.asset.key();
                let known = cost_known.entry(key.clone()).or_insert(true);
                *known &= position.avg_cost.is_some();
                match merged.iter_mut().find(|row| row.asset.key() == key) {
                    Some(row) => {
                        let quantity = row.quantity + position.quantity;
                        row.avg_cost = match (row.avg_cost, position.avg_cost) {
                            (Some(left), Some(right)) if !quantity.is_zero() => {
                                Some((left * row.quantity + right * position.quantity) / quantity)
                            }
                            _ => None,
                        };
                        row.quantity = quantity;
                        row.value_usd += position.value_usd;
                        row.value_base += position.value_base;
                        row.unrealized_base += position.unrealized_base;
                    }
                    None => merged.push(position),
                }
            }
        }
        for row in &mut merged {
            if !cost_known.get(&row.asset.key()).copied().unwrap_or(false) {
                row.avg_cost = None;
            }
        }
        merged.sort_by_key(|position| std::cmp::Reverse(position.value_base));
        merged
    }

    /// Direct children of a store.
    fn children_of(&self, parent_id: &str) -> Vec<&Store> {
        self.doc
            .stores
            .iter()
            .filter(|child| child.parent_id.as_deref() == Some(parent_id))
            .collect()
    }

    /// Every descendant id of `root_id`, without `root_id` itself.
    fn subtree_ids(&self, root_id: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut seen = HashSet::from([root_id.to_string()]);
        let mut queue = vec![root_id.to_string()];
        while let Some(id) = queue.pop() {
            for child in self.children_of(&id) {
                if seen.insert(child.id.clone()) {
                    result.push(child.id.clone());
                    queue.push(child.id.clone());
                }
            }
        }
        result
    }

    /// The whole wallet tree with own and rolled-up values. Roots (and
    /// orphans whose parent is gone) come first; live before archived,
    /// alphabetical inside each group.
    pub fn store_tree(&self) -> Vec<StoreTreeNode> {
        let (_base, fx) = self.base_and_fx();
        let mut own_usd: HashMap<&str, Amount> = HashMap::new();
        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| !holding.asset.is_derivative())
        {
            let value = holding.quantity * self.price_usd_for(&holding.asset.key());
            *own_usd.entry(holding.store_id.as_str()).or_default() += value;
        }
        let mut tx_count: HashMap<String, u32> = HashMap::new();
        for tx in &self.doc.transactions {
            let mut touched: HashSet<String> = HashSet::new();
            touched.extend(tx.store_ids());
            for id in touched {
                *tx_count.entry(id).or_default() += 1;
            }
        }
        let ctx = TreeCtx {
            fx,
            own_usd,
            tx_count,
        };
        let mut seen = HashSet::new();
        let roots: Vec<&Store> = self
            .doc
            .stores
            .iter()
            .filter(|store| {
                store
                    .parent_id
                    .as_deref()
                    .is_none_or(|parent| self.find_store(parent).is_none())
            })
            .collect();
        let mut out: Vec<StoreTreeNode> = roots
            .into_iter()
            .map(|store| self.tree_node(store, 0, &[], &ctx, &mut seen))
            .collect();
        sort_tree(&mut out);
        out
    }

    fn tree_node(
        &self,
        store: &Store,
        depth: usize,
        parent_path: &[String],
        ctx: &TreeCtx,
        seen: &mut HashSet<String>,
    ) -> StoreTreeNode {
        seen.insert(store.id.clone());
        let mut path = parent_path.to_vec();
        path.push(store.name.clone());
        let mut children: Vec<StoreTreeNode> = self
            .children_of(&store.id)
            .into_iter()
            .filter(|child| !seen.contains(&child.id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|child| self.tree_node(child, depth + 1, &path, ctx, seen))
            .collect();
        sort_tree(&mut children);
        let own_usd = ctx
            .own_usd
            .get(store.id.as_str())
            .copied()
            .unwrap_or(Amount::ZERO);
        let subtree_usd = children
            .iter()
            .fold(own_usd, |acc, child| acc + child.subtree_usd);
        StoreTreeNode {
            store: store.clone(),
            depth: depth as u32,
            path,
            own_usd,
            own_base: own_usd * ctx.fx,
            subtree_usd,
            subtree_base: subtree_usd * ctx.fx,
            own_tx_count: ctx.tx_count.get(&store.id).copied().unwrap_or(0),
            // A kind without sub-account types (property, cash) cannot take one.
            can_add_child: depth < MAX_STORE_DEPTH
                && !store.archived
                && !store.kind.sub_types().is_empty(),
            children,
        }
    }

    /// Single-round-trip view of one store for the wallet page.
    /// `include_children` widens the totals, PnL and transaction stats to the
    /// whole subtree; positions are reported both ways regardless.
    pub fn store_overview(&self, store_id: &str, include_children: bool) -> Result<StoreOverview> {
        let store = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        let (base, fx) = self.base_and_fx();
        let node = find_tree_node(self.store_tree(), store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;

        let mut subtree: Vec<String> = vec![store_id.to_string()];
        subtree.extend(self.subtree_ids(store_id));
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        let txs = self.scoped_txs(&scope);
        let first_tx_at = txs
            .iter()
            .map(|tx| tx.meta().timestamp.as_str())
            .min()
            .unwrap_or("")
            .to_string();
        let last_tx_at = txs
            .iter()
            .map(|tx| tx.meta().timestamp.as_str())
            .max()
            .unwrap_or("")
            .to_string();

        let children = node
            .children
            .iter()
            .map(|child| StoreChildRow {
                store: child.store.clone(),
                value_usd: child.own_usd,
                value_base: child.own_base,
                subtree_base: child.subtree_base,
                share: if node.subtree_usd.is_zero() {
                    0.0
                } else {
                    (child.subtree_usd / node.subtree_usd).to_f64()
                },
            })
            .collect();

        Ok(StoreOverview {
            ancestors: self
                .ancestors_of(store_id)
                .into_iter()
                .map(|parent| StoreCrumb {
                    id: parent.id.clone(),
                    name: parent.name.clone(),
                    color: parent.color.clone(),
                })
                .collect(),
            store,
            base_currency: base,
            fx_per_usd: fx,
            include_children,
            own_usd: node.own_usd,
            own_base: node.own_base,
            subtree_usd: node.subtree_usd,
            subtree_base: node.subtree_base,
            realized_pnl_base: self.realized_pnl_filtered(Some(&scope)).total_base,
            positions: self.store_positions(store_id, fx),
            subtree_positions: self.merged_positions(&subtree, fx),
            open_positions: self.store_positions_of(store_id, fx, true),
            children,
            can_add_child: node.can_add_child,
            tx_count: txs.len() as u32,
            first_tx_at,
            last_tx_at,
        })
    }

    /// Portfolio value of one store (optionally with its subtree) over time.
    pub fn store_history(
        &self,
        store_id: &str,
        include_children: bool,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        self.history_series_filtered(Some(&scope), base_currency)
    }

    /// Realized PnL of one store (optionally with its subtree).
    pub fn store_pnl(&self, store_id: &str, include_children: bool) -> RealizedPnl {
        let scope = self.resolve_scope(&StoreScope::store(store_id, include_children));
        self.realized_pnl_filtered(Some(&scope))
    }

    /// Timestamp of the freshest cached price or FX point.
    pub fn prices_as_of(&self) -> Option<String> {
        let prices = self.doc.price_cache.iter().map(|point| point.ts.as_str());
        let fx = self.doc.fx_cache.iter().map(|point| point.ts.as_str());
        prices
            .chain(fx)
            .filter_map(|ts| {
                chrono::DateTime::parse_from_rfc3339(ts)
                    .ok()
                    .map(|parsed| (parsed, ts))
            })
            .max_by_key(|(parsed, _)| *parsed)
            .map(|(_, ts)| ts.to_string())
    }

    pub fn portfolio_summary(&self) -> PortfolioSummary {
        let (base, fx) = self.base_and_fx();
        let mut total_usd = Amount::ZERO;
        let mut by_asset: HashMap<String, AssetStat> = HashMap::new();
        let mut by_chain: HashMap<String, Amount> = HashMap::new();
        let mut by_store: HashMap<String, Amount> = HashMap::new();

        for holding in self
            .doc
            .holdings
            .iter()
            .filter(|holding| !holding.asset.is_derivative())
        {
            let price = self.price_usd_for(&holding.asset.key());
            let value_usd = holding.quantity * price;
            let asset_key = holding.asset.key();

            let st = by_asset
                .entry(asset_key.clone())
                .or_insert_with(|| AssetStat {
                    asset: holding.asset.clone(),
                    quantity: Amount::ZERO,
                    price_usd: price,
                    value_usd: Amount::ZERO,
                    price_base: price * fx,
                    value_base: Amount::ZERO,
                });
            st.quantity += holding.quantity;
            st.price_usd = price;
            st.value_usd += value_usd;
            st.price_base = price * fx;
            st.value_base += value_usd * fx;

            let chain_key = holding
                .asset
                .chain
                .map(|chain| chain.label().to_string())
                .unwrap_or_else(|| "Other".into());
            *by_chain.entry(chain_key).or_default() += value_usd;
            *by_store.entry(holding.store_id.clone()).or_default() += value_usd;
            total_usd += value_usd;
        }

        let total_base = total_usd * fx;
        let mut stats: Vec<AssetStat> = by_asset.into_values().collect();
        stats.sort_by_key(|stat| std::cmp::Reverse(stat.value_usd));
        let pct = |value: Amount| {
            if total_usd == Amount::ZERO {
                0.0
            } else {
                (value / total_usd).to_f64() * 100.0
            }
        };

        let worth = self.net_worth();
        PortfolioSummary {
            prices_as_of: self.prices_as_of(),
            base_fx_as_of: self.base_fx_as_of(),
            assets_base: worth.financial + worth.non_financial + worth.receivables,
            liabilities_base: worth.liabilities + worth.payables,
            net_worth_base: worth.net,
            base_currency: base,
            fx_per_usd: fx,
            total_usd,
            total_base,
            assets: stats,
            by_chain: by_chain
                .into_iter()
                .map(|(key, value)| NamedValue {
                    label: key,
                    value_usd: value,
                    value_base: value * fx,
                    pct: pct(value),
                })
                .collect(),
            by_store: by_store
                .into_iter()
                .map(|(key, value)| {
                    let name = self
                        .doc
                        .stores
                        .iter()
                        .find(|stat| stat.id == key)
                        .map(|stat| stat.name.clone())
                        .unwrap_or(key.clone());
                    StoreValue {
                        store_id: key,
                        name,
                        value_usd: value,
                        value_base: value * fx,
                        pct: pct(value),
                    }
                })
                .collect(),
        }
    }

    /// Portfolio value over time using transaction timestamps as event
    /// boundaries and the nearest cached price (≤ event ts) per asset.
    pub fn history_series(&self, base_currency: &str) -> Vec<HistoryPoint> {
        self.history_series_filtered(None, base_currency)
    }

    /// Shared history replay. `scope` restricts both the event axis and the
    /// valuation to legs booked on those stores (`None` covers the whole
    /// vault).
    fn history_series_filtered(
        &self,
        scope: Option<&HashSet<String>>,
        base_currency: &str,
    ) -> Vec<HistoryPoint> {
        let base = base_currency.to_lowercase();
        let book = self.rate_book();
        let fx_now = self
            .per_usd(&base)
            .unwrap_or_else(|| Amount::new(rust_decimal::Decimal::ONE));
        let in_scope = |store_id: &str| scope.is_none_or(|ids| ids.contains(store_id));

        // Flatten to legs first: a transfer contributes to one store's series
        // from its outgoing leg and to another's from its incoming one, and the
        // event axis is the set of leg timestamps.
        let mut movements: Vec<(String, String, Amount)> = Vec::new();
        for tx in &self.doc.transactions {
            for leg in tx.legs() {
                if !in_scope(&leg.store_id) || leg.asset.is_derivative() {
                    continue;
                }
                movements.push((leg.asset.key(), tx.meta().timestamp.clone(), leg.amount));
            }
        }
        let mut stamps: Vec<String> = movements.iter().map(|(_, ts, _)| ts.clone()).collect();
        stamps.push(now_iso());
        stamps.sort();
        stamps.dedup();
        movements.sort_by(|left, right| left.1.cmp(&right.1));

        // Running quantity per asset across the whole scope. Valuing these
        // (not the current holdings) keeps an asset that was sold off later in
        // the series, and counts an asset held in two scoped stores once.
        let mut qty: HashMap<String, Amount> = HashMap::new();
        let mut out = Vec::with_capacity(stamps.len());
        let mut idx = 0usize;

        for ev in &stamps {
            while idx < movements.len() && movements[idx].1 <= *ev {
                let (asset, _, amount) = &movements[idx];
                *qty.entry(asset.clone()).or_default() += *amount;
                idx += 1;
            }
            let secs = secs_or_now(ev);
            let mut value_usd = Amount::ZERO;
            for (akey, quantity) in &qty {
                if quantity.is_zero() {
                    continue;
                }
                if let Some(price) = book.history_price(akey, secs) {
                    value_usd += *quantity * price;
                }
            }
            // Each point in the base currency of its own day: a ruble
            // portfolio does not grow just because the ruble fell later.
            let fx = book.fx_at(&base, secs).unwrap_or(fx_now);
            out.push(HistoryPoint {
                ts: ev.clone(),
                ts_ms: iso_to_millis(ev),
                value_usd,
                value_base: value_usd * fx,
            });
        }
        out
    }

    /// The latest USD price a transaction recorded for an asset (e.g. the
    /// last buy), for assets no provider quotes.
    fn last_stamped_price(&self, asset_key: &str) -> Option<Amount> {
        self.doc
            .transactions
            .iter()
            .filter_map(|tx| {
                tx.legs()
                    .into_iter()
                    .find(|leg| leg.asset.key() == asset_key && leg.role == LegRole::Principal)
                    .and_then(|leg| leg.price_usd)
                    .map(|price| (tx.meta().timestamp.clone(), price))
            })
            .max_by(|left, right| left.0.cmp(&right.0))
            .map(|(_, price)| price)
    }

    pub fn realized_pnl(&self) -> RealizedPnl {
        self.realized_pnl_filtered(None)
    }

    /// Shared PnL replay. `scope` restricts it to the legs booked on those
    /// stores (`None` replays the whole vault).
    pub(crate) fn realized_pnl_filtered(&self, scope: Option<&HashSet<String>>) -> RealizedPnl {
        let (base, _fx) = self.base_and_fx();
        let book = self.rate_book();

        // Flatten to (timestamp, leg) and replay. Only legs booked in scope
        // count, which is what makes a per-store PnL differ from the
        // vault-wide one even though they share this code path.
        let mut movements: Vec<(String, Leg)> = Vec::new();
        for tx in &self.doc.transactions {
            let ts = tx.meta().timestamp.clone();
            for leg in tx.legs() {
                if scope.is_some_and(|ids| !ids.contains(&leg.store_id)) {
                    continue;
                }
                movements.push((ts.clone(), leg));
            }
        }
        movements.sort_by(|left, right| left.0.cmp(&right.0));

        // avg cost tracking per (store, asset)
        let mut cost: HashMap<(String, String), Amount> = HashMap::new();
        let mut qty: HashMap<(String, String), Amount> = HashMap::new();
        let mut total = Amount::ZERO;
        let mut fees = Amount::ZERO;
        let mut legs = Vec::new();

        for (ts, leg) in &movements {
            let delta = leg.amount;
            if delta.is_zero() || leg.asset.is_derivative() {
                continue;
            }
            let key = (leg.store_id.clone(), leg.asset.key());
            let price = book.leg_price_in_base(leg, &base, secs_or_now(ts));
            let cur = qty.get(&key).copied().unwrap_or(Amount::ZERO);
            match leg.role {
                // A fee is a cost of its own, never a sale at a profit.
                LegRole::Fee => fees += delta.abs() * price,
                // Settled by the exchange: the PnL is the amount itself.
                LegRole::Pnl => {
                    let pnl = delta * price;
                    total += pnl;
                    legs.push(PnlLeg {
                        ts: ts.clone(),
                        store_id: leg.store_id.clone(),
                        asset_key: leg.asset.key(),
                        quantity: delta.abs(),
                        pnl_base: pnl,
                        side: "settled".into(),
                    });
                }
                // Moving to one's own wallet, or spending without a known
                // price, realizes nothing.
                LegRole::Principal
                    if delta.is_negative() && leg.to_store_id.is_none() && !price.is_zero() =>
                {
                    let avg = cost.get(&key).copied().unwrap_or(Amount::ZERO);
                    let pnl = (price - avg) * delta.abs();
                    total += pnl;
                    legs.push(PnlLeg {
                        ts: ts.clone(),
                        store_id: leg.store_id.clone(),
                        asset_key: leg.asset.key(),
                        quantity: delta.abs(),
                        pnl_base: pnl,
                        side: "sell".into(),
                    });
                }
                LegRole::Principal | LegRole::Adjustment if delta.is_positive() => {
                    let new_qty = cur + delta;
                    let new_cost = if new_qty.is_zero() {
                        Amount::ZERO
                    } else {
                        (cost.get(&key).copied().unwrap_or(Amount::ZERO) * cur + price * delta)
                            / new_qty
                    };
                    cost.insert(key.clone(), new_cost);
                }
                // Found missing: the position shrinks, nothing is sold.
                LegRole::Principal | LegRole::Adjustment => {}
            }
            qty.insert(key, cur + delta);
        }
        RealizedPnl {
            total_base: total,
            fees_base: fees,
            legs,
        }
    }

    // -- exchange keys --------------------------------------------------------

    /// Status of the Bybit key stored for exactly this store.
    pub fn exchange_key_status(&self, store_id: &str) -> crate::ipc_contract::ExchangeKeyStatus {
        self.doc
            .exchanges
            .bybit
            .iter()
            .find(|entry| entry.store_id == store_id)
            .map(|entry| crate::ipc_contract::ExchangeKeyStatus {
                configured: true,
                hint: Some(entry.hint()),
                spot_store_id: entry.spot_store_id.clone(),
                futures_store_id: entry.futures_store_id.clone(),
            })
            .unwrap_or_default()
    }

    /// Store (or replace) a read-only Bybit key for an exchange store.
    /// Sub-account targets of a replaced key are kept.
    pub fn set_exchange_key(
        &mut self,
        store_id: &str,
        api_key: &str,
        api_secret: &str,
    ) -> Result<crate::ipc_contract::ExchangeKeyStatus> {
        let (api_key, api_secret) = (api_key.trim(), api_secret.trim());
        if api_key.is_empty() || api_secret.is_empty() {
            return Err(Error::InvalidInput(
                "api key and secret are required".into(),
            ));
        }
        let store = self
            .find_store(store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if store.kind != StoreKind::Exchange {
            return Err(Error::InvalidInput("store is not an exchange".into()));
        }
        // Keys only make sense once the user said which exchange this is.
        if !store
            .exchange
            .as_ref()
            .is_some_and(|profile| profile.provider.supported())
        {
            return Err(Error::WalletRule(WalletRule::ProviderRequired));
        }
        let previous = self
            .doc
            .exchanges
            .bybit
            .iter()
            .find(|entry| entry.store_id == store_id)
            .map(|entry| (entry.spot_store_id.clone(), entry.futures_store_id.clone()))
            .unwrap_or_default();
        self.doc
            .exchanges
            .bybit
            .retain(|entry| entry.store_id != store_id);
        self.doc
            .exchanges
            .bybit
            .push(crate::model::vault::ExchangeKeys {
                store_id: store_id.to_string(),
                api_key: api_key.to_string(),
                api_secret: api_secret.to_string(),
                spot_store_id: previous.0,
                futures_store_id: previous.1,
            });
        self.persist()?;
        Ok(self.exchange_key_status(store_id))
    }

    /// Point imported spot / futures fills at sub-accounts of the key's
    /// store. Each target must be an exchange store inside that wallet.
    pub fn set_exchange_targets(
        &mut self,
        store_id: &str,
        spot_store_id: Option<String>,
        futures_store_id: Option<String>,
    ) -> Result<crate::ipc_contract::ExchangeKeyStatus> {
        let mut allowed: HashSet<String> = self.subtree_ids(store_id).into_iter().collect();
        allowed.insert(store_id.to_string());
        for target in [&spot_store_id, &futures_store_id].into_iter().flatten() {
            let ok = allowed.contains(target)
                && self
                    .find_store(target)
                    .is_some_and(|store| store.kind == StoreKind::Exchange && !store.archived);
            if !ok {
                return Err(Error::InvalidInput(
                    "fills can only go to an exchange sub-account of this wallet".into(),
                ));
            }
        }
        let entry = self
            .doc
            .exchanges
            .bybit
            .iter_mut()
            .find(|entry| entry.store_id == store_id)
            .ok_or_else(|| Error::Missing("exchange key".into()))?;
        entry.spot_store_id = spot_store_id;
        entry.futures_store_id = futures_store_id;
        self.persist()?;
        Ok(self.exchange_key_status(store_id))
    }

    // -- bybit import staging (D2.4) ---------------------------------------

    /// Stage fills for user confirmation.
    ///
    /// Fills are keyed by Bybit's `execId`, so re-importing the same history is
    /// idempotent. Nothing here touches `transactions` — the user has to confirm
    /// first, which is what keeps a background import from rewriting history.
    /// Returns how many fills were newly staged.
    pub fn stage_imports(&mut self, imports: Vec<PendingImport>) -> Result<u32> {
        let before = self.doc.pending_imports.len();
        let mut known: HashSet<String> = self
            .doc
            .pending_imports
            .iter()
            .map(|pending| pending.exec_id.clone())
            .collect();

        for imp in imports {
            if imp.exec_id.is_empty() || known.contains(&imp.exec_id) {
                continue;
            }
            if self.ensure_store(&imp.store_id).is_err() {
                continue;
            }
            known.insert(imp.exec_id.clone());
            self.doc.pending_imports.push(imp);
            if self.doc.pending_imports.len() >= MAX_PENDING_IMPORTS {
                break;
            }
        }

        let staged = (self.doc.pending_imports.len() - before) as u32;
        if staged > 0 {
            self.persist()?;
        }
        Ok(staged)
    }

    /// Every staged fill, newest first.
    pub fn pending_imports(&self) -> Vec<PendingImport> {
        let mut out = self.doc.pending_imports.clone();
        out.sort_by(|left, right| right.occurred_at.cmp(&left.occurred_at));
        out
    }

    /// Where a fill of this category is booked: the sub-account picked for
    /// spot or futures on the exchange key, else the wallet's live sub-account
    /// bound to that market, else the key's own store.
    fn import_target(&self, imp: &PendingImport) -> String {
        if imp.kind == crate::model::vault::ImportKind::EarnIncome {
            return self
                .bybit_account_store(&imp.store_id, crate::exchange::bybit::ACCOUNT_EARN)
                .unwrap_or_else(|| imp.store_id.clone());
        }
        let futures = imp.category != "spot";
        let market = if futures {
            crate::model::store::MarketKind::Futures
        } else {
            crate::model::store::MarketKind::Spot
        };
        self.doc
            .exchanges
            .bybit
            .iter()
            .find(|keys| keys.store_id == imp.store_id)
            .and_then(|keys| {
                if futures {
                    keys.futures_store_id.clone()
                } else {
                    keys.spot_store_id.clone()
                }
            })
            // Bybit trades spot and derivatives in one unified account.
            .or_else(|| {
                self.bybit_account_store(&imp.store_id, crate::exchange::bybit::ACCOUNT_UNIFIED)
            })
            .or_else(|| {
                self.children_of(&imp.store_id)
                    .into_iter()
                    .find(|child| !child.archived && child.binding == Binding::Market(market))
                    .map(|child| child.id.clone())
            })
            .unwrap_or_else(|| imp.store_id.clone())
    }

    /// The Bybit key that serves a store: its own, else its wallet's — a
    /// sub-account never borrows another wallet's key.
    pub fn bybit_key_for(&self, store_id: &str) -> Option<&crate::model::vault::ExchangeKeys> {
        let mut current = self.find_store(store_id)?;
        loop {
            if let Some(keys) = self
                .doc
                .exchanges
                .bybit
                .iter()
                .find(|keys| keys.store_id == current.id)
            {
                return Some(keys);
            }
            current = self.find_store(current.parent_id.as_deref()?)?;
        }
    }

    /// The live sub-account of an exchange wallet that stands for a Bybit
    /// account type: `FUND` → Funding, `UNIFIED` (and the legacy `SPOT`,
    /// `CONTRACT`) → Unified, `EARN`/`INVESTMENT` → Earn.
    fn bybit_account_store(&self, root_id: &str, account: &str) -> Option<String> {
        use crate::model::store::MarketKind;
        let market = match account.trim().to_ascii_uppercase().as_str() {
            "FUND" => MarketKind::Funding,
            "UNIFIED" | "SPOT" | "CONTRACT" => MarketKind::Unified,
            "EARN" | "INVESTMENT" => MarketKind::Earn,
            _ => return None,
        };
        self.children_of(root_id)
            .into_iter()
            .find(|child| !child.archived && child.binding == Binding::Market(market))
            .map(|child| child.id.clone())
    }

    /// Give a Bybit wallet its three accounts — Funding, Unified Trading and
    /// Earn — creating the ones missing. Returns the ids `(funding, unified, earn)`.
    pub fn ensure_bybit_accounts(
        &mut self,
        store_id: &str,
        names: [&str; 3],
    ) -> Result<[String; 3]> {
        let ids = self.ensure_bybit_accounts_for(store_id, names, [true; 3])?;
        Ok(ids.map(Option::unwrap_or_default))
    }

    /// [`Self::ensure_bybit_accounts`] for the accounts `wanted`
    /// (funding, unified, earn); the others are left alone.
    pub fn ensure_bybit_accounts_for(
        &mut self,
        store_id: &str,
        names: [&str; 3],
        wanted: [bool; 3],
    ) -> Result<[Option<String>; 3]> {
        use crate::model::store::MarketKind;
        let root = self
            .find_store(store_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?;
        if root.kind != StoreKind::Exchange || root.parent_id.is_some() {
            return Err(Error::InvalidInput("not an exchange wallet".into()));
        }
        let accounts = [
            (SubType::Funding, MarketKind::Funding),
            (SubType::Unified, MarketKind::Unified),
            (SubType::Earn, MarketKind::Earn),
        ];
        let mut ids: [Option<String>; 3] = Default::default();
        for (index, (sub_type, market)) in accounts.into_iter().enumerate() {
            let existing = self
                .children_of(store_id)
                .into_iter()
                .find(|child| !child.archived && child.binding == Binding::Market(market))
                .map(|child| child.id.clone());
            ids[index] = match existing {
                Some(id) => Some(id),
                None if wanted[index] => Some(
                    self.create_store_with(
                        names[index],
                        StoreKind::Exchange,
                        &root.color,
                        "",
                        Some(store_id.to_string()),
                        StoreExtras {
                            sub_type: Some(sub_type),
                            binding: Some(Binding::Market(market)),
                            ..StoreExtras::default()
                        },
                    )?
                    .id,
                ),
                None => None,
            };
        }
        Ok(ids)
    }

    /// The live Unified Trading account of an exchange wallet: where spot and
    /// futures trades are booked.
    pub fn unified_account_of(&self, store_id: &str) -> Option<String> {
        let root = self.root_of(store_id);
        self.bybit_account_store(&root, crate::exchange::bybit::ACCOUNT_UNIFIED)
    }

    /// What each Bybit account holds per the API next to what the ledger
    /// says its sub-account holds, coin by coin.
    pub fn bybit_reconcile(
        &self,
        store_id: &str,
        balances: &[(String, Vec<crate::exchange::CoinBalance>)],
    ) -> Vec<crate::ipc_contract::ExchangeAccountBalance> {
        balances
            .iter()
            .map(|(account, coins)| {
                let target = self.bybit_account_store(store_id, account);
                let ledger: HashMap<String, Amount> = target
                    .as_deref()
                    .map(|id| {
                        self.doc
                            .holdings
                            .iter()
                            .filter(|holding| {
                                holding.store_id == id && !holding.asset.is_derivative()
                            })
                            .map(|holding| {
                                (holding.asset.symbol.to_ascii_uppercase(), holding.quantity)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let mut symbols: Vec<String> = coins.iter().map(|coin| coin.coin.clone()).collect();
                symbols.extend(ledger.keys().cloned());
                symbols.sort();
                symbols.dedup();
                let rows = symbols
                    .into_iter()
                    .map(|coin| {
                        let exchange = coins
                            .iter()
                            .find(|row| row.coin == coin)
                            .and_then(|row| Amount::parse(&row.amount).ok())
                            .unwrap_or(Amount::ZERO);
                        let booked = ledger.get(&coin).copied().unwrap_or(Amount::ZERO);
                        crate::ipc_contract::ExchangeCoinRow {
                            coin,
                            exchange,
                            ledger: booked,
                            diff: exchange - booked,
                        }
                    })
                    .collect();
                crate::ipc_contract::ExchangeAccountBalance {
                    account: account.clone(),
                    store_id: target,
                    rows,
                }
            })
            .collect()
    }

    // -- exchange profile and terminal ------------------------------------------

    fn exchange_profile_mut(&mut self, store_id: &str) -> Result<&mut ExchangeProfile> {
        self.doc
            .stores
            .iter_mut()
            .find(|store| store.id == store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
            .exchange
            .as_mut()
            .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
    }

    /// Background import interval of one exchange; `0` disables it,
    /// otherwise it is kept between a minute and a day.
    pub fn set_import_interval(&mut self, store_id: &str, interval_sec: u32) -> Result<()> {
        let profile = self.exchange_profile_mut(store_id)?;
        profile.import_interval_sec = if interval_sec == 0 {
            0
        } else {
            interval_sec.clamp(60, 86_400)
        };
        self.persist()
    }

    /// Exchanges with a key whose background import is on:
    /// `(store_id, interval_sec, api_key, api_secret)`.
    pub fn import_schedule(&self) -> Vec<(String, u32, String, String)> {
        self.doc
            .exchanges
            .bybit
            .iter()
            .filter(|entry| !entry.api_key.is_empty() && !entry.api_secret.is_empty())
            .filter_map(|entry| {
                let store = self.find_store(&entry.store_id)?;
                let interval = store.exchange.as_ref()?.import_interval_sec;
                (!store.archived && interval > 0).then(|| {
                    (
                        entry.store_id.clone(),
                        interval,
                        entry.api_key.clone(),
                        entry.api_secret.clone(),
                    )
                })
            })
            .collect()
    }

    pub fn terminal_state(&self, store_id: &str) -> Result<TerminalState> {
        self.find_store(store_id)
            .ok_or_else(|| Error::Missing(format!("store {store_id}")))?
            .exchange
            .as_ref()
            .map(|profile| profile.terminal.clone())
            .ok_or(Error::WalletRule(WalletRule::ProviderRequired))
    }

    /// Patch the terminal of one exchange; only the given fields change, so a
    /// timeframe switch never overwrites the watchlist or the drawings.
    pub fn update_terminal_state(
        &mut self,
        store_id: &str,
        patch: TerminalPatch,
    ) -> Result<TerminalState> {
        let terminal = &mut self.exchange_profile_mut(store_id)?.terminal;
        if let Some(watchlist) = patch.watchlist {
            let mut seen = HashSet::new();
            terminal.watchlist = watchlist
                .into_iter()
                .map(|symbol| symbol.trim().to_uppercase())
                .filter(|symbol| !symbol.is_empty() && seen.insert(symbol.clone()))
                .take(200)
                .collect();
        }
        if let Some(symbol) = patch.active_symbol {
            terminal.active_symbol =
                Some(symbol.trim().to_uppercase()).filter(|symbol| !symbol.is_empty());
        }
        if let Some(timeframe) = patch.timeframe {
            terminal.timeframe = timeframe;
        }
        if let Some(indicators) = patch.indicators {
            terminal.indicators = indicators;
        }
        if let Some(layout) = patch.layout {
            terminal.layout = layout;
        }
        let out = terminal.clone();
        self.persist()?;
        Ok(out)
    }

    /// Replace one symbol's drawings; an empty export removes the entry.
    pub fn save_terminal_drawings(
        &mut self,
        store_id: &str,
        symbol: &str,
        json: &str,
    ) -> Result<()> {
        if json.len() > crate::model::store::MAX_DRAWINGS_JSON {
            return Err(Error::TooLarge("chart drawings".into()));
        }
        let symbol = symbol.trim().to_uppercase();
        if symbol.is_empty() {
            return Err(Error::InvalidInput("symbol is required".into()));
        }
        let terminal = &mut self.exchange_profile_mut(store_id)?.terminal;
        let empty = json.trim().is_empty() || json.trim() == "[]";
        if empty {
            terminal.drawings.remove(&symbol);
        } else {
            terminal.drawings.insert(symbol, json.to_string());
        }
        self.persist()
    }

    /// The transaction a staged fill becomes: a spot trade, or a futures
    /// open/close for linear and inverse contracts.
    fn transaction_for_import(&self, imp: &PendingImport) -> Transaction {
        use crate::model::vault::ImportKind;
        let mut meta = TxMeta::new(&imp.comment);
        meta.categories = vec!["trading".to_string()];
        meta.origin = TxOrigin::Import {
            exec_id: imp.exec_id.clone(),
        };
        if !imp.occurred_at.is_empty() {
            meta.timestamp = imp.occurred_at.clone();
        }
        match imp.kind {
            ImportKind::Fill => {}
            ImportKind::Transfer => {
                meta.categories = vec!["transfer".to_string()];
                let side = |account: &Option<String>| {
                    account
                        .as_deref()
                        .and_then(|account| self.bybit_account_store(&imp.store_id, account))
                        .unwrap_or_else(|| imp.store_id.clone())
                };
                return Transaction::Transfer(TransferTx {
                    meta,
                    from: AccountRef::Store(side(&imp.from_account)),
                    to: AccountRef::Store(side(&imp.to_account)),
                    asset: self.asset_for_import(&imp.asset_bought),
                    amount: imp.qty_bought,
                    fee: None,
                    network: None,
                    tx_hash: None,
                    debt: None,
                    price: PriceStamp::default(),
                });
            }
            ImportKind::EarnIncome => {
                meta.categories = vec!["deposit_interest".to_string()];
                return Transaction::Topup(TopupTx {
                    meta,
                    store_id: self.import_target(imp),
                    asset: self.asset_for_import(&imp.asset_bought),
                    amount: imp.qty_bought,
                    fee: None,
                    from: None,
                    details: KindDetails::default(),
                    price: PriceStamp::default(),
                });
            }
        }
        let store_id = self.import_target(imp);
        let is_buy = imp.side.eq_ignore_ascii_case("buy");
        let (base_symbol, quote_symbol) = if is_buy {
            (&imp.asset_bought, &imp.asset_sold)
        } else {
            (&imp.asset_sold, &imp.asset_bought)
        };
        let inverse = imp.category == "inverse";
        let settle_symbol = if inverse { base_symbol } else { quote_symbol };
        // Spot fees come in the coin Bybit names; futures fees are always
        // charged in the settle asset.
        let fee_symbol = match imp.category.as_str() {
            "spot" if !imp.fee_asset.is_empty() => imp.fee_asset.as_str(),
            "spot" => quote_symbol.as_str(),
            _ => settle_symbol.as_str(),
        };
        let fee = (!imp.fee.is_zero()).then(|| Fee {
            asset: self.asset_for_import(fee_symbol),
            amount: imp.fee.abs(),
            price: PriceStamp::default(),
        });
        // A stablecoin quote makes the fill price a USD price.
        let usd_quoted = ["USDT", "USDC", "USD", "FDUSD", "BUSD"]
            .contains(&quote_symbol.to_ascii_uppercase().as_str());
        let stamp = |usd: Option<Amount>| PriceStamp {
            price_usd: usd,
            ..PriceStamp::default()
        };
        if imp.category == "spot" {
            let base_stamp = stamp(usd_quoted.then_some(imp.price));
            let quote_stamp = stamp(usd_quoted.then(|| Amount::new(rust_decimal::Decimal::ONE)));
            let (sold_stamp, bought_stamp) = if is_buy {
                (quote_stamp, base_stamp)
            } else {
                (base_stamp, quote_stamp)
            };
            return Transaction::TradeSpot(TradeSpotTx {
                meta,
                store_id,
                sold: TradeSide {
                    asset: self.asset_for_import(&imp.asset_sold),
                    qty: imp.qty_sold,
                    price: sold_stamp,
                },
                bought: TradeSide {
                    asset: self.asset_for_import(&imp.asset_bought),
                    qty: imp.qty_bought,
                    price: bought_stamp,
                },
                fee,
                liquidity: imp.is_maker.map(|maker| {
                    if maker {
                        Liquidity::Maker
                    } else {
                        Liquidity::Taker
                    }
                }),
                order_id: None,
            });
        }
        let closing = imp.closed_size.is_some_and(|size| size.is_positive());
        // Buying closes a short, selling closes a long.
        let direction = match (is_buy, closing) {
            (true, false) | (false, true) => FuturesDirection::Long,
            (false, false) | (true, true) => FuturesDirection::Short,
        };
        let settle = self.asset_for_import(settle_symbol);
        Transaction::TradeFutures(TradeFuturesTx {
            meta,
            store_id,
            contract: AssetRef::perp("bybit", &imp.symbol),
            settle,
            direction,
            action: if closing {
                FuturesAction::Close
            } else {
                FuturesAction::Open
            },
            quantity: if is_buy { imp.qty_bought } else { imp.qty_sold },
            fill_price: imp.price,
            leverage: None,
            fee,
            closed_pnl: if closing { imp.closed_pnl } else { None },
            order_id: None,
            category: if inverse {
                FuturesCategory::Inverse
            } else {
                FuturesCategory::Linear
            },
            price: stamp((usd_quoted && !inverse).then(|| Amount::new(rust_decimal::Decimal::ONE))),
        })
    }

    /// Turn staged fills into real trades.
    ///
    /// An empty `exec_ids` confirms everything staged. Confirmed entries leave
    /// the staging area so they cannot be imported twice; a fill that fails
    /// validation (its store was archived, say) stays staged.
    pub fn confirm_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        let mut confirmed = 0u32;
        let mut kept = Vec::new();

        for imp in std::mem::take(&mut self.doc.pending_imports) {
            if !(all || wanted.contains(imp.exec_id.as_str())) {
                kept.push(imp);
                continue;
            }
            if self.find_store(&imp.store_id).is_none() {
                continue; // store vanished: drop the staged fill, don't confirm it
            }
            let tx = self.transaction_for_import(&imp);
            if self.validate_transaction(&tx).is_err() {
                kept.push(imp);
                continue;
            }
            self.doc.transactions.push(tx);
            confirmed += 1;
        }

        self.doc.pending_imports = kept;
        if confirmed > 0 {
            self.recompute_holdings();
        }
        self.persist()?;
        Ok(confirmed)
    }

    /// Drop staged fills without creating transactions.
    /// An empty `exec_ids` discards everything.
    pub fn discard_pending_imports(&mut self, exec_ids: &[String]) -> Result<u32> {
        let all = exec_ids.is_empty();
        let before = self.doc.pending_imports.len();
        let wanted: HashSet<&str> = exec_ids.iter().map(String::as_str).collect();
        self.doc
            .pending_imports
            .retain(|pending| !(all || wanted.contains(pending.exec_id.as_str())));
        let discarded = (before - self.doc.pending_imports.len()) as u32;
        if discarded > 0 {
            self.persist()?;
        }
        Ok(discarded)
    }

    /// Build an [`AssetRef`] for an imported symbol.
    ///
    /// The chain is inherited from an asset the user already tracks, so an
    /// imported fill lands in the same bucket as manual entries. Unknown
    /// symbols stay chain-less (`None`) rather than being guessed at — the
    /// user can bind a chain later.
    fn asset_for_import(&self, symbol: &str) -> AssetRef {
        let upper = symbol.trim().to_ascii_uppercase();
        match self
            .doc
            .holdings
            .iter()
            .find(|holding| holding.asset.symbol.eq_ignore_ascii_case(symbol))
            .map(|holding| holding.asset.clone())
        {
            // Reuse the identity the user already tracks (name, coingecko id,
            // chain) so the fill lands in the same bucket as manual entries.
            Some(mut asset) => {
                asset.symbol = upper;
                asset
            }
            // Unseen symbol: leave the chain unresolved so the user can bind
            // it later, rather than attributing the asset to asset wrong network.
            None => {
                let mut asset = AssetRef::crypto(&upper, &upper, None, Chain::Fiat);
                asset.chain = None;
                asset
            }
        }
    }

    // -- helpers ------------------------------------------------------------

    fn ensure_store(&self, id: &str) -> Result<()> {
        match self.doc.stores.iter().find(|store| store.id == id) {
            Some(store) if store.archived => Err(Error::Archived(id.into())),
            Some(_) => Ok(()),
            None => Err(Error::Missing(format!("store {id}"))),
        }
    }
}

// -- reporting DTOs ----------------------------------------------------------

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct AssetStat {
    pub asset: AssetRef,
    pub quantity: Amount,
    pub price_usd: Amount,
    pub value_usd: Amount,
    /// One unit in the base currency at the latest rate.
    pub price_base: Amount,
    pub value_base: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct NamedValue {
    pub label: String,
    pub value_usd: Amount,
    pub value_base: Amount,
    pub pct: f64,
}

/// One store's slice of the portfolio. Unlike [`NamedValue`] it carries the
/// store id so the sidebar can link straight to the wallet page.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreValue {
    pub store_id: String,
    pub name: String,
    pub value_usd: Amount,
    pub value_base: Amount,
    pub pct: f64,
}

/// A single open position inside one store.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StorePosition {
    pub asset: AssetRef,
    pub quantity: Amount,
    /// Weighted average cost per unit in the base currency, if known.
    pub avg_cost: Option<Amount>,
    pub price_usd: Amount,
    /// One unit in the base currency at the latest rate.
    pub price_base: Amount,
    pub value_usd: Amount,
    pub value_base: Amount,
    /// `(price_base − avg_cost) × |quantity|`; zero when `avg_cost` is unknown.
    pub unrealized_base: Amount,
}

/// A direct sub-account row on the wallet page.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreChildRow {
    pub store: Store,
    /// Own value (without the child's sub-accounts).
    pub value_usd: Amount,
    pub value_base: Amount,
    /// Value including the child's own sub-accounts.
    pub subtree_base: Amount,
    /// Share of the parent's subtree value, 0..1.
    pub share: f64,
}

/// One ancestor in the wallet page breadcrumbs.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreCrumb {
    pub id: String,
    pub name: String,
    pub color: String,
}

/// Everything the wallet page needs about one store, in a single round trip.
/// Computed on the fly — nothing here is persisted, so the vault format
/// version is unaffected.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreOverview {
    pub store: Store,
    /// From the top-level wallet down to the direct parent.
    pub ancestors: Vec<StoreCrumb>,
    pub base_currency: String,
    pub fx_per_usd: Amount,
    /// Whether PnL and transaction stats below cover the subtree.
    pub include_children: bool,
    /// Value held on the store itself.
    pub own_usd: Amount,
    pub own_base: Amount,
    /// Own value plus every sub-account at any depth.
    pub subtree_usd: Amount,
    pub subtree_base: Amount,
    pub realized_pnl_base: Amount,
    /// Own positions, sorted by `value_base`, descending.
    pub positions: Vec<StorePosition>,
    /// Positions merged per asset across the whole subtree.
    pub subtree_positions: Vec<StorePosition>,
    /// Open futures positions (derivatives); never part of the totals.
    pub open_positions: Vec<StorePosition>,
    /// Direct sub-accounts.
    pub children: Vec<StoreChildRow>,
    /// Whether a sub-account may be created here (depth and archive rules).
    pub can_add_child: bool,
    pub tx_count: u32,
    /// RFC3339; empty when there are no transactions.
    pub first_tx_at: String,
    pub last_tx_at: String,
}

/// One node of the wallet tree (sidebar, wallets page, pickers).
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreTreeNode {
    pub store: Store,
    /// 0 for a top-level wallet.
    pub depth: u32,
    /// Names from the top-level wallet down to this node, inclusive.
    pub path: Vec<String>,
    pub own_usd: Amount,
    pub own_base: Amount,
    pub subtree_usd: Amount,
    pub subtree_base: Amount,
    /// Transactions with a leg on this very store.
    pub own_tx_count: u32,
    /// `depth < MAX_STORE_DEPTH` and not archived.
    pub can_add_child: bool,
    pub children: Vec<StoreTreeNode>,
}

/// Allowed destinations for moving a sub-account.
#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct StoreMoveTargets {
    /// Flat (no `children`), in tree order; the current parent is left out.
    pub parents: Vec<StoreTreeNode>,
    /// Whether it may become a top-level wallet (no root has its name).
    pub can_detach: bool,
}

/// Lookups shared by every node while building the tree.
struct TreeCtx<'doc> {
    fx: Amount,
    own_usd: HashMap<&'doc str, Amount>,
    tx_count: HashMap<String, u32>,
}

/// Live before archived, then by name.
fn sort_tree(nodes: &mut [StoreTreeNode]) {
    nodes.sort_by(|left, right| {
        left.store
            .archived
            .cmp(&right.store.archived)
            .then_with(|| {
                left.store
                    .name
                    .to_lowercase()
                    .cmp(&right.store.name.to_lowercase())
            })
    });
}

/// The node with `id`, children intact.
fn find_tree_node(nodes: Vec<StoreTreeNode>, id: &str) -> Option<StoreTreeNode> {
    for node in nodes {
        if node.store.id == id {
            return Some(node);
        }
        if let Some(found) = find_tree_node(node.children, id) {
            return Some(found);
        }
    }
    None
}

/// Depth-first flat order without `children`: parent, then its subtree.
/// Sub-account defaults for [`UnlockedVault::create_store`]: the kind's first
/// template, bound to a market, or to USD for currency accounts and BTC for
/// tokens. Real callers pass type and binding explicitly.
fn default_sub_extras(kind: StoreKind) -> StoreExtras {
    let sub_type = kind.sub_types().first().copied();
    let binding = sub_type.map(|sub_type| match sub_type.binding_shape() {
        BindingShape::Market(market) => Binding::Market(market),
        BindingShape::Pair => Binding::Pair {
            base: AssetRef::crypto("BTC", "Bitcoin", Some("bitcoin"), Chain::Bitcoin),
            quote: AssetRef::fiat("USD", "US Dollar"),
        },
        BindingShape::Asset if sub_type == SubType::Token => Binding::Asset(AssetRef::crypto(
            "BTC",
            "Bitcoin",
            Some("bitcoin"),
            Chain::Bitcoin,
        )),
        BindingShape::Asset => Binding::Asset(AssetRef::fiat("USD", "US Dollar")),
    });
    StoreExtras {
        sub_type,
        binding,
        ..StoreExtras::default()
    }
}

/// The top-up that records a new wallet's opening balance.
fn opening_topup(store_id: &str, opening: OpeningBalance) -> Transaction {
    let mut meta = TxMeta::new("Opening balance");
    meta.categories = vec!["opening_balance".to_string()];
    Transaction::Topup(crate::model::transaction::TopupTx {
        meta,
        store_id: store_id.to_string(),
        asset: opening.asset,
        amount: opening.amount,
        fee: None,
        from: None,
        details: crate::model::transaction::KindDetails::default(),
        price: PriceStamp::default(),
    })
}

fn flatten_tree(nodes: Vec<StoreTreeNode>) -> Vec<StoreTreeNode> {
    let mut out = Vec::new();
    for mut node in nodes {
        let children = std::mem::take(&mut node.children);
        out.push(node);
        out.extend(flatten_tree(children));
    }
    out
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PortfolioSummary {
    pub base_currency: String,
    pub fx_per_usd: Amount,
    pub total_usd: Amount,
    pub total_base: Amount,
    pub assets: Vec<AssetStat>,
    pub by_chain: Vec<NamedValue>,
    pub by_store: Vec<StoreValue>,
    /// Everything owned, incl. property and what people owe (base currency).
    pub assets_base: Amount,
    /// Loans and what I owe people, as a positive number.
    pub liabilities_base: Amount,
    pub net_worth_base: Amount,
    /// When the freshest fetched price or FX rate was stored, RFC3339; `None`
    /// before the first successful refresh.
    pub prices_as_of: Option<String>,
    /// When the base currency's rate was taken; `None` means no rate is known
    /// and every `*_base` figure is really USD. Empty for a USD base.
    pub base_fx_as_of: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct HistoryPoint {
    pub ts: String,
    pub ts_ms: i64,
    pub value_usd: Amount,
    pub value_base: Amount,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct PnlLeg {
    pub ts: String,
    pub store_id: String,
    pub asset_key: String,
    /// Absolute size of the closed-out quantity.
    pub quantity: Amount,
    pub pnl_base: Amount,
    pub side: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, ts_rs::TS)]
pub struct RealizedPnl {
    pub total_base: Amount,
    /// Fees paid in scope, at their stamped price; not part of `total_base`.
    pub fees_base: Amount,
    pub legs: Vec<PnlLeg>,
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::model::transaction::{AccountRef, KindDetails, TopupTx};
    use crate::storage::SnapshotFormat;
    use tempfile::tempdir;

    #[test]
    fn payment_reduces_the_balance() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut top_up = income(&store, "100", "top up");
        at(&mut top_up, "2026-01-01T00:00:00Z");
        vault.add_transaction(top_up).unwrap();
        let mut groceries = payment(&store, "30", "groceries");
        at(&mut groceries, "2026-01-02T00:00:00Z");
        vault.add_transaction(groceries).unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "70");
    }

    #[test]
    fn a_zero_or_negative_amount_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        for raw in ["0", "-10"] {
            assert!(matches!(
                vault.add_transaction(payment(&store, raw, "nothing")),
                Err(Error::InvalidInput(_))
            ));
        }
    }

    /// Fees used to be recorded and then ignored, so holdings were too high.
    #[test]
    fn fees_reduce_the_holdings() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault.add_transaction(income(&store, "100", "in")).unwrap();
        let Transaction::Payment(mut rent) = payment(&store, "30", "rent") else {
            unreachable!()
        };
        rent.fee = fee_usd("1.5");
        vault.add_transaction(Transaction::Payment(rent)).unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "68.5");
    }

    #[test]
    fn categories_are_normalized_and_must_exist() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut tx = payment(&store, "30", "groceries");
        tx.meta_mut().categories = vec!["  Groceries ".into(), "groceries".into(), "TAXI".into()];
        let stored = vault.add_transaction(tx).unwrap();
        assert_eq!(stored.meta().categories, vec!["groceries", "taxi"]);

        let mut unknown = payment(&store, "1", "x");
        unknown.meta_mut().categories = vec!["no_such".into()];
        assert!(vault.add_transaction(unknown).is_err());
    }

    #[test]
    fn a_trade_moves_both_sides_of_one_record() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault.add_transaction(income(&store, "1000", "in")).unwrap();
        vault
            .add_transaction(trade(&store, "600", "0.01", "buy"))
            .unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "400");
        assert_eq!(holding_of(&vault, &store, &btc()).to_string(), "0.01");
        let id = vault.doc.transactions[1].meta().id.clone();
        vault.delete_transaction(&id).unwrap();
        assert_eq!(holding_of(&vault, &store, &btc()), Amount::ZERO);
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "1000");
    }

    #[test]
    fn a_trade_of_an_asset_for_itself_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let Transaction::TradeSpot(mut swap) = trade(&store, "1", "1", "swap") else {
            unreachable!()
        };
        swap.bought.asset = usd();
        assert!(vault.add_transaction(Transaction::TradeSpot(swap)).is_err());
    }

    #[test]
    fn a_transfer_moves_quantities_between_stores() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[1].id.clone();
        let cold = vault.doc.stores[2].id.clone();
        vault
            .add_transaction(income_of(&binance, btc(), "1", "50000", "buy"))
            .unwrap();
        let Transaction::Transfer(mut out) = transfer(&binance, &cold, "0.4") else {
            unreachable!()
        };
        out.fee = Some(Fee {
            asset: btc(),
            amount: amount("0.0001"),
            price: PriceStamp::default(),
        });
        vault.add_transaction(Transaction::Transfer(out)).unwrap();
        // the receiver gets exactly the amount; the sender pays the fee on top
        assert_eq!(holding_of(&vault, &binance, &btc()).to_string(), "0.5999");
        assert_eq!(holding_of(&vault, &cold, &btc()).to_string(), "0.4");
        assert_eq!(
            vault.transactions(&StoreScope::store(&cold, false)).len(),
            1
        );
        assert_eq!(
            vault
                .transactions(&StoreScope::store(&binance, false))
                .len(),
            2
        );
    }

    #[test]
    fn a_transfer_to_the_same_store_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        assert!(
            vault
                .add_transaction(transfer(&store, &store, "1"))
                .is_err()
        );
    }

    #[test]
    fn unknown_and_archived_stores_are_rejected() {
        let (_dir, mut vault) = setup();
        assert!(matches!(
            vault.add_transaction(income("nope", "1", "x")),
            Err(Error::Missing(_))
        ));
        let store = vault.doc.stores[1].id.clone();
        vault.archive_store(&store, true).unwrap();
        assert!(matches!(
            vault.add_transaction(income(&store, "1", "x")),
            Err(Error::Archived(_))
        ));
    }

    #[test]
    fn history_is_newest_first() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        for (day, comment) in [("01", "first"), ("03", "third"), ("02", "second")] {
            let mut tx = income(&store, "1", comment);
            at(&mut tx, &format!("2026-01-{day}T00:00:00Z"));
            vault.add_transaction(tx).unwrap();
        }
        let comments: Vec<String> = vault
            .transactions(&StoreScope::default())
            .iter()
            .map(|tx| tx.meta().comment.clone())
            .collect();
        assert_eq!(comments, vec!["third", "second", "first"]);
    }

    /// An on-chain transfer out of a cold wallet with a given network and hash.
    fn on_chain(store: &str, network: Option<Chain>, hash: &str) -> Transaction {
        let asset = match network {
            Some(Chain::Ethereum) => {
                AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum)
            }
            _ => btc(),
        };
        let Transaction::Transfer(mut tx) = transfer_between(
            AccountRef::Store(store.into()),
            AccountRef::Raw("exchange deposit".into()),
            asset,
            "0.1",
        ) else {
            unreachable!()
        };
        tx.network = network;
        tx.tx_hash = Some(hash.into());
        Transaction::Transfer(tx)
    }

    #[test]
    fn on_chain_hashes_are_shape_checked() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[2].id.clone();
        assert!(matches!(
            vault.add_transaction(on_chain(&cold, Some(Chain::Bitcoin), "not-a-hash")),
            Err(Error::TxHashMalformed(_))
        ));
        let bitcoin = "a".repeat(64);
        vault
            .add_transaction(on_chain(&cold, Some(Chain::Bitcoin), &bitcoin))
            .unwrap();
        let evm = format!("0x{}", "b".repeat(64));
        vault
            .add_transaction(on_chain(&cold, Some(Chain::Ethereum), &evm))
            .unwrap();
        // a hash on a network the moved asset does not live on
        assert!(matches!(
            vault
                .add_transaction(on_chain(
                    &cold,
                    Some(Chain::Ethereum),
                    &format!("0x{}", "c".repeat(64))
                ))
                .and_then(|_| vault.add_transaction({
                    let mut tx = on_chain(
                        &cold,
                        Some(Chain::Ethereum),
                        &format!("0x{}", "d".repeat(64)),
                    );
                    if let Transaction::Transfer(inner) = &mut tx {
                        inner.asset = btc();
                    }
                    tx
                })),
            Err(Error::TxHashNetworkMismatch(_))
        ));
    }

    #[test]
    fn editing_keeps_the_id_and_origin() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut original = income(&store, "100", "salary");
        original.meta_mut().origin = TxOrigin::Import {
            exec_id: "e1".into(),
        };
        let stored = vault.add_transaction(original).unwrap();
        let mut edited = income(&store, "250", "salary, corrected");
        edited.meta_mut().origin = TxOrigin::Manual;
        let saved = vault.update_transaction(&stored.meta().id, edited).unwrap();
        assert_eq!(saved.meta().id, stored.meta().id);
        assert_eq!(
            saved.meta().origin,
            TxOrigin::Import {
                exec_id: "e1".into()
            }
        );
        assert_eq!(vault.doc.transactions.len(), 1);
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "250");
        assert!(
            vault
                .update_transaction("missing", income(&store, "1", "x"))
                .is_err()
        );
    }

    /// Property bought with a unit price is valued at it until revalued.
    #[test]
    fn property_is_bought_at_its_unit_price() {
        let (_dir, mut vault) = empty_vault();
        vault.set_base_currency("usd");
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        let thing = AssetRef {
            symbol: "FLAT".into(),
            name: "Flat".into(),
            coingecko_id: None,
            chain: None,
            contract: None,
            quote: "FLAT".into(),
        };
        let buy = |unit_price: Option<&str>| {
            Transaction::Topup(TopupTx {
                meta: meta("bought", &["income_other"]),
                store_id: flat.clone(),
                asset: thing.clone(),
                amount: amount("1"),
                fee: None,
                from: None,
                details: KindDetails {
                    unit_price: unit_price.map(amount),
                    ..KindDetails::default()
                },
                price: PriceStamp::default(),
            })
        };
        assert!(vault.add_transaction(buy(None)).is_err());
        vault.add_transaction(buy(Some("100000"))).unwrap();
        assert_eq!(vault.net_worth().non_financial, amount("100000"));
        vault
            .set_manual_price(&thing.key(), amount("120000"), "usd")
            .unwrap();
        assert_eq!(vault.net_worth().non_financial, amount("120000"));
        vault.clear_manual_price(&thing.key()).unwrap();
        assert_eq!(vault.net_worth().non_financial, amount("100000"));
    }

    #[test]
    fn stores_join_the_budget_by_kind_and_inherit_it() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
        let sub = store(&mut vault, "Spot", StoreKind::Exchange, Some(&exchange));
        let usd_account = store(&mut vault, "USD", StoreKind::Bank, Some(&bank));
        let find = |vault: &UnlockedVault, id: &str| vault.find_store(id).unwrap().in_budget();
        assert!(find(&vault, &bank));
        assert!(!find(&vault, &exchange));
        assert!(!find(&vault, &sub));
        assert!(find(&vault, &usd_account));
        // loan terms only on a loan
        let terms = LiabilityTerms {
            kind: crate::model::store::LiabilityKind::Loan,
            rate: None,
            limit: None,
            due_day: Some(5),
            monthly_payment: None,
        };
        let extras = StoreExtras {
            liability: Some(terms),
            ..StoreExtras::default()
        };
        assert!(
            vault
                .create_store_with("X", StoreKind::Bank, "", "", None, extras.clone())
                .is_err()
        );
        vault
            .create_store_with("Loan", StoreKind::Liability, "", "", None, extras)
            .unwrap();
    }

    // -- wallet tree -----------------------------------------------------------

    fn node(vault: &mut UnlockedVault, name: &str, parent: Option<&str>) -> String {
        vault
            .create_store(
                name,
                StoreKind::Exchange,
                "#000",
                "",
                parent.map(str::to_string),
            )
            .unwrap()
            .id
    }

    fn violation(result: Result<impl std::fmt::Debug>) -> StoreTreeViolation {
        match result {
            Err(Error::StoreTree(violation)) => violation,
            other => panic!("expected a tree violation, got {other:?}"),
        }
    }

    fn rule(result: Result<impl std::fmt::Debug>) -> WalletRule {
        match result {
            Err(Error::WalletRule(rule)) => rule,
            other => panic!("expected a wallet rule, got {other:?}"),
        }
    }

    /// Bybit › Spot, Bybit › Futures, plus an empty exchange Ledger and a Bank.
    fn two_levels() -> (tempfile::TempDir, UnlockedVault, [String; 5]) {
        let dir = tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let bybit = node(&mut vault, "Bybit", None);
        let spot = node(&mut vault, "Spot", Some(&bybit));
        let futures = vault
            .create_store_with(
                "Futures",
                StoreKind::Exchange,
                "",
                "",
                Some(bybit.clone()),
                StoreExtras {
                    sub_type: Some(SubType::Futures),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        let ledger = node(&mut vault, "Ledger", None);
        let bank = vault
            .create_store("Bank", StoreKind::Bank, "", "", None)
            .unwrap()
            .id;
        (dir, vault, [bybit, spot, futures, ledger, bank])
    }

    #[test]
    fn a_store_cannot_become_its_own_parent() {
        let (_dir, mut vault, [_bybit, spot, ..]) = two_levels();
        assert_eq!(
            violation(vault.move_store(&spot, Some(&spot))),
            StoreTreeViolation::Cycle
        );
    }

    #[test]
    fn a_third_level_is_rejected() {
        let (_dir, mut vault, [_bybit, spot, ..]) = two_levels();
        let created = vault.create_store("Deep", StoreKind::Exchange, "", "", Some(spot));
        assert_eq!(violation(created), StoreTreeViolation::TooDeep);
    }

    #[test]
    fn names_are_unique_among_siblings_only() {
        let (_dir, mut vault, [bybit, _spot, _futures, ledger, _bank]) = two_levels();
        // a second "Spot" elsewhere is fine
        node(&mut vault, "Spot", Some(&ledger));
        // a case-different twin under the same parent is not
        let twin = vault.create_store(" spot ", StoreKind::Exchange, "", "", Some(bybit.clone()));
        assert_eq!(violation(twin), StoreTreeViolation::NameTaken);
        let root = vault.create_store("bybit", StoreKind::Exchange, "", "", None);
        assert_eq!(violation(root), StoreTreeViolation::NameTaken);
    }

    #[test]
    fn renaming_into_a_sibling_name_is_rejected() {
        let (_dir, mut vault, [_bybit, spot, futures, ..]) = two_levels();
        assert_eq!(
            violation(vault.update_store(&futures, "SPOT", "", "", StoreExtras::default())),
            StoreTreeViolation::NameTaken
        );
        // keeping its own name (or changing only the case) is fine
        vault
            .update_store(&spot, "SPOT", "#fff", "note", StoreExtras::default())
            .unwrap();
    }

    #[test]
    fn a_top_level_wallet_never_becomes_a_sub_account() {
        let (_dir, mut vault, [bybit, _spot, _futures, ledger, _bank]) = two_levels();
        assert_eq!(
            violation(vault.move_store(&ledger, Some(&bybit))),
            StoreTreeViolation::RootMove
        );
        assert_eq!(
            vault.store_move_targets(&ledger).unwrap(),
            StoreMoveTargets::default()
        );
    }

    #[test]
    fn sub_accounts_move_only_between_wallets_of_their_kind() {
        let (_dir, mut vault, [_bybit, spot, _futures, ledger, bank]) = two_levels();
        assert_eq!(
            rule(vault.move_store(&spot, None)),
            WalletRule::NotApplicable
        );
        assert_eq!(
            rule(vault.move_store(&spot, Some(&bank))),
            WalletRule::SubTypeNotAllowed
        );
        let targets = vault.store_move_targets(&spot).unwrap();
        let ids: Vec<&str> = targets
            .parents
            .iter()
            .map(|node| node.store.id.as_str())
            .collect();
        assert_eq!(ids, vec![ledger.as_str()]);
        assert!(!targets.can_detach);
        vault.move_store(&spot, Some(&ledger)).unwrap();
        assert_eq!(vault.root_of(&spot), ledger);
    }

    #[test]
    fn archiving_cascades_and_restoring_needs_the_parent() {
        let (_dir, mut vault, [bybit, spot, ..]) = two_levels();
        vault.archive_store(&bybit, true).unwrap();
        assert!(vault.find_store(&spot).unwrap().archived);

        let created = vault.create_store("New", StoreKind::Exchange, "", "", Some(bybit.clone()));
        assert_eq!(violation(created), StoreTreeViolation::ParentArchived);
        assert_eq!(
            violation(vault.archive_store(&spot, false).map(|()| "ok")),
            StoreTreeViolation::RestoreParentFirst
        );

        vault.archive_store(&bybit, false).unwrap();
        assert!(!vault.find_store(&spot).unwrap().archived);
    }

    #[test]
    fn tree_totals_roll_up() {
        let dir = tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = store(&mut vault, "Card", StoreKind::Bank, Some(&bank));
        vault.add_transaction(income(&bank, "1", "own")).unwrap();
        vault.add_transaction(income(&card, "10", "card")).unwrap();
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: usd().key(),
            value_usd: amount("1"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
        });

        let tree = vault.store_tree();
        let root = tree.iter().find(|node| node.store.id == bank).unwrap();
        assert_eq!(root.own_usd, amount("1"));
        assert_eq!(root.subtree_usd, amount("11"));
        assert_eq!(root.children[0].path, vec!["Bank", "Card"]);
        assert!(root.can_add_child);
        assert!(!root.children[0].can_add_child);

        let overview = vault.store_overview(&bank, true).unwrap();
        assert_eq!(overview.subtree_usd, amount("11"));
        assert_eq!(overview.children[0].value_usd, amount("10"));
        assert_eq!(overview.tx_count, 2);
        assert_eq!(vault.store_overview(&bank, false).unwrap().tx_count, 1);

        let leaf = vault.store_overview(&card, true).unwrap();
        let crumbs: Vec<&str> = leaf
            .ancestors
            .iter()
            .map(|crumb| crumb.name.as_str())
            .collect();
        assert_eq!(crumbs, vec!["Bank"]);
    }

    #[test]
    fn history_and_pnl_follow_the_scope() {
        let (_dir, mut vault, [bybit, spot, ..]) = two_levels();
        let mut buy = income_of(&spot, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();
        let mut sell = sell_btc(&spot, "0.5", "30000", "60000");
        at(&mut sell, "2026-01-02T00:00:00Z");
        vault.add_transaction(sell).unwrap();

        assert!(vault.store_pnl(&bybit, false).legs.is_empty());
        assert_eq!(vault.store_pnl(&bybit, true).legs.len(), 1);
        assert_eq!(
            vault.store_pnl(&bybit, true).total_base,
            vault.store_pnl(&spot, false).total_base
        );

        let own = vault.store_history(&bybit, false, "usd");
        assert!(own.iter().all(|point| point.value_usd.is_zero()));
        let rolled = vault.store_history(&bybit, true, "usd");
        assert!(rolled.iter().any(|point| !point.value_usd.is_zero()));
        assert_eq!(
            vault.transactions(&StoreScope::store(&bybit, true)).len(),
            2
        );
        assert!(
            vault
                .transactions(&StoreScope::store(&bybit, false))
                .is_empty()
        );
    }

    // -- wallet rules -----------------------------------------------------------

    #[test]
    fn an_opening_balance_is_one_service_topup_or_nothing() {
        let (_dir, mut vault) = empty_vault();
        let opening = |raw: &str, asset: AssetRef| StoreExtras {
            opening: Some(OpeningBalance {
                asset,
                amount: amount(raw),
            }),
            ..StoreExtras::default()
        };
        let bank = vault
            .create_store_with("Bank", StoreKind::Bank, "", "", None, opening("250", usd()))
            .unwrap();
        assert_eq!(holding_of(&vault, &bank.id, &usd()), amount("250"));
        let tx = vault.doc.transactions.last().unwrap();
        assert_eq!(tx.meta().categories, vec!["opening_balance"]);
        // not income: the opening balance stays out of the budget
        assert_eq!(vault.classify_flow(tx), FlowClass::Ignored);

        // a bank holds only fiat: the whole create is refused, nothing remains
        let stores = vault.doc.stores.len();
        assert!(
            vault
                .create_store_with("Bad", StoreKind::Bank, "", "", None, opening("1", btc()))
                .is_err()
        );
        assert_eq!(vault.doc.stores.len(), stores);
        assert_eq!(
            rule(vault.create_store_with(
                "Flat",
                StoreKind::Property,
                "",
                "",
                None,
                opening("1", usd())
            )),
            WalletRule::NotApplicable
        );
    }

    #[test]
    fn the_vault_has_one_cash_wallet_that_stays() {
        let (_dir, mut vault) = empty_vault();
        let cash: Vec<&Store> = vault
            .doc
            .stores
            .iter()
            .filter(|store| store.kind == StoreKind::Cash)
            .collect();
        assert_eq!(cash.len(), 1);
        let cash = cash[0].id.clone();
        assert_eq!(
            rule(vault.create_store("More cash", StoreKind::Cash, "", "", None)),
            WalletRule::NotCreatable
        );
        assert_eq!(
            rule(vault.archive_store(&cash, true).map(|()| "ok")),
            WalletRule::CashLocked
        );
        assert_eq!(
            rule(vault.delete_store(&cash).map(|()| "ok")),
            WalletRule::CashLocked
        );
        assert!(!vault.ensure_cash_wallet("Наличные").unwrap());
    }

    #[test]
    fn sub_accounts_need_an_allowed_type_and_a_binding() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let with = |sub_type, binding| StoreExtras {
            sub_type,
            binding,
            ..StoreExtras::default()
        };
        let create = |vault: &mut UnlockedVault, extras| {
            vault.create_store_with("Sub", StoreKind::Bank, "", "", Some(bank.clone()), extras)
        };
        assert_eq!(
            rule(create(&mut vault, with(None, None))),
            WalletRule::BindingRequired
        );
        assert_eq!(
            rule(create(&mut vault, with(Some(SubType::Spot), None))),
            WalletRule::SubTypeNotAllowed
        );
        assert_eq!(
            rule(create(&mut vault, with(Some(SubType::Card), None))),
            WalletRule::BindingRequired
        );
        assert_eq!(
            rule(create(
                &mut vault,
                with(
                    Some(SubType::Card),
                    Some(Binding::Market(crate::model::store::MarketKind::Spot))
                )
            )),
            WalletRule::BindingMismatch
        );
        let card = create(
            &mut vault,
            with(Some(SubType::Deposit), Some(Binding::Asset(usd()))),
        )
        .unwrap();
        // a deposit is savings, not everyday money
        assert_eq!(card.purpose, Purpose::Savings);
        assert!(!card.in_budget());
    }

    #[test]
    fn a_bound_sub_account_refuses_other_assets() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = store(&mut vault, "Card", StoreKind::Bank, Some(&bank));
        let eur = AssetRef::fiat("EUR", "Euro");
        let refused = vault.add_transaction(income_of(&card, eur.clone(), "5", "1", "eur"));
        assert_eq!(rule(refused), WalletRule::BindingMismatch);
        vault.add_transaction(income(&card, "5", "usd")).unwrap();
        // re-binding to EUR would orphan the USD already booked
        let rebind = vault.update_store(
            &card,
            "Card",
            "",
            "",
            StoreExtras {
                binding: Some(Binding::Asset(eur)),
                ..StoreExtras::default()
            },
        );
        assert_eq!(rule(rebind), WalletRule::BindingMismatch);
    }

    #[test]
    fn a_pair_converts_both_ways() {
        let (_dir, mut vault) = empty_vault();
        let bybit = store(&mut vault, "Bybit", StoreKind::Exchange, None);
        let pair = vault
            .create_store_with(
                "BTCUSD",
                StoreKind::Exchange,
                "",
                "",
                Some(bybit),
                StoreExtras {
                    sub_type: Some(SubType::Pair),
                    binding: Some(Binding::Pair {
                        base: btc(),
                        quote: usd(),
                    }),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault.add_transaction(income(&pair, "100", "fund")).unwrap();
        vault
            .add_transaction(trade(&pair, "50", "0.001", "buy"))
            .unwrap();
        vault
            .add_transaction(sell_btc(&pair, "0.0005", "30", "60000"))
            .unwrap();
        let eth = AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum);
        assert_eq!(
            rule(vault.add_transaction(income_of(&pair, eth, "1", "1", "eth"))),
            WalletRule::BindingMismatch
        );
    }

    #[test]
    fn extras_only_where_they_apply() {
        let (_dir, mut vault) = empty_vault();
        let bad = vault.create_store_with(
            "Bank",
            StoreKind::Bank,
            "",
            "",
            None,
            StoreExtras {
                provider: Some(ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        );
        assert_eq!(rule(bad), WalletRule::NotApplicable);
        let bybit = vault
            .create_store_with(
                "Bybit",
                StoreKind::Exchange,
                "",
                "",
                None,
                StoreExtras {
                    provider: Some(ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        assert_eq!(
            bybit.exchange.as_ref().map(|profile| profile.provider),
            Some(ExchangeProvider::Bybit)
        );
        let flat = store(&mut vault, "Flat", StoreKind::Property, None);
        assert!(vault.find_store(&flat).unwrap().property.is_some());
        let mortgage = vault
            .create_store_with(
                "Mortgage",
                StoreKind::Liability,
                "",
                "",
                None,
                StoreExtras {
                    secured_by: Some(flat.clone()),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        assert_eq!(mortgage.secured_by, Some(flat));
    }

    #[test]
    fn each_exchange_keeps_its_own_terminal() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[1].id.clone();
        let other = vault
            .create_store_with(
                "Bybit 2",
                StoreKind::Exchange,
                "",
                "",
                None,
                StoreExtras {
                    provider: Some(ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault
            .update_terminal_state(
                &binance,
                TerminalPatch {
                    watchlist: Some(vec!["solusdt".into(), "SOLUSDT".into(), " ".into()]),
                    ..TerminalPatch::default()
                },
            )
            .unwrap();
        vault
            .save_terminal_drawings(&binance, "solusdt", r#"[{"id":"1"}]"#)
            .unwrap();
        // a timeframe change keeps the watchlist and the drawings
        let after = vault
            .update_terminal_state(
                &binance,
                TerminalPatch {
                    timeframe: Some("4h".into()),
                    ..TerminalPatch::default()
                },
            )
            .unwrap();
        assert_eq!(after.watchlist, vec!["SOLUSDT".to_string()]);
        assert_eq!(after.timeframe, "4h");
        assert!(after.drawings.contains_key("SOLUSDT"));
        assert_eq!(
            vault.terminal_state(&other).unwrap(),
            TerminalState::default()
        );

        vault
            .save_terminal_drawings(&binance, "SOLUSDT", "[]")
            .unwrap();
        assert!(vault.terminal_state(&binance).unwrap().drawings.is_empty());

        // a wallet without a chosen exchange has no terminal and takes no key
        let cold = vault.doc.stores[2].id.clone();
        assert_eq!(
            rule(vault.terminal_state(&cold)),
            WalletRule::ProviderRequired
        );
        let bare = store(&mut vault, "Bare", StoreKind::Exchange, None);
        assert_eq!(
            rule(vault.set_exchange_key(&bare, "key", "secret")),
            WalletRule::ProviderRequired
        );
    }

    /// The same asset in two stores must be valued once per unit, not once
    /// per holding.
    #[test]
    fn history_counts_an_asset_held_twice_once() {
        let (_dir, vault) = two_stores_with_quotes();
        let last = vault.history_series("usd").pop().unwrap();
        assert_eq!(last.value_usd, amount("150"));
    }

    // -- base currency rate ----------------------------------------------------

    /// The FX cache keeps history oldest first; the summary must use the newest
    /// rate, and say when no rate is known at all.
    #[test]
    fn summary_uses_the_latest_base_rate() {
        let dir = tempdir().unwrap();
        let mut vault = UnlockedVault::create(dir.path().join("fx.wallet"), "pw").unwrap();
        vault.set_base_currency("rub");
        assert_eq!(vault.portfolio_summary().base_fx_as_of, None);
        let point = |rate: &str, ts: &str| crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount(rate),
            ts: ts.into(),
            source: "test".into(),
        };
        vault.apply_fx(&[point("80", "2026-01-01T00:00:00Z")]);
        vault.apply_fx(&[point("95", "2026-03-01T00:00:00Z")]);
        let summary = vault.portfolio_summary();
        assert_eq!(summary.fx_per_usd, amount("95"));
        assert_eq!(
            summary.base_fx_as_of.as_deref(),
            Some("2026-03-01T00:00:00Z")
        );
    }

    // -- password ------------------------------------------------------------

    #[test]
    fn change_password_rekeys_the_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pw.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "old").unwrap();
        vault.change_password("old", "new").unwrap();
        assert!(UnlockedVault::open(path.clone(), "new").is_ok());
        assert!(matches!(
            UnlockedVault::open(path, "old"),
            Err(Error::Secret)
        ));
    }

    #[test]
    fn change_password_rejects_a_wrong_current_one() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pw.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "old").unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(matches!(
            vault.change_password("nope", "new"),
            Err(Error::Secret)
        ));
        assert!(matches!(
            vault.change_password("old", ""),
            Err(Error::InvalidInput(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(UnlockedVault::open(path, "old").is_ok());
    }

    // -- analytics -----------------------------------------------------------

    /// Fixture: two bank stores, each holding a known USD balance.
    fn two_stores_with_quotes() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        let bank_a = vault
            .create_store("A", StoreKind::Bank, "#111", "", None)
            .unwrap();
        let bank_b = vault
            .create_store("B", StoreKind::Bank, "#222", "", None)
            .unwrap();
        let mut left = income(&bank_a.id, "100", "in");
        at(&mut left, "2026-01-01T00:00:00Z");
        vault.add_transaction(left).unwrap();
        let mut right = income(&bank_b.id, "50", "in");
        at(&mut right, "2026-01-01T00:00:00Z");
        vault.add_transaction(right).unwrap();

        // Valuing a store means looking its assets up in the price cache, not
        // reading the price off the transaction, so seed it.
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: crate::model::store::AssetRef::fiat("USD", "US Dollar").key(),
            value_usd: amount("1"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
        });
        (dir, vault)
    }

    #[test]
    fn portfolio_summary_splits_by_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let summary = vault.portfolio_summary();
        assert_eq!(summary.total_usd.to_string(), "150");
        assert_eq!(summary.by_store.len(), 2);
    }

    #[test]
    fn prices_as_of_is_the_freshest_point() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        assert_eq!(vault.prices_as_of(), None);
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: "coin:bitcoin".into(),
            value_usd: crate::service::test_support::amount("1"),
            ts: "2026-01-01T10:00:00Z".into(),
            source: "test".into(),
        });
        vault.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "eur".into(),
            per_usd: crate::service::test_support::amount("1"),
            ts: "2026-01-01T12:30:00.5Z".into(),
            source: "test".into(),
        });
        assert_eq!(
            vault.prices_as_of().as_deref(),
            Some("2026-01-01T12:30:00.5Z")
        );
    }

    #[test]
    fn store_overview_matches_global_slice() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[1].id.clone();
        let overview = vault.store_overview(&first, true).unwrap();
        assert_eq!(overview.own_usd.to_string(), "100");
        assert_eq!(overview.tx_count, 1);
    }

    /// A per-store series must not be valued against the other store's
    /// positions — the leg filter is what guarantees that.
    #[test]
    fn store_history_isolates_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[1].id.clone();
        let mine = vault.store_history(&first, true, "usd");
        let everyone = vault.history_series("usd");
        assert!(mine.len() <= everyone.len());
        for point in &mine {
            assert!(point.value_usd <= everyone.last().unwrap().value_usd);
        }
    }

    /// Realized PnL is measured against the weighted average cost that
    /// `recompute_holdings` also derives, so the two cannot disagree.
    #[test]
    fn store_pnl_filters_by_store() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();

        let mut buy = income_of(&store, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();

        let mut sell = sell_btc(&store, "0.5", "30000", "60000");
        at(&mut sell, "2026-01-02T00:00:00Z");
        vault.add_transaction(sell).unwrap();

        // bought at 50k, sold at 60k, half the position
        let pnl = vault.store_pnl(&store, true);
        assert_eq!(pnl.total_base, amount("5000"));
        assert_eq!(pnl.legs.len(), 1);
        assert_eq!(pnl.legs[0].side, "sell");
    }

    /// The average cost a holding carries must equal the one PnL measures
    /// against, since both come from the same leg replay.
    #[test]
    fn average_cost_is_consistent_with_pnl() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut buy = income_of(&store, btc(), "1", "40000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        vault.add_transaction(buy).unwrap();

        let holding = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == store && holding.asset.key() == btc().key())
            .unwrap();
        assert_eq!(holding.avg_cost.unwrap().to_string(), "40000");
    }

    #[test]
    fn replace_doc_rederives_holdings_and_pins_identity() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault
            .add_transaction(income(&store, "100", "top up"))
            .unwrap();

        let mut edited = vault.doc.clone();
        // A hand-edited holdings array must not be able to disagree with the
        // transactions it summarizes.
        edited.holdings = vec![Holding {
            id: "forged".into(),
            store_id: store.clone(),
            asset: usd(),
            quantity: amount("999999"),
            avg_cost: None,
        }];
        edited.version = crate::VAULT_FORMAT_VERSION + 7;
        let id = vault.doc.id.clone();

        vault.replace_doc(edited).unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "100");
        assert_eq!(vault.doc.version, crate::VAULT_FORMAT_VERSION);
        assert_eq!(vault.doc.id, id);
    }

    #[test]
    fn dev_format_is_json() {
        let (dir, vault) = setup();
        let (_dir2, _vault) = (dir, vault);
        assert_eq!(SnapshotFormat::for_profile(), SnapshotFormat::Json);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::exchange::Execution;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, UnlockedVault) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = UnlockedVault::create(path, "pw").unwrap();
        vault
            .create_store_with(
                "Bybit",
                StoreKind::Exchange,
                "#f0b90b",
                "",
                None,
                StoreExtras {
                    provider: Some(crate::model::store::ExchangeProvider::Bybit),
                    ..StoreExtras::default()
                },
            )
            .unwrap();
        (dir, vault)
    }

    fn fill(exec_id: &str, side: &str, qty: &str, price: &str) -> Execution {
        Execution {
            exec_id: exec_id.into(),
            symbol: "BTCUSDT".into(),
            side: side.into(),
            order_type: "Market".into(),
            exec_price: price.into(),
            exec_qty: qty.into(),
            // Quote leg; callers that care about leg symmetry pass sane values,
            // and the "garbled input" test deliberately passes junk.
            exec_value: "32500".into(),
            exec_fee: "0.5".into(),
            exec_time: "1700000000000".into(),
            is_maker: false,
            closed_size: None,
            category: String::new(),
        }
    }

    fn staged(store: &str, exec_id: &str, side: &str) -> PendingImport {
        PendingImport::from_execution(store, "spot", &fill(exec_id, side, "0.5", "65000")).unwrap()
    }

    /// Staging must never create transactions on its own — that is the whole
    /// point of the confirmation step.
    #[test]
    fn staging_creates_no_transactions() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let count = vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(vault.doc.transactions.len(), 0);
        assert_eq!(vault.pending_imports().len(), 1);
    }

    /// Re-running an import must not duplicate fills: execId is the key.
    #[test]
    fn staging_is_idempotent_per_exec_id() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        assert_eq!(
            vault
                .stage_imports(vec![staged(&store, "e1", "buy")])
                .unwrap(),
            1
        );
        assert_eq!(
            vault
                .stage_imports(vec![staged(&store, "e1", "buy")])
                .unwrap(),
            0
        );
        // A batch that repeats asset known id alongside asset new one adds only the new one.
        assert_eq!(
            vault
                .stage_imports(vec![
                    staged(&store, "e1", "buy"),
                    staged(&store, "e2", "sell"),
                ])
                .unwrap(),
            1
        );
        assert_eq!(vault.pending_imports().len(), 2);
    }

    const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];

    #[test]
    fn bybit_accounts_are_created_once() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let first = vault.ensure_bybit_accounts(&store, ACCOUNT_NAMES).unwrap();
        let again = vault.ensure_bybit_accounts(&store, ACCOUNT_NAMES).unwrap();
        assert_eq!(first, again);
        assert_eq!(vault.children_of(&store).len(), 3);
    }

    /// Keys of a wallet that only trades: the unified account is created, the
    /// empty funding and earn accounts are not.
    #[test]
    fn bybit_accounts_follow_what_the_exchange_holds() {
        use crate::exchange::bybit::{CoinBalance, accounts_in_use};
        let balances = vec![
            ("FUND".to_string(), Vec::new()),
            (
                "UNIFIED".to_string(),
                vec![CoinBalance {
                    coin: "USDT".into(),
                    amount: "10".into(),
                }],
            ),
            ("EARN".to_string(), Vec::new()),
        ];
        let wanted = accounts_in_use(&balances);
        assert_eq!(wanted, [false, true, false]);

        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let ids = vault
            .ensure_bybit_accounts_for(&store, ACCOUNT_NAMES, wanted)
            .unwrap();
        assert!(ids[0].is_none() && ids[2].is_none());
        assert_eq!(vault.children_of(&store).len(), 1);
        assert_eq!(vault.unified_account_of(&store), ids[1]);
        // Trades on the wallet or any of its accounts go to the unified one.
        assert_eq!(vault.unified_account_of(ids[1].as_deref().unwrap()), ids[1]);
    }

    #[test]
    fn legacy_sub_account_types_stay_valid_but_are_no_longer_offered() {
        assert!(!StoreKind::Exchange.sub_types().contains(&SubType::Spot));
        assert!(StoreKind::Exchange.admits_sub_type(SubType::Spot));
        assert!(!StoreKind::Bank.sub_types().contains(&SubType::Account));
        assert!(StoreKind::Bank.admits_sub_type(SubType::Account));
        assert!(!StoreKind::Bank.admits_sub_type(SubType::Token));
    }

    /// Moves between Bybit accounts become transfers between the matching
    /// sub-accounts, Earn payouts income on Earn, fills land on Unified.
    #[test]
    fn bybit_imports_are_routed_by_account() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let [funding, unified, earn] = vault.ensure_bybit_accounts(&store, ACCOUNT_NAMES).unwrap();
        let usdt = AssetRef::fiat("USDT", "USDT");
        let mut seed =
            crate::service::test_support::income_of(&funding, usdt, "100", "1", "salary");
        seed.meta_mut().timestamp = "2023-01-01T00:00:00Z".into();
        vault.add_transaction(seed).unwrap();

        let transfer = PendingImport::from_transfer(
            &store,
            &crate::exchange::InterTransfer {
                transfer_id: "t1".into(),
                coin: "usdt".into(),
                amount: "40".into(),
                from_account_type: "FUND".into(),
                to_account_type: "UNIFIED".into(),
                timestamp: "1700000000000".into(),
                status: "SUCCESS".into(),
            },
        )
        .unwrap();
        let payout = PendingImport::from_earn_yield(
            &store,
            &crate::exchange::EarnYield {
                id: "y1".into(),
                coin: "USDT".into(),
                product_id: "p".into(),
                amount: "0.5".into(),
                created_at: "1700000000000".into(),
            },
        )
        .unwrap();
        assert_eq!(
            vault
                .stage_imports(vec![transfer, payout, staged(&store, "e1", "buy")])
                .unwrap(),
            3
        );
        assert_eq!(vault.confirm_pending_imports(&[]).unwrap(), 3);

        let moved = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.tag() == "transfer")
            .unwrap();
        let Transaction::Transfer(moved) = moved else {
            panic!()
        };
        assert_eq!(moved.from, AccountRef::Store(funding.clone()));
        assert_eq!(moved.to, AccountRef::Store(unified.clone()));
        let income = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.tag() == "topup" && tx.meta().categories == ["deposit_interest"])
            .unwrap();
        let Transaction::Topup(income) = income else {
            panic!()
        };
        assert_eq!(income.store_id, earn);
        let trade = vault
            .doc
            .transactions
            .iter()
            .find(|tx| tx.tag() == "trade_spot")
            .unwrap();
        let Transaction::TradeSpot(trade) = trade else {
            panic!()
        };
        assert_eq!(trade.store_id, unified);

        // The API says Funding holds 70; the ledger has 60 left after the move.
        let report = vault.bybit_reconcile(
            &store,
            &[(
                "FUND".into(),
                vec![crate::exchange::CoinBalance {
                    coin: "USDT".into(),
                    amount: "70".into(),
                }],
            )],
        );
        assert_eq!(report[0].store_id.as_deref(), Some(funding.as_str()));
        assert_eq!(report[0].rows[0].diff, Amount::parse("10").unwrap());
    }

    /// A fill that belongs to a store which no longer exists must be dropped,
    /// never staged against a dangling id.
    #[test]
    fn staging_skips_fills_for_unknown_stores() {
        let (_dir, mut vault) = setup();
        let count = vault
            .stage_imports(vec![staged("ghost-store", "e1", "buy")])
            .unwrap();
        assert_eq!(count, 0);
        assert!(vault.pending_imports().is_empty());
    }

    /// A buy becomes a two-leg trade: quote out (negative), base in (positive).
    #[test]
    fn confirming_a_buy_creates_a_balanced_trade() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();

        assert_eq!(vault.confirm_pending_imports(&[]).unwrap(), 1);
        // One fill is one trade: a single record holding both sides, not two
        // transactions that had to be paired back up by a shared group id.
        assert_eq!(vault.doc.transactions.len(), 1);
        let trade = &vault.doc.transactions[0];
        assert_eq!(trade.tag(), "trade_spot");
        assert_eq!(
            trade.meta().origin,
            TxOrigin::Import {
                exec_id: "e1".into()
            }
        );
        let legs = trade.legs();
        assert_eq!(legs.len(), 3);
        assert_eq!(legs[0].amount.to_string(), "-32500"); // USDT spent
        assert_eq!(legs[1].amount.to_string(), "0.5"); // BTC received
        assert_eq!(legs[1].price_usd, Some(Amount::parse("65000").unwrap()));
        assert_eq!(legs[2].role, LegRole::Fee);
        let Transaction::TradeSpot(spot) = trade else {
            unreachable!()
        };
        assert_eq!(spot.liquidity, Some(Liquidity::Taker));
        // Staging area is drained so the same fill cannot be confirmed twice.
        assert!(vault.pending_imports().is_empty());
        // Holdings reflect the bought asset.
        assert!(
            vault
                .doc
                .holdings
                .iter()
                .any(|holding| holding.asset.symbol == "BTC")
        );
    }

    #[test]
    fn confirming_selectively_leaves_the_rest_staged() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault
            .stage_imports(vec![
                staged(&store, "e1", "buy"),
                staged(&store, "e2", "sell"),
            ])
            .unwrap();

        let confirmed = vault.confirm_pending_imports(&["e1".to_string()]).unwrap();
        assert_eq!(confirmed, 1);
        assert_eq!(vault.doc.transactions.len(), 1);
        let left: Vec<String> = vault
            .pending_imports()
            .into_iter()
            .map(|pending| pending.exec_id)
            .collect();
        assert_eq!(left, vec!["e2".to_string()]);
    }

    #[test]
    fn discarding_creates_nothing_and_drops_the_fill() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        vault
            .stage_imports(vec![staged(&store, "e1", "buy")])
            .unwrap();
        assert_eq!(vault.discard_pending_imports(&[]).unwrap(), 1);
        assert!(vault.pending_imports().is_empty());
        assert_eq!(vault.doc.transactions.len(), 0);
    }

    #[test]
    fn confirmed_fills_reject_a_missing_exec_id() {
        let (_dir, mut vault) = setup();
        assert_eq!(
            vault
                .confirm_pending_imports(&["nope".to_string()])
                .unwrap(),
            0
        );
        assert_eq!(vault.doc.transactions.len(), 0);
    }

    /// Fills without an id, or with an unparsable quantity, must never reach
    /// staging — a garbled fill cannot become a transaction.
    #[test]
    fn unusable_fills_are_rejected_at_the_boundary() {
        assert!(PendingImport::from_execution("s", "spot", &fill("", "buy", "1", "100")).is_none());
        assert!(
            PendingImport::from_execution("s", "spot", &fill("e1", "buy", "not-a-number", "100"))
                .is_none()
        );
        assert!(
            PendingImport::from_execution("s", "spot", &fill("e1", "buy", "0", "100")).is_none()
        );
    }

    #[test]
    fn imported_fill_maps_legs_and_metadata() {
        let imp = PendingImport::from_execution(
            "store-1",
            "linear",
            &fill("exec-9", "Buy", "0.5", "65000"),
        )
        .unwrap();
        assert_eq!(imp.exec_id, "exec-9");
        assert_eq!(imp.store_id, "store-1");
        assert_eq!(imp.category, "linear");
        assert_eq!(imp.side, "buy");
        assert_eq!(imp.asset_bought, "BTC");
        assert_eq!(imp.qty_bought.to_string(), "0.5");
        assert_eq!(imp.asset_sold, "USDT");
        assert_eq!(imp.occurred_at, "2023-11-14T22:13:20Z");
        assert!(imp.comment.contains("bybit"));
    }

    /// Linear fills become futures trades on the futures sub-account, and a
    /// closing fill (it has a closed size) closes the opposite side.
    #[test]
    fn futures_fills_open_and_close_positions_on_the_sub_account() {
        let (_dir, mut vault) = setup();
        let bybit = vault.doc.stores[1].id.clone();
        let futures = vault
            .create_store_with(
                "Futures",
                StoreKind::Exchange,
                "",
                "",
                Some(bybit.clone()),
                StoreExtras {
                    sub_type: Some(SubType::Futures),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
            .id;
        vault.set_exchange_key(&bybit, "key", "secret").unwrap();
        vault
            .set_exchange_targets(&bybit, None, Some(futures.clone()))
            .unwrap();
        assert!(
            vault
                .set_exchange_targets(&bybit, Some("elsewhere".into()), None)
                .is_err()
        );

        let mut open = fill("o1", "Buy", "0.5", "65000");
        open.is_maker = true;
        let mut close = fill("c1", "Sell", "0.5", "66000");
        close.closed_size = Some("0.5".into());
        let mut closing = PendingImport::from_execution(&bybit, "linear", &close).unwrap();
        closing.closed_pnl = Some(Amount::parse("500").unwrap());
        vault
            .stage_imports(vec![
                PendingImport::from_execution(&bybit, "linear", &open).unwrap(),
                closing,
            ])
            .unwrap();
        assert_eq!(vault.confirm_pending_imports(&[]).unwrap(), 2);

        let kinds: Vec<(FuturesDirection, FuturesAction)> = vault
            .doc
            .transactions
            .iter()
            .map(|tx| match tx {
                Transaction::TradeFutures(inner) => {
                    assert_eq!(inner.store_id, futures);
                    (inner.direction, inner.action)
                }
                other => panic!("expected futures, got {}", other.tag()),
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                (FuturesDirection::Long, FuturesAction::Open),
                (FuturesDirection::Long, FuturesAction::Close)
            ]
        );
        // the position is closed, the PnL landed in USDT, minus two fees
        assert!(
            vault
                .doc
                .holdings
                .iter()
                .all(|holding| !holding.asset.is_derivative())
        );
        let usdt = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.asset.symbol == "USDT")
            .unwrap();
        assert_eq!(usdt.quantity.to_string(), "499.0");
        assert_eq!(
            vault.realized_pnl().total_base,
            Amount::parse("500").unwrap()
        );
    }

    /// Perpetual symbols are tagged as derivatives even on a swept import.
    #[test]
    fn category_hint_separates_perps_from_spot() {
        let mut execution = fill("e1", "buy", "1", "100");
        assert_eq!(execution.category_hint(), "spot");
        execution.symbol = "BTCUSDT_PERP".into();
        assert_eq!(execution.category_hint(), "linear");
        // A USDT perpetual has no suffix: only the fetched category tells.
        execution.symbol = "BTCUSDT".into();
        execution.category = "linear".into();
        assert_eq!(execution.category_hint(), "linear");
    }
}

#[cfg(test)]
mod persist_tests {
    use super::*;
    use crate::model::vault::Settings;
    use tempfile::tempdir;

    /// Every settings field the UI can edit must survive a save and a reopen.
    ///
    /// This is the "does it come back after a restart" guarantee: a field that
    /// is not round-tripped here is a setting that silently reverts.
    #[test]
    fn every_settings_field_survives_a_restart() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "pw").unwrap();

        let edited = Settings {
            display_decimals: 6,
            dashboard_widgets: vec![crate::model::vault::DashboardWidgetCfg {
                id: "assets".into(),
                col: 4,
                row: 2,
                width: 6,
                height: 5,
                pinned: true,
                store_id: None,
            }],
            fx_refresh_interval_sec: 900,
            minimize_to_tray: false,
            savings_rate: Amount::parse("0.25").unwrap(),
            daily_budget_mode: crate::model::vault::DailyBudgetMode::UntilIncome,
            budget_currency: "thb".into(),
            notifications: crate::model::notify::NotificationSettings::default(),
        };

        // Applied the way `update_settings` does it: mutate, then persist.
        vault.doc.settings = edited.clone();
        vault.persist().unwrap();

        let (reopened, _) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(
            reopened.settings, edited,
            "settings reverted across a restart"
        );
    }

    /// The same, but through `replace_doc` — the path the raw JSON editor uses.
    /// Editing one field in the editor must not drop the others.
    #[test]
    fn settings_survive_a_json_editor_replace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("editor.wallet");
        let mut vault = UnlockedVault::create(path.clone(), "pw").unwrap();
        vault.doc.settings.display_decimals = 4;
        vault.doc.settings.fx_refresh_interval_sec = 1800;
        vault.persist().unwrap();

        // What the editor sends back: the document it read, decimals changed.
        let (mut draft, _) = VaultFile::open(&path, "pw").unwrap();
        draft.settings.display_decimals = 2;
        vault.replace_doc(draft).unwrap();

        let (reopened, _) = VaultFile::open(&path, "pw").unwrap();
        assert_eq!(reopened.settings.display_decimals, 2);
        assert_eq!(
            reopened.settings.fx_refresh_interval_sec, 1800,
            "an unrelated setting was lost by the editor write"
        );
    }
}
