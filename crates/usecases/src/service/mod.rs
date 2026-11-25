//! Domain services: everything the app does on top of the [`VaultDoc`].
//! This module is pure Rust (no Tauri); the `app` crate only maps commands.

use std::path::PathBuf;

use zeroize::Zeroizing;

use crate::Amount;
use crate::crypto::SealKey;
use crate::error::{Error, Result};
use crate::model::store::{
    AssetRef, Binding, Custody, ExchangeProvider, LiabilityTerms, PropertyInfo, Purpose, SubType,
};
use crate::model::vault::{Settings, VaultDoc};
// What the inline tests reach through `super::*` since the exchange sync and import moved out.
#[cfg(test)]
use crate::error::{StoreTreeViolation, WalletRule};
#[cfg(test)]
use crate::model::chain::Chain;
#[cfg(test)]
use crate::model::store::{Holding, Store, StoreKind, StoreScope, TerminalState};
#[cfg(test)]
use crate::model::transaction::{
    AccountRef, Fee, FuturesAction, FuturesDirection, LegRole, Liquidity, PriceStamp, Transaction,
    TxOrigin,
};
#[cfg(test)]
use crate::model::vault::PendingImport;
use crate::storage::{SnapshotFormat, VaultFile};

pub use password::{CopyPolicy, PlainCopies, RotationReport};
pub use saving::{FlushJob, SaveHook};

mod automations;
mod backups;
mod books;
pub(crate) mod budget;
mod cryptotrack;
pub use cryptotrack::network::Probe;
mod document_edit;
mod exchange_keys;
mod exchange_sync;
mod password;
pub mod security_level;
pub use mwt_cryptotrack::exchange_sync::TerminalPatch;
pub(crate) mod peer_sync;
mod saving;
mod shared_events;
pub use peer_sync::{AppliedBatch, ExternalMerge, PeerRef, SyncTap, UndoReport};
#[cfg(test)]
mod golden_tests;
mod ports;
pub use ports::{BybitApi, LlamaPrices, RpcChain};
mod rates;
#[cfg(any(test, feature = "testkit"))]
pub(crate) mod test_support;
mod validate;
mod workflow_templates;
mod workflows;
pub use mwt_money::stores::{FirstCard, OpeningBalance};

pub use shared_events::{
    EventTotals, ItemShareView, ItemView, MyPosition, PersonView, PlannedPayment, PurchaseView,
    SettlementView,
};

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
    /// Create only: an exchange's futures sub-account keeps its margin in it.
    pub margin: Option<AssetRef>,
}

/// Upper bound on staged imports held in the vault. A background poll must not
/// be able to grow the encrypted file without limit.
pub const MAX_PENDING_IMPORTS: usize = 10_000;

/// The id of the cash wallet the vault creates for itself (R1 of the sync repair).
pub(crate) const CASH_WALLET_ID: &str = "cash";

/// An unlocked vault held in memory for the duration of a session.
/// The master password lives only here (zeroized on drop) — it is never
/// written to disk and is required again on the next unlock.
pub struct Session {
    pub path: PathBuf,
    pub fmt: SnapshotFormat,
    pub doc: VaultDoc,
    /// Portfolio base currency for this session.
    ///
    /// Deliberately *not* part of `doc`: the base currency is an app-level
    /// preference (see [`crate::appconfig::AppSettings`]), so opening a
    /// different vault must not change it. The app layer pushes the current
    /// value in right after the vault is opened.
    pub(crate) base_currency: String,
    /// Whether notices made inside the core (not passed a language) are in
    /// Russian. Runtime only; the app layer sets it before running workflows.
    pub(crate) notice_ru: bool,
    pub(crate) password: Zeroizing<String>,
    /// Released when the vault is locked (dropped).
    pub(crate) _lock: crate::storage::VaultLock,
    /// A sandbox copy: it runs every rule like the real vault but never
    /// writes the file (see [`Self::sandbox`]).
    pub(crate) sandbox: bool,
    /// What the last save looked like, to stamp changes for sync.
    pub(crate) tracker: Option<crate::sync::track::Tracker>,
    /// This device's clock for sync stamps.
    pub(crate) clock: crate::sync::Clock,
    /// Told every save's changed keys while a sync session is live.
    pub(crate) sync_tap: Option<std::sync::Arc<dyn SyncTap>>,
    /// Keys stamped by any path (save, sync calls) and not yet announced: only
    /// the save-time push drains it, so a sync call cannot swallow an edit.
    pub(crate) announce: std::collections::BTreeSet<String>,
    /// The key derived at unlock: saves seal with it, Argon2 does not run again.
    pub(crate) key: Option<std::sync::Arc<SealKey>>,
    /// Unsaved changes and the batched writer (see `saving`).
    pub(crate) session: crate::kernel::session::Write,
    /// What the session knows of the device secrets file (see `exchange_keys`).
    pub(crate) device_keys: exchange_keys::DeviceKeys,
    /// The answer being given to the question of an automation run that asks; set only while
    /// that one answer runs (see `confirm_distribution`).
    pub(crate) confirmation: Option<crate::usecases::planning::automation_rules::Confirmation>,
}

impl Session {
    /// [`Self::announce_to_tap`] for the sync scenarios of `usecases` (that
    /// method is private to `service`; its rule keeps one home in `saving`).
    pub(crate) fn announce_keys_to_tap(&mut self, keys: Vec<String>) {
        self.announce_to_tap(keys);
    }

    /// A fresh read context; the book inside is built once, when first used.
    /// (`Books` is in `money/books.rs`; the vault is its source, see `books.rs`.)
    pub fn books(&self) -> crate::money::books::Books<'_> {
        crate::money::books::Books::new(self)
    }

    pub fn create(path: PathBuf, password: &str) -> Result<Self> {
        Self::create_with(path, Some(password))
    }

    /// A new vault; `None` makes one without a password (its file is not
    /// encrypted, see `crypto`). An empty password is refused: say `None`.
    pub fn create_with(path: PathBuf, password: Option<&str>) -> Result<Self> {
        let password = match password {
            Some("") => {
                return Err(Error::InvalidInput("the password must not be empty".into()));
            }
            other => other.unwrap_or_default(),
        };
        let key = if password.is_empty() {
            SealKey::plain()
        } else {
            SealKey::fresh(password.as_bytes())?
        };
        Self::create_keyed(path, password, key)
    }

    /// A new vault whose key is derived at `kdf` (level 3 is the hardened
    /// profile). An empty password is refused.
    pub fn create_with_kdf(
        path: PathBuf,
        password: &str,
        kdf: crate::crypto::kdf_profile::KdfParams,
    ) -> Result<Self> {
        if password.is_empty() {
            return Err(Error::InvalidInput("the password must not be empty".into()));
        }
        let key = SealKey::fresh_with(password.as_bytes(), kdf)?;
        Self::create_keyed(path, password, key)
    }

    pub(crate) fn create_keyed(path: PathBuf, password: &str, key: SealKey) -> Result<Self> {
        VaultFile::create_keyed(&path, &key)?;
        // A secrets file left by a vault that once had this name is not ours.
        crate::vault_io::device_secrets::remove(&path)?;
        let lock = crate::storage::VaultLock::acquire(&path)?;
        let mut doc = VaultDoc::default();
        // A new vault has nothing to convert: the one-time conversion is done from the start.
        crate::usecases::planning::conversion::mark_done(&mut doc);
        let fmt = SnapshotFormat::for_profile();
        let mut vault = Self {
            path,
            fmt,
            doc,
            base_currency: crate::kernel::session::DEFAULT_BASE_CURRENCY.to_string(),
            notice_ru: true,
            password: Zeroizing::new(password.to_string()),
            _lock: lock,
            sandbox: false,
            tracker: None,
            clock: crate::sync::Clock::default(),
            sync_tap: None,
            announce: std::collections::BTreeSet::new(),
            key: Some(std::sync::Arc::new(key)),
            session: crate::kernel::session::Write::default(),
            device_keys: exchange_keys::DeviceKeys::default(),
            confirmation: None,
        };
        crate::usecases::sync::peer_sync::start_tracking(&mut vault)?;
        crate::usecases::money::stores::ensure_cash_wallet(&mut vault, "Cash")?;
        Ok(vault)
    }

    /// A vault over `doc` that touches no file and runs no Argon2: a plain
    /// key, a detached lock, saves going to `store`. For tests and the test
    /// kit only.
    #[cfg(any(test, feature = "testkit"))]
    pub fn with_persistence(
        doc: VaultDoc,
        store: std::sync::Arc<dyn crate::kernel::persistence::Persistence>,
    ) -> Self {
        Self {
            path: PathBuf::from("in-memory.wallet"),
            fmt: SnapshotFormat::for_profile(),
            doc,
            base_currency: crate::kernel::session::DEFAULT_BASE_CURRENCY.to_string(),
            notice_ru: true,
            password: Zeroizing::new(String::new()),
            _lock: crate::storage::VaultLock::detached(),
            sandbox: false,
            tracker: None,
            clock: crate::sync::Clock::default(),
            sync_tap: None,
            announce: std::collections::BTreeSet::new(),
            key: Some(std::sync::Arc::new(SealKey::plain())),
            session: crate::kernel::session::Write::with_store(store),
            device_keys: exchange_keys::DeviceKeys::default(),
            confirmation: None,
        }
    }

    /// The vault password typed again (before handing the vault to another
    /// device): `Error::Secret` unless it is this vault's. Constant time.
    pub fn check_password(&self, candidate: &str) -> Result<()> {
        use hmac::{KeyInit, Mac};
        // Nothing to check on a vault without a password.
        if !self.is_encrypted() {
            return Ok(());
        }
        type Check = hmac::Hmac<sha2::Sha256>;
        let tag = |password: &str| {
            let mut mac = <Check as KeyInit>::new_from_slice(password.as_bytes())
                .map_err(|_| Error::Secret)?;
            mac.update(b"mwt-password-check");
            Ok::<_, Error>(mac)
        };
        let expected = tag(&self.password)?.finalize().into_bytes();
        tag(candidate)?
            .verify_slice(&expected)
            .map_err(|_| Error::Secret)
    }

    pub fn open(path: PathBuf, password: &str) -> Result<Self> {
        Self::open_as(path, password, None)
    }

    /// Like [`Self::open`], with this device's id known up front so that the
    /// stamps written while opening (default categories, pruning, scrubbing)
    /// already carry its tag instead of an empty one.
    pub fn open_as(path: PathBuf, password: &str, device_id: Option<&str>) -> Result<Self> {
        let (decoded, fmt, key) = VaultFile::open_keyed(&path, password)?;
        // A file without a password opens whatever was typed; none is kept.
        let password = if key.is_plain() { "" } else { password };
        // After the password check: a wrong password must not hint the vault is in use.
        let lock = crate::storage::VaultLock::acquire(&path)?;
        // Under the lock no other instance is raising this vault: a temp file
        // of a re-encryption is what a crash left behind.
        crate::vault_io::local::sweep_reencrypt_leftovers(&path);
        // A v4 file is transitioned once (it comes with a `Carry`); a file of the live
        // format has nothing to convert.
        let migrated = decoded.carry.is_some();
        // The old Bybit queue became operations of the crypto module: the
        // holdings are folded from them once the document is a vault.
        let queue_converted = decoded
            .carry
            .as_ref()
            .is_some_and(|carry| carry.adoption.converted > 0);
        if migrated {
            // The file as it was stays next to the vault before anything rewrites it.
            VaultFile::backup_version(&path, decoded.from_version)?;
        }
        let mut vault = Self {
            path,
            fmt,
            doc: decoded.doc,
            base_currency: crate::kernel::session::DEFAULT_BASE_CURRENCY.to_string(),
            notice_ru: true,
            password: Zeroizing::new(password.to_string()),
            _lock: lock,
            sandbox: false,
            tracker: None,
            clock: crate::sync::Clock::default(),
            sync_tap: None,
            announce: std::collections::BTreeSet::new(),
            key: Some(std::sync::Arc::new(key)),
            session: crate::kernel::session::Write::default(),
            device_keys: exchange_keys::DeviceKeys::default(),
            confirmation: None,
        };
        if let Some(device_id) = device_id {
            crate::usecases::sync::peer_sync::set_device(&mut vault, device_id);
        }
        // Before tracking starts, so the opening entries of envelopes (the
        // same on every device) are never stamped as an edit.
        let envelopes_settled = crate::usecases::household::envelopes::settle_envelopes(&mut vault);
        crate::usecases::sync::peer_sync::start_tracking(&mut vault)?;
        // A merge left the tree or a reference broken in a copy that was
        // saved before the invariant pass existed: fixed once, stamped as an
        // edit of this device.
        let repaired = vault.repair_until_settled()?;
        // A vault from before the single cash wallet gets one on first open.
        crate::usecases::money::stores::ensure_cash_wallet(&mut vault, "Cash")?;
        // Built-in categories added in later versions reach old vaults too.
        let categories_added =
            crate::usecases::planning::passive::ensure_default_categories(&mut vault);
        // The old salary -> deposit automation becomes a template's rule, once (M6, Q14).
        let salary_converted =
            crate::usecases::planning::conversion::convert_salary_to_rule(&mut vault)?;
        // A field became required after the workflow was switched on: off, and said so.
        let incomplete = crate::usecases::workflow::records::disable_incomplete_workflows(
            &mut vault,
            crate::calendar::now_secs(),
        );
        // Files removed last session (their "Undo" is long gone) go now.
        let attachments_pruned =
            crate::usecases::household::attachments_cleanup::prune_attachments(&mut vault);
        // Files no record names (local disk cleanup only).
        crate::usecases::household::attachments_cleanup::sweep_orphan_attachment_files(&vault);
        // Sync history written before secrets were hidden.
        let secrets_scrubbed = crate::usecases::sync::sync_journal::scrub_sync_secrets(&mut vault);
        // Level 3: exchange keys of an old document move to the device file.
        let keys_moved = vault.adopt_device_secrets()?;
        // The raise to level 3 was cut short between the re-encryption and
        // the archiving (spec E11): it left its mark in the document, so the
        // archiving runs here and the mark goes. Without the mark nothing is
        // archived, whatever the level: a vault raised before the crypto
        // module existed keeps the wallets its user keeps by hand.
        let retired = vault.finish_cut_short_raise();
        if queue_converted {
            crate::usecases::cryptotrack::recompute::crypto_sync_holdings(&mut vault)?;
        }
        if migrated
            || keys_moved
            || envelopes_settled
            || categories_added
            || salary_converted
            || incomplete
            || attachments_pruned
            || repaired
            || retired
            || secrets_scrubbed
            || fmt != vault.fmt
        {
            vault.persist()?;
        }
        Ok(vault)
    }

    /// Run the invariant pass until it changes nothing: a fix can change the
    /// key of the notice it announces, so one more pass would write one more
    /// notice on the next open. Bounded, as every pass either fixes or settles.
    /// A single pass is not enough on the v4 fixtures (pinned by
    /// `open_leaves_nothing_for_another_repair_pass` and
    /// `v4_transition_is_idempotent`).
    fn repair_until_settled(&mut self) -> Result<bool> {
        const MAX_PASSES: usize = 3;
        let mut repaired = false;
        for _ in 0..MAX_PASSES {
            if crate::usecases::sync::repair::repair_invariants(self, None)?.is_empty() {
                break;
            }
            repaired = true;
        }
        Ok(repaired)
    }

    /// Language of notices the core makes without being asked for one.
    pub fn set_notice_language(&mut self, ru: bool) {
        self.notice_ru = ru;
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
        crate::usecases::money::portfolio::recompute_holdings(self);
    }

    /// Current base currency, never empty.
    pub fn base_currency(&self) -> &str {
        &self.base_currency
    }

    /// A copy of the vault in memory for emulation: every operation and rule
    /// works on it as on the real one, but nothing is ever written, so trying
    /// things out cannot break the user's data.
    pub fn sandbox(&self) -> Session {
        Session {
            path: self.path.clone(),
            fmt: self.fmt,
            doc: self.doc.clone(),
            base_currency: self.base_currency.clone(),
            notice_ru: self.notice_ru,
            password: Zeroizing::new(String::new()),
            _lock: crate::storage::VaultLock::detached(),
            sandbox: true,
            tracker: None,
            clock: self.clock.clone(),
            sync_tap: None,
            announce: std::collections::BTreeSet::new(),
            key: None,
            session: crate::kernel::session::Write::sandboxed(),
            device_keys: exchange_keys::DeviceKeys::default(),
            confirmation: None,
        }
    }

    /// Copy every later save to `mirror` (`None` stops copying). A sandbox
    /// never gets one: it writes nothing.
    pub fn set_mirror(&mut self, mirror: Option<std::sync::Arc<dyn crate::vault_io::VaultMirror>>) {
        if !self.sandbox {
            self.session.set_mirror(mirror);
        }
    }

    /// Whether saves are copied to an external document.
    pub fn has_mirror(&self) -> bool {
        self.session.has_mirror()
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
        // Sync bookkeeping is the vault's own: an edited copy is ignored, and
        // the edits themselves get stamped on save like any other change.
        doc.sync = std::mem::take(&mut self.doc.sync);
        // Exchange keys are the device's: an edited or restored copy never
        // brings keys in nor drops the ones this device holds.
        doc.exchanges = std::mem::take(&mut self.doc.exchanges);
        self.doc = doc;
        crate::usecases::money::portfolio::recompute_holdings(self);
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
    /// by whoever sits down at it. No `.bak` is written on purpose: a copy
    /// that still opens with the old password would defeat changing a leaked
    /// one. The new key keeps the vault's KDF cost, the device secrets file
    /// follows the key, the pre-sync copies are re-sealed under it; other
    /// copies are left for the user ([`Self::change_password_with`]).
    pub fn change_password(&mut self, current_password: &str, new_password: &str) -> Result<()> {
        // The copies next to the vault stay as they are, except the ones from
        // before a sync, which follow the vault (see `password.rs`).
        self.change_password_with(current_password, new_password, CopyPolicy::default())
            .map(drop)
    }

    /// Re-encode on disk (dev JSON <-> prod binary) — mostly used by tests.
    pub fn switch_format(&mut self, fmt: SnapshotFormat) -> Result<()> {
        self.fmt = fmt;
        self.persist_now()
    }

    // -- settings -----------------------------------------------------------

    /// Apply a partial update of the vault-scoped settings: every field that is
    /// present is written (clamped to a sane range), the rest is kept.
    pub fn update_settings(
        &mut self,
        input: crate::ipc_contract::UpdateSettingsInput,
    ) -> Result<Settings> {
        if [input.manual_income, input.manual_expenses]
            .iter()
            .flatten()
            .any(|value| value.is_negative())
        {
            return Err(Error::InvalidInput(
                "the monthly income and spending cannot be negative".into(),
            ));
        }
        if let Some(rate) = input.savings_rate
            && (rate.is_negative() || rate > Amount::new(rust_decimal::Decimal::ONE))
        {
            return Err(Error::InvalidInput(
                "the savings rate must be between 0 and 100%".into(),
            ));
        }
        // A patch that changes nothing writes nothing (the dashboard re-saves its layout).
        let before = self.doc.settings.clone();
        let settings = &mut self.doc.settings;
        if input.savings_rate.is_some()
            || input.daily_budget_mode.is_some()
            || input.budget_currency.is_some()
        {
            settings.budget_configured = true;
        }
        if let Some(steps) = input.onboarding_skipped {
            let mut kept: Vec<String> = Vec::new();
            for step in steps {
                let step = step.trim().to_string();
                if !step.is_empty() && !kept.contains(&step) {
                    kept.push(step);
                }
            }
            settings.onboarding_skipped = kept;
        }
        if let Some(decimals) = input.display_decimals {
            settings.display_decimals = decimals.clamp(0, 18);
        }
        if let Some(widgets) = input.dashboard_widgets {
            settings.dashboard_widgets = widgets;
        }
        if let Some(period) = input.dashboard_period {
            settings.dashboard_period = period;
        }
        if let Some(hidden) = input.onboarding_checklist_hidden {
            settings.onboarding_checklist_hidden = hidden;
        }
        // `input.crypto` is read for compatibility and ignored: the answer is derived (spec E14).
        if let Some(interval) = input.fx_refresh_interval_sec {
            settings.fx_refresh_interval_sec = interval.clamp(30, 3600);
        }
        if let Some(minimize) = input.minimize_to_tray {
            settings.minimize_to_tray = minimize;
        }
        if let Some(rate) = input.savings_rate {
            settings.savings_rate = rate;
        }
        if let Some(income) = input.manual_income {
            settings.manual_income = income.is_positive().then_some(income);
        }
        if let Some(expenses) = input.manual_expenses {
            settings.manual_expenses = expenses.is_positive().then_some(expenses);
        }
        if let Some(mode) = input.daily_budget_mode {
            settings.daily_budget_mode = mode;
        }
        if let Some(currency) = input.budget_currency {
            settings.budget_currency = currency.trim().to_lowercase();
        }
        // The usual monthly figures are numbers "in the budget currency": fix
        // the currency of whoever types them, so a device with another base
        // does not read them as its own money. Saved figures stay as they are.
        let typed_figure = [input.manual_income, input.manual_expenses]
            .iter()
            .flatten()
            .any(|value| value.is_positive());
        if typed_figure && settings.budget_currency.is_empty() {
            settings.budget_currency = self.base_currency.trim().to_lowercase();
        }
        if let Some(rates) = input.inflation_manual {
            // The form sends the whole table. A shipped value nobody set before
            // is not data; one already in the table stays, so putting a rate
            // back to the shipped number is a real edit that travels.
            let shipped = crate::appconfig::default_inflation();
            let before = std::mem::take(&mut settings.inflation_manual);
            settings.inflation_manual = crate::appconfig::normalize_inflation(&rates)
                .into_iter()
                .filter(|(currency, rate)| {
                    before.contains_key(currency) || shipped.get(currency) != Some(rate)
                })
                .collect();
        }
        if let Some(mut notifications) = input.notifications {
            for cfg in &mut notifications.categories {
                cfg.lead_days = cfg.lead_days.min(90);
            }
            let valid_time =
                notifications
                    .default_time
                    .split_once(':')
                    .is_some_and(|(hour, minute)| {
                        hour.parse::<u32>().is_ok_and(|hour| hour < 24)
                            && minute.parse::<u32>().is_ok_and(|minute| minute < 60)
                    });
            if !valid_time {
                notifications.default_time = "10:00".into();
            }
            settings.notifications = notifications;
        }
        if *settings != before {
            self.persist()?;
        }
        Ok(self.doc.settings.clone())
    }

    // -- stores -------------------------------------------------------------
    //
    // Stores form a tree of at most `MAX_STORE_DEPTH + 1` levels. Every write
    // that places a node goes through `validate_placement`, so the tree rules
    // live in one spot; reads stay lenient so a hand-edited vault still opens.

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

    // -- holdings recomputation ---------------------------------------------

    // -- analytics ----------------------------------------------------------

    // -- exchange keys --------------------------------------------------------

    // -- bybit import staging (D2.4) ---------------------------------------

    // -- exchange profile and terminal ------------------------------------------

    // -- helpers ------------------------------------------------------------

    pub(crate) fn ensure_store(&self, id: &str) -> Result<()> {
        match self.doc.stores.iter().find(|store| store.id == id) {
            Some(store) if store.archived => Err(Error::Archived(id.into())),
            Some(_) => Ok(()),
            None => Err(Error::Missing(format!("store {id}"))),
        }
    }
}

// -- reporting DTOs ----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use crate::model::transaction::{AccountRef, KindDetails, TopupTx};
    use crate::storage::SnapshotFormat;
    use crate::usecases::sync::peer_sync;
    use tempfile::tempdir;

    #[test]
    fn payment_reduces_the_balance() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut top_up = income(&store, "100", "top up");
        at(&mut top_up, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, top_up).unwrap();
        let mut groceries = payment(&store, "30", "groceries");
        at(&mut groceries, "2026-01-02T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, groceries).unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "70");
    }

    #[test]
    fn a_zero_or_negative_amount_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        for raw in ["0", "-10"] {
            assert!(matches!(
                crate::usecases::money::transactions::add_transaction(
                    &mut vault,
                    payment(&store, raw, "nothing")
                ),
                Err(Error::InvalidInput(_))
            ));
        }
    }

    /// Fees used to be recorded and then ignored, so holdings were too high.
    #[test]
    fn fees_reduce_the_holdings() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&store, "100", "in"),
        )
        .unwrap();
        let Transaction::Payment(mut rent) = payment(&store, "30", "rent") else {
            unreachable!()
        };
        rent.fee = fee_usd("1.5");
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            Transaction::Payment(rent),
        )
        .unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "68.5");
    }

    #[test]
    fn categories_are_normalized_and_must_exist() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut tx = payment(&store, "30", "groceries");
        tx.meta_mut().categories = vec!["  Groceries ".into(), "groceries".into(), "TAXI".into()];
        let stored = crate::usecases::money::transactions::add_transaction(&mut vault, tx).unwrap();
        assert_eq!(stored.meta().categories, vec!["groceries", "taxi"]);

        let mut unknown = payment(&store, "1", "x");
        unknown.meta_mut().categories = vec!["no_such".into()];
        assert!(
            crate::usecases::money::transactions::add_transaction(&mut vault, unknown).is_err()
        );
    }

    #[test]
    fn a_trade_moves_both_sides_of_one_record() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&store, "1000", "in"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            trade(&store, "600", "0.01", "buy"),
        )
        .unwrap();
        assert_eq!(holding_of(&vault, &store, &usd()).to_string(), "400");
        assert_eq!(holding_of(&vault, &store, &btc()).to_string(), "0.01");
        let id = vault.doc.transactions[1].meta().id.clone();
        crate::usecases::money::transactions::delete_transaction(&mut vault, &id).unwrap();
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
        assert!(
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                Transaction::TradeSpot(swap)
            )
            .is_err()
        );
    }

    #[test]
    fn a_transfer_moves_quantities_between_stores() {
        let (_dir, mut vault) = setup();
        let binance = vault.doc.stores[1].id.clone();
        let cold = vault.doc.stores[2].id.clone();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&binance, btc(), "1", "50000", "buy"),
        )
        .unwrap();
        let Transaction::Transfer(mut out) = transfer(&binance, &cold, "0.4") else {
            unreachable!()
        };
        out.fee = Some(Fee {
            asset: btc(),
            amount: amount("0.0001"),
            price: PriceStamp::default(),
        });
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            Transaction::Transfer(out),
        )
        .unwrap();
        // the receiver gets exactly the amount; the sender pays the fee on top
        assert_eq!(holding_of(&vault, &binance, &btc()).to_string(), "0.5999");
        assert_eq!(holding_of(&vault, &cold, &btc()).to_string(), "0.4");
        assert_eq!(
            crate::usecases::money::transactions::transactions(
                &vault,
                &StoreScope::store(&cold, false)
            )
            .len(),
            1
        );
        assert_eq!(
            crate::usecases::money::transactions::transactions(
                &vault,
                &StoreScope::store(&binance, false)
            )
            .len(),
            2
        );
    }

    #[test]
    fn a_transfer_to_the_same_store_is_rejected() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        assert!(
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                transfer(&store, &store, "1")
            )
            .is_err()
        );
    }

    #[test]
    fn unknown_and_archived_stores_are_rejected() {
        let (_dir, mut vault) = setup();
        assert!(matches!(
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                income("nope", "1", "x")
            ),
            Err(Error::Missing(_))
        ));
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::money::stores::archive_store(&mut vault, &store, true).unwrap();
        assert!(matches!(
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                income(&store, "1", "x")
            ),
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
            crate::usecases::money::transactions::add_transaction(&mut vault, tx).unwrap();
        }
        let comments: Vec<String> =
            crate::usecases::money::transactions::transactions(&vault, &StoreScope::default())
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
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                on_chain(&cold, Some(Chain::Bitcoin), "not-a-hash")
            ),
            Err(Error::TxHashMalformed(_))
        ));
        let bitcoin = "a".repeat(64);
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            on_chain(&cold, Some(Chain::Bitcoin), &bitcoin),
        )
        .unwrap();
        let evm = format!("0x{}", "b".repeat(64));
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            on_chain(&cold, Some(Chain::Ethereum), &evm),
        )
        .unwrap();
        // a hash on a network the moved asset does not live on
        assert!(matches!(
            crate::usecases::money::transactions::add_transaction(
                &mut vault,
                on_chain(
                    &cold,
                    Some(Chain::Ethereum),
                    &format!("0x{}", "c".repeat(64))
                )
            )
            .and_then(|_| crate::usecases::money::transactions::add_transaction(
                &mut vault,
                {
                    let mut tx = on_chain(
                        &cold,
                        Some(Chain::Ethereum),
                        &format!("0x{}", "d".repeat(64)),
                    );
                    if let Transaction::Transfer(inner) = &mut tx {
                        inner.asset = btc();
                    }
                    tx
                }
            )),
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
        let stored =
            crate::usecases::money::transactions::add_transaction(&mut vault, original).unwrap();
        let mut edited = income(&store, "250", "salary, corrected");
        edited.meta_mut().origin = TxOrigin::Manual;
        let saved = crate::usecases::money::transactions::update_transaction(
            &mut vault,
            &stored.meta().id,
            edited,
        )
        .unwrap();
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
            crate::usecases::money::transactions::update_transaction(
                &mut vault,
                "missing",
                income(&store, "1", "x")
            )
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
                parts: Vec::new(),
                distribution: None,
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
        assert!(
            crate::usecases::money::transactions::add_transaction(&mut vault, buy(None)).is_err()
        );
        crate::usecases::money::transactions::add_transaction(&mut vault, buy(Some("100000")))
            .unwrap();
        assert_eq!(
            crate::usecases::money::capital::net_worth(&vault.books()).non_financial,
            amount("100000")
        );
        crate::usecases::money::capital::set_manual_price(
            &mut vault,
            &thing.key(),
            amount("120000"),
            "usd",
        )
        .unwrap();
        assert_eq!(
            crate::usecases::money::capital::net_worth(&vault.books()).non_financial,
            amount("120000")
        );
        crate::usecases::money::capital::clear_manual_price(&mut vault, &thing.key()).unwrap();
        assert_eq!(
            crate::usecases::money::capital::net_worth(&vault.books()).non_financial,
            amount("100000")
        );
    }

    #[test]
    fn stores_join_the_budget_by_kind_and_inherit_it() {
        let (_dir, mut vault) = empty_vault();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let exchange = store(&mut vault, "Bybit", StoreKind::Exchange, None);
        let sub = store(&mut vault, "Spot", StoreKind::Exchange, Some(&exchange));
        let usd_account = store(&mut vault, "USD", StoreKind::Bank, Some(&bank));
        let find = |vault: &Session, id: &str| {
            crate::money::tree::find_store(&vault.doc, id)
                .unwrap()
                .in_budget()
        };
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
            crate::usecases::money::stores::create_store_with(
                &mut vault,
                "X",
                StoreKind::Bank,
                "",
                "",
                None,
                extras.clone()
            )
            .is_err()
        );
        crate::usecases::money::stores::create_store_with(
            &mut vault,
            "Loan",
            StoreKind::Liability,
            "",
            "",
            None,
            extras,
        )
        .unwrap();
    }

    // -- wallet tree -----------------------------------------------------------

    fn node(vault: &mut Session, name: &str, parent: Option<&str>) -> String {
        crate::usecases::money::stores::create_store(
            vault,
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
    fn two_levels() -> (tempfile::TempDir, Session, [String; 5]) {
        let dir = tempdir().unwrap();
        let mut vault = Session::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let bybit = node(&mut vault, "Bybit", None);
        let spot = node(&mut vault, "Spot", Some(&bybit));
        let futures = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        let bank = crate::usecases::money::stores::create_store(
            &mut vault,
            "Bank",
            StoreKind::Bank,
            "",
            "",
            None,
        )
        .unwrap()
        .id;
        (dir, vault, [bybit, spot, futures, ledger, bank])
    }

    #[test]
    fn a_futures_account_keeps_its_margin_coin_and_nothing_else_does() {
        let (_dir, mut vault, [bybit, ..]) = two_levels();
        let usdt = AssetRef::fiat("USDT", "Tether");
        let mut create = |name: &str, sub_type| {
            crate::usecases::money::stores::create_store_with(
                &mut vault,
                name,
                StoreKind::Exchange,
                "",
                "",
                Some(bybit.clone()),
                StoreExtras {
                    sub_type: Some(sub_type),
                    margin: Some(usdt.clone()),
                    ..StoreExtras::default()
                },
            )
            .unwrap()
        };
        let futures = create("Perps", SubType::Futures);
        assert_eq!(
            futures.margin.map(|asset| asset.symbol),
            Some("USDT".into())
        );
        assert_eq!(
            create("Savings", SubType::Earn).margin,
            None,
            "earn has no margin"
        );
    }

    #[test]
    fn a_store_cannot_become_its_own_parent() {
        let (_dir, mut vault, [_bybit, spot, ..]) = two_levels();
        assert_eq!(
            violation(crate::usecases::money::stores::move_store(
                &mut vault,
                &spot,
                Some(&spot)
            )),
            StoreTreeViolation::Cycle
        );
    }

    #[test]
    fn a_third_level_is_rejected() {
        let (_dir, mut vault, [_bybit, spot, ..]) = two_levels();
        let created = crate::usecases::money::stores::create_store(
            &mut vault,
            "Deep",
            StoreKind::Exchange,
            "",
            "",
            Some(spot),
        );
        assert_eq!(violation(created), StoreTreeViolation::TooDeep);
    }

    #[test]
    fn names_are_unique_among_siblings_only() {
        let (_dir, mut vault, [bybit, _spot, _futures, ledger, _bank]) = two_levels();
        // a second "Spot" elsewhere is fine
        node(&mut vault, "Spot", Some(&ledger));
        // a case-different twin under the same parent is not
        let twin = crate::usecases::money::stores::create_store(
            &mut vault,
            " spot ",
            StoreKind::Exchange,
            "",
            "",
            Some(bybit.clone()),
        );
        assert_eq!(violation(twin), StoreTreeViolation::NameTaken);
        let root = crate::usecases::money::stores::create_store(
            &mut vault,
            "bybit",
            StoreKind::Exchange,
            "",
            "",
            None,
        );
        assert_eq!(violation(root), StoreTreeViolation::NameTaken);
    }

    #[test]
    fn renaming_into_a_sibling_name_is_rejected() {
        let (_dir, mut vault, [_bybit, spot, futures, ..]) = two_levels();
        assert_eq!(
            violation(crate::usecases::money::stores::update_store(
                &mut vault,
                &futures,
                "SPOT",
                "",
                "",
                StoreExtras::default()
            )),
            StoreTreeViolation::NameTaken
        );
        // keeping its own name (or changing only the case) is fine
        crate::usecases::money::stores::update_store(
            &mut vault,
            &spot,
            "SPOT",
            "#fff",
            "note",
            StoreExtras::default(),
        )
        .unwrap();
    }

    #[test]
    fn a_top_level_wallet_never_becomes_a_sub_account() {
        let (_dir, mut vault, [bybit, _spot, _futures, ledger, _bank]) = two_levels();
        assert_eq!(
            violation(crate::usecases::money::stores::move_store(
                &mut vault,
                &ledger,
                Some(&bybit)
            )),
            StoreTreeViolation::RootMove
        );
        assert_eq!(
            crate::usecases::money::stores::store_move_targets(&vault, &ledger).unwrap(),
            crate::usecases::money::stores::StoreMoveTargets::default()
        );
    }

    #[test]
    fn sub_accounts_move_only_between_wallets_of_their_kind() {
        let (_dir, mut vault, [_bybit, spot, _futures, ledger, bank]) = two_levels();
        assert_eq!(
            rule(crate::usecases::money::stores::move_store(
                &mut vault, &spot, None
            )),
            WalletRule::NotApplicable
        );
        assert_eq!(
            rule(crate::usecases::money::stores::move_store(
                &mut vault,
                &spot,
                Some(&bank)
            )),
            WalletRule::SubTypeNotAllowed
        );
        let targets = crate::usecases::money::stores::store_move_targets(&vault, &spot).unwrap();
        let ids: Vec<&str> = targets
            .parents
            .iter()
            .map(|node| node.store.id.as_str())
            .collect();
        assert_eq!(ids, vec![ledger.as_str()]);
        assert!(!targets.can_detach);
        crate::usecases::money::stores::move_store(&mut vault, &spot, Some(&ledger)).unwrap();
        assert_eq!(crate::money::tree::root_of(&vault.doc, &spot), ledger);
    }

    #[test]
    fn archiving_cascades_and_restoring_needs_the_parent() {
        let (_dir, mut vault, [bybit, spot, ..]) = two_levels();
        crate::usecases::money::stores::archive_store(&mut vault, &bybit, true).unwrap();
        assert!(
            crate::money::tree::find_store(&vault.doc, &spot)
                .unwrap()
                .archived
        );

        let created = crate::usecases::money::stores::create_store(
            &mut vault,
            "New",
            StoreKind::Exchange,
            "",
            "",
            Some(bybit.clone()),
        );
        assert_eq!(violation(created), StoreTreeViolation::ParentArchived);
        assert_eq!(
            violation(
                crate::usecases::money::stores::archive_store(&mut vault, &spot, false)
                    .map(|()| "ok")
            ),
            StoreTreeViolation::RestoreParentFirst
        );

        crate::usecases::money::stores::archive_store(&mut vault, &bybit, false).unwrap();
        assert!(
            !crate::money::tree::find_store(&vault.doc, &spot)
                .unwrap()
                .archived
        );
    }

    #[test]
    fn tree_totals_roll_up() {
        let dir = tempdir().unwrap();
        let mut vault = Session::create(dir.path().join("tree.wallet"), "pw").unwrap();
        let bank = store(&mut vault, "Bank", StoreKind::Bank, None);
        let card = store(&mut vault, "Card", StoreKind::Bank, Some(&bank));
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&bank, "1", "own"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&card, "10", "card"),
        )
        .unwrap();
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: usd().key(),
            value_usd: amount("1"),
            ts: "2000-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });

        let tree = crate::money::portfolio::store_tree(&vault.books());
        let root = tree.iter().find(|node| node.store.id == bank).unwrap();
        assert_eq!(root.own_usd, amount("1"));
        assert_eq!(root.subtree_usd, amount("11"));
        assert_eq!(root.children[0].path, vec!["Bank", "Card"]);
        assert!(root.can_add_child);
        assert!(!root.children[0].can_add_child);

        let overview =
            crate::money::portfolio::store_overview(&vault.books(), &bank, true).unwrap();
        assert_eq!(overview.subtree_usd, amount("11"));
        assert_eq!(overview.children[0].value_usd, amount("10"));
        assert_eq!(overview.tx_count, 2);
        assert_eq!(
            crate::money::portfolio::store_overview(&vault.books(), &bank, false)
                .unwrap()
                .tx_count,
            1
        );

        let leaf = crate::money::portfolio::store_overview(&vault.books(), &card, true).unwrap();
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
        crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
        let mut sell = sell_btc(&spot, "0.5", "30000", "60000");
        at(&mut sell, "2026-01-02T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, sell).unwrap();

        assert!(
            crate::money::portfolio::store_pnl(&vault.books(), &bybit, false)
                .legs
                .is_empty()
        );
        assert_eq!(
            crate::money::portfolio::store_pnl(&vault.books(), &bybit, true)
                .legs
                .len(),
            1
        );
        assert_eq!(
            crate::money::portfolio::store_pnl(&vault.books(), &bybit, true).total_base,
            crate::money::portfolio::store_pnl(&vault.books(), &spot, false).total_base
        );

        let own = crate::money::portfolio::store_history(&vault.books(), &bybit, false, "usd");
        assert!(own.iter().all(|point| point.value_usd.is_zero()));
        let rolled = crate::money::portfolio::store_history(&vault.books(), &bybit, true, "usd");
        assert!(rolled.iter().any(|point| !point.value_usd.is_zero()));
        assert_eq!(
            crate::usecases::money::transactions::transactions(
                &vault,
                &StoreScope::store(&bybit, true)
            )
            .len(),
            2
        );
        assert!(
            crate::usecases::money::transactions::transactions(
                &vault,
                &StoreScope::store(&bybit, false)
            )
            .is_empty()
        );
    }

    // -- wallet rules -----------------------------------------------------------

    #[test]
    fn an_opening_balance_keeps_its_date() {
        let (_dir, mut vault) = empty_vault();
        let extras = StoreExtras {
            opening: Some(OpeningBalance {
                asset: usd(),
                amount: amount("95"),
                at: Some("2026-10-01T00:00:00Z".to_string()),
            }),
            ..StoreExtras::default()
        };
        crate::usecases::money::stores::create_store_with(
            &mut vault,
            "Bank",
            StoreKind::Bank,
            "",
            "",
            None,
            extras,
        )
        .unwrap();
        let tx = vault.doc.transactions.last().unwrap();
        assert_eq!(tx.meta().timestamp, "2026-10-01T00:00:00Z");
    }

    #[test]
    fn an_opening_balance_is_one_service_topup_or_nothing() {
        let (_dir, mut vault) = empty_vault();
        let opening = |raw: &str, asset: AssetRef| StoreExtras {
            opening: Some(OpeningBalance {
                asset,
                amount: amount(raw),
                at: None,
            }),
            ..StoreExtras::default()
        };
        let bank = crate::usecases::money::stores::create_store_with(
            &mut vault,
            "Bank",
            StoreKind::Bank,
            "",
            "",
            None,
            opening("250", usd()),
        )
        .unwrap();
        assert_eq!(holding_of(&vault, &bank.id, &usd()), amount("250"));
        let tx = vault.doc.transactions.last().unwrap();
        assert_eq!(tx.meta().categories, vec!["opening_balance"]);
        // not income: the opening balance stays out of the budget
        assert_eq!(
            crate::money::worth::classify_flow(&vault.doc, tx),
            crate::money::worth::FlowClass::Ignored
        );

        // a bank holds only fiat: the whole create is refused, nothing remains
        let stores = vault.doc.stores.len();
        assert!(
            crate::usecases::money::stores::create_store_with(
                &mut vault,
                "Bad",
                StoreKind::Bank,
                "",
                "",
                None,
                opening("1", btc())
            )
            .is_err()
        );
        assert_eq!(vault.doc.stores.len(), stores);
        assert_eq!(
            rule(crate::usecases::money::stores::create_store_with(
                &mut vault,
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
            rule(crate::usecases::money::stores::create_store(
                &mut vault,
                "More cash",
                StoreKind::Cash,
                "",
                "",
                None
            )),
            WalletRule::NotCreatable
        );
        assert_eq!(
            rule(
                crate::usecases::money::stores::archive_store(&mut vault, &cash, true)
                    .map(|()| "ok")
            ),
            WalletRule::CashLocked
        );
        assert_eq!(
            rule(crate::usecases::money::stores::delete_store(&mut vault, &cash).map(|()| "ok")),
            WalletRule::CashLocked
        );
        assert!(
            !crate::usecases::money::stores::ensure_cash_wallet(&mut vault, "Наличные").unwrap()
        );
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
        let create = |vault: &mut Session, extras| {
            crate::usecases::money::stores::create_store_with(
                vault,
                "Sub",
                StoreKind::Bank,
                "",
                "",
                Some(bank.clone()),
                extras,
            )
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
        let refused = crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&card, eur.clone(), "5", "1", "eur"),
        );
        assert_eq!(rule(refused), WalletRule::BindingMismatch);
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&card, "5", "usd"),
        )
        .unwrap();
        // re-binding to EUR would orphan the USD already booked
        let rebind = crate::usecases::money::stores::update_store(
            &mut vault,
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
        let pair = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&pair, "100", "fund"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            trade(&pair, "50", "0.001", "buy"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            sell_btc(&pair, "0.0005", "30", "60000"),
        )
        .unwrap();
        let eth = AssetRef::crypto("ETH", "Ethereum", Some("ethereum"), Chain::Ethereum);
        assert_eq!(
            rule(crate::usecases::money::transactions::add_transaction(
                &mut vault,
                income_of(&pair, eth, "1", "1", "eth")
            )),
            WalletRule::BindingMismatch
        );
    }

    #[test]
    fn extras_only_where_they_apply() {
        let (_dir, mut vault) = empty_vault();
        let bad = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        let bybit = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        assert!(
            crate::money::tree::find_store(&vault.doc, &flat)
                .unwrap()
                .property
                .is_some()
        );
        let mortgage = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        let other = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        crate::usecases::cryptotrack::exchange_import::update_terminal_state(
            &mut vault,
            &binance,
            TerminalPatch {
                watchlist: Some(vec!["solusdt".into(), "SOLUSDT".into(), " ".into()]),
                ..TerminalPatch::default()
            },
        )
        .unwrap();
        crate::usecases::cryptotrack::exchange_import::save_terminal_drawings(
            &mut vault,
            &binance,
            "solusdt",
            r#"[{"id":"1"}]"#,
        )
        .unwrap();
        // a timeframe change keeps the watchlist and the drawings
        let after = crate::usecases::cryptotrack::exchange_import::update_terminal_state(
            &mut vault,
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
            crate::cryptotrack::exchange_import::terminal_state(&vault.doc, &other).unwrap(),
            TerminalState::default()
        );

        crate::usecases::cryptotrack::exchange_import::save_terminal_drawings(
            &mut vault, &binance, "SOLUSDT", "[]",
        )
        .unwrap();
        assert!(
            crate::cryptotrack::exchange_import::terminal_state(&vault.doc, &binance)
                .unwrap()
                .drawings
                .is_empty()
        );

        // a wallet without a chosen exchange has no terminal and takes no key
        let cold = vault.doc.stores[2].id.clone();
        assert_eq!(
            rule(crate::cryptotrack::exchange_import::terminal_state(
                &vault.doc, &cold
            )),
            WalletRule::ProviderRequired
        );
        let bare = store(&mut vault, "Bare", StoreKind::Exchange, None);
        vault.raise_to_level3_for_tests();
        assert_eq!(
            rule(
                crate::usecases::cryptotrack::exchange_import::set_exchange_key(
                    &mut vault, &bare, "key", "secret"
                )
            ),
            WalletRule::ProviderRequired
        );
    }

    /// The same asset in two stores must be valued once per unit, not once
    /// per holding.
    #[test]
    fn history_counts_an_asset_held_twice_once() {
        let (_dir, vault) = two_stores_with_quotes();
        let last = crate::money::portfolio::history_series(&vault.books(), "usd")
            .pop()
            .unwrap();
        assert_eq!(last.value_usd, amount("150"));
    }

    // -- base currency rate ----------------------------------------------------

    /// The FX cache keeps history oldest first; the summary must use the newest
    /// rate, and say when no rate is known at all.
    #[test]
    fn summary_uses_the_latest_base_rate() {
        let dir = tempdir().unwrap();
        let mut vault = Session::create(dir.path().join("fx.wallet"), "pw").unwrap();
        vault.set_base_currency("rub");
        assert_eq!(
            crate::usecases::money::capital::portfolio_summary(&vault.books()).base_fx_as_of,
            None
        );
        let point = |rate: &str, ts: &str| crate::model::vault::FxPoint {
            currency: "rub".into(),
            per_usd: amount(rate),
            ts: ts.into(),
            source: "test".into(),
        };
        crate::usecases::money::rates::apply_fx(&mut vault, &[point("80", "2026-01-01T00:00:00Z")]);
        crate::usecases::money::rates::apply_fx(&mut vault, &[point("95", "2026-03-01T00:00:00Z")]);
        let summary = crate::usecases::money::capital::portfolio_summary(&vault.books());
        assert_eq!(summary.fx_per_usd, amount("95"));
        assert_eq!(
            summary.base_fx_as_of.as_deref(),
            Some("2026-03-01T00:00:00Z")
        );
    }

    // -- crypto direction (derived since spec E14) -----------------------------

    #[test]
    fn settings_with_an_old_crypto_field_still_load() {
        let mut json = serde_json::to_value(Settings::default()).unwrap();
        json.as_object_mut()
            .unwrap()
            .insert("crypto".into(), serde_json::Value::Bool(true));
        let from_json: Settings = serde_json::from_value(json.clone()).unwrap();
        let packed = rmp_serde::to_vec_named(&json).unwrap();
        let from_pack: Settings = rmp_serde::from_slice(&packed).unwrap();
        assert_eq!(from_json, from_pack);
    }

    // -- password ------------------------------------------------------------

    #[test]
    fn change_password_rekeys_the_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pw.wallet");
        let mut vault = Session::create(path.clone(), "old").unwrap();
        vault.change_password("old", "new").unwrap();
        assert!(Session::open(path.clone(), "new").is_ok());
        assert!(matches!(Session::open(path, "old"), Err(Error::Secret)));
    }

    #[test]
    fn change_password_reseals_the_pre_sync_backups() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pw.wallet");
        let mut vault = Session::create(path.clone(), "old").unwrap();
        peer_sync::sync_backup(&mut vault).unwrap();
        peer_sync::sync_backup(&mut vault).unwrap();
        assert!(peer_sync::has_sync_backup(&vault));
        vault.change_password("old", "new").unwrap();
        assert!(peer_sync::has_sync_backup(&vault));
        let copies: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|file| file.to_string_lossy().contains(".presync"))
            .collect();
        assert_eq!(copies.len(), 2);
        for copy in copies {
            assert!(VaultFile::open(&copy, "new").is_ok());
            assert!(matches!(VaultFile::open(&copy, "old"), Err(Error::Secret)));
        }
    }

    #[test]
    fn change_password_rejects_a_wrong_current_one() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pw.wallet");
        let mut vault = Session::create(path.clone(), "old").unwrap();
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
        assert!(Session::open(path, "old").is_ok());
    }

    #[derive(Default)]
    struct RecordingMirror(std::sync::Mutex<Vec<Vec<u8>>>);

    impl crate::vault_io::VaultMirror for RecordingMirror {
        fn push(&self, sealed: &[u8]) {
            self.0.lock().unwrap().push(sealed.to_vec());
        }
    }

    #[test]
    fn a_mirror_gets_exactly_the_saved_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("mirror.wallet");
        let mut vault = Session::create(path.clone(), "pw").unwrap();
        let mirror = std::sync::Arc::new(RecordingMirror::default());
        vault.set_mirror(Some(mirror.clone()));
        assert!(vault.has_mirror());

        vault.persist().unwrap();
        let pushed = mirror.0.lock().unwrap().clone();
        assert_eq!(pushed, vec![std::fs::read(&path).unwrap()]);

        // A sandbox neither writes nor copies.
        let mut sandbox = vault.sandbox();
        sandbox.set_mirror(Some(mirror.clone()));
        assert!(!sandbox.has_mirror());
        sandbox.persist().unwrap();
        assert_eq!(mirror.0.lock().unwrap().len(), 1);

        vault.set_mirror(None);
        vault.persist().unwrap();
        assert_eq!(mirror.0.lock().unwrap().len(), 1);
    }

    // -- analytics -----------------------------------------------------------

    /// Fixture: two bank stores, each holding a known USD balance.
    fn two_stores_with_quotes() -> (tempfile::TempDir, Session) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = Session::create(path, "pw").unwrap();
        let bank_a = crate::usecases::money::stores::create_store(
            &mut vault,
            "A",
            StoreKind::Bank,
            "#111",
            "",
            None,
        )
        .unwrap();
        let bank_b = crate::usecases::money::stores::create_store(
            &mut vault,
            "B",
            StoreKind::Bank,
            "#222",
            "",
            None,
        )
        .unwrap();
        let mut left = income(&bank_a.id, "100", "in");
        at(&mut left, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, left).unwrap();
        let mut right = income(&bank_b.id, "50", "in");
        at(&mut right, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, right).unwrap();

        // Valuing a store means looking its assets up in the price cache, not
        // reading the price off the transaction, so seed it.
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: crate::model::store::AssetRef::fiat("USD", "US Dollar").key(),
            value_usd: amount("1"),
            ts: "2026-01-01T00:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
        (dir, vault)
    }

    #[test]
    fn portfolio_summary_splits_by_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let summary = crate::usecases::money::capital::portfolio_summary(&vault.books());
        assert_eq!(summary.total_usd.to_string(), "150");
        assert_eq!(summary.by_store.len(), 2);
    }

    #[test]
    fn prices_as_of_is_the_freshest_point() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        assert_eq!(crate::money::portfolio::prices_as_of(&vault.doc), None);
        vault.doc.price_cache.push(crate::model::vault::PricePoint {
            asset_key: "coin:bitcoin".into(),
            value_usd: crate::service::test_support::amount("1"),
            ts: "2026-01-01T10:00:00Z".into(),
            source: "test".into(),
            change_24h_pct: None,
        });
        vault.doc.fx_cache.push(crate::model::vault::FxPoint {
            currency: "eur".into(),
            per_usd: crate::service::test_support::amount("1"),
            ts: "2026-01-01T12:30:00.5Z".into(),
            source: "test".into(),
        });
        assert_eq!(
            crate::money::portfolio::prices_as_of(&vault.doc).as_deref(),
            Some("2026-01-01T12:30:00.5Z")
        );
    }

    #[test]
    fn store_overview_matches_global_slice() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[1].id.clone();
        let overview =
            crate::money::portfolio::store_overview(&vault.books(), &first, true).unwrap();
        assert_eq!(overview.own_usd.to_string(), "100");
        assert_eq!(overview.tx_count, 1);
    }

    #[test]
    fn store_tree_lists_top_assets_and_last_activity() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[1].id.clone();
        let mut buy = income_of(&cold, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-03-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
        let mut cash = income_of(&cold, usd(), "100", "1", "cash");
        at(&mut cash, "2026-05-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, cash).unwrap();
        let tree = crate::money::portfolio::store_tree(&vault.books());
        let node = tree
            .iter()
            .find(|node| node.store.id == cold)
            .expect("the wallet is a root");
        let symbols: Vec<&str> = node
            .top_assets
            .iter()
            .map(|asset| asset.symbol.as_str())
            .collect();
        assert_eq!(symbols, ["BTC", "USD"]);
        assert_eq!(node.last_tx_at.as_deref(), Some("2026-05-01T00:00:00Z"));
    }

    #[test]
    fn store_tree_balances_merge_sub_accounts_by_ticker() {
        let (_dir, mut vault) = setup();
        let cold = vault.doc.stores[1].id.clone();
        let mut child = vault.doc.stores[1].clone();
        child.id = "child".into();
        child.name = "Sub".into();
        child.parent_id = Some(cold.clone());
        vault.doc.stores.push(child);
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&cold, btc(), "0.5", "50000", "buy"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of(&cold, usd(), "100", "1", "cash"),
        )
        .unwrap();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income_of("child", btc(), "0.25", "50000", "buy"),
        )
        .unwrap();
        let tree = crate::money::portfolio::store_tree(&vault.books());
        let node = tree
            .iter()
            .find(|node| node.store.id == cold)
            .expect("the wallet is a root");
        let balances: Vec<(&str, Amount)> = node
            .balances
            .iter()
            .map(|balance| (balance.asset.symbol.as_str(), balance.quantity))
            .collect();
        assert_eq!(balances, [("BTC", amount("0.75")), ("USD", amount("100"))]);
        assert_eq!(node.children[0].balances.len(), 1);
    }

    #[test]
    fn store_overview_sums_unrealized_and_reports_prices_as_of() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut buy = income_of(&store, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
        crate::usecases::money::rates::apply_prices(
            &mut vault,
            &[crate::model::vault::PricePoint {
                asset_key: btc().key(),
                value_usd: amount("60000"),
                ts: "2026-10-19T09:00:00Z".into(),
                source: "coingecko".into(),
                change_24h_pct: Some(amount("1.5")),
            }],
        );
        let overview =
            crate::money::portfolio::store_overview(&vault.books(), &store, true).unwrap();
        assert_eq!(overview.unrealized_base, amount("10000"));
        assert_eq!(overview.subtree_unrealized_base, amount("10000"));
        assert_eq!(
            overview.prices_as_of.as_deref(),
            Some("2026-10-19T09:00:00Z")
        );
        let position = &overview.positions[0];
        assert_eq!(position.change_24h_pct, Some(amount("1.5")));
        assert_eq!(
            position.price_info.origin,
            crate::money::price_info::PriceOrigin::Market
        );
    }

    /// A per-store series must not be valued against the other store's
    /// positions — the leg filter is what guarantees that.
    #[test]
    fn store_history_isolates_store() {
        let (_dir, vault) = two_stores_with_quotes();
        let first = vault.doc.stores[1].id.clone();
        let mine = crate::money::portfolio::store_history(&vault.books(), &first, true, "usd");
        let everyone = crate::money::portfolio::history_series(&vault.books(), "usd");
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
        crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();

        let mut sell = sell_btc(&store, "0.5", "30000", "60000");
        at(&mut sell, "2026-01-02T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, sell).unwrap();

        // bought at 50k, sold at 60k, half the position
        let pnl = crate::money::portfolio::store_pnl(&vault.books(), &store, true);
        assert_eq!(pnl.total_base, amount("5000"));
        assert_eq!(pnl.legs.len(), 1);
        assert_eq!(pnl.legs[0].side, "sell");
    }

    /// The average cost a holding carries must equal the one PnL measures
    /// against, since both replays use `cost_basis`. An unpriced buy moves
    /// the quantity but does not dilute the cost.
    #[test]
    fn average_cost_is_consistent_with_pnl() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let cost_of = |vault: &Session| {
            vault
                .doc
                .holdings
                .iter()
                .find(|holding| holding.store_id == store && holding.asset.key() == btc().key())
                .map(|holding| (holding.quantity, holding.avg_cost))
                .unwrap()
        };
        let mut buy = income_of(&store, btc(), "1", "50000", "buy");
        at(&mut buy, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, buy).unwrap();
        let mut unpriced = income_of(&store, btc(), "1", "0", "airdrop");
        at(&mut unpriced, "2026-01-02T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, unpriced).unwrap();
        let (quantity, cost) = cost_of(&vault);
        assert_eq!(quantity, amount("2"));
        assert_eq!(cost, Some(amount("50000")));

        let mut sell = sell_btc(&store, "1", "60000", "60000");
        at(&mut sell, "2026-01-03T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, sell).unwrap();
        let pnl = crate::money::portfolio::realized_pnl(&vault.books());
        assert_eq!(pnl.legs.len(), 1);
        assert_eq!(
            pnl.legs[0].pnl_base,
            (amount("60000") - cost.unwrap()) * amount("1")
        );
        assert_eq!(pnl.legs[0].pnl_base, amount("10000"));
        let (quantity, cost) = cost_of(&vault);
        assert_eq!(quantity, amount("1"));
        assert_eq!(cost, Some(amount("50000")));
    }

    /// Paying 0.5 BTC the vault never saw arrive is spending, not profit,
    /// and leaves no cost behind.
    #[test]
    fn spending_from_an_untracked_crypto_balance_is_not_profit() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let mut sale = sell_btc(&store, "0.5", "30000", "60000");
        at(&mut sale, "2026-01-01T00:00:00Z");
        crate::usecases::money::transactions::add_transaction(&mut vault, sale).unwrap();

        let pnl = crate::money::portfolio::realized_pnl(&vault.books());
        assert_eq!(pnl.total_base, Amount::ZERO);
        assert!(pnl.legs.is_empty());
        let holding = vault
            .doc
            .holdings
            .iter()
            .find(|holding| holding.store_id == store && holding.asset.key() == btc().key())
            .unwrap();
        assert_eq!(holding.quantity, amount("-0.5"));
        assert_eq!(holding.avg_cost, None);
    }

    #[test]
    fn replace_doc_rederives_holdings_and_pins_identity() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&store, "100", "top up"),
        )
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
    // -- the one-time transition of a v4 vault (vault reset) -----------------

    const V4_PASSWORD: &str = "v4-fixture-password";
    const V4_FIXTURES: [(&str, &[u8]); 2] = [
        (
            "json",
            include_bytes!("../../../vault/src/storage/fixtures/v4.json.wallet"),
        ),
        (
            "pack",
            include_bytes!("../../../vault/src/storage/fixtures/v4.pack.wallet"),
        ),
    ];

    fn v4_vault_at(dir: &std::path::Path, bytes: &[u8]) -> PathBuf {
        let path = dir.join("vault.wallet");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn bak_of(path: &std::path::Path) -> PathBuf {
        let mut name = path.file_name().unwrap().to_os_string();
        name.push(".v4.bak");
        path.with_file_name(name)
    }

    fn container_of(path: &std::path::Path) -> u32 {
        crate::crypto::parse_file(&std::fs::read(path).unwrap())
            .unwrap()
            .0
            .version
    }

    #[test]
    fn v4_transition_backs_up_rewrites_and_keeps_the_exchange_keys() {
        for (name, bytes) in V4_FIXTURES {
            let dir = tempdir().unwrap();
            let path = v4_vault_at(dir.path(), bytes);
            let vault = Session::open(path.clone(), V4_PASSWORD).unwrap();
            assert_eq!(std::fs::read(bak_of(&path)).unwrap(), bytes, "{name}");
            assert_eq!(container_of(&path), 2, "{name}");
            let (reread, _) = VaultFile::open(&path, V4_PASSWORD).unwrap();
            assert_eq!(reread.version, crate::VAULT_FORMAT_VERSION, "{name}");
            let expected: serde_json::Value = serde_json::from_str(include_str!(
                "../../../vault/src/storage/fixtures/v4.expected.json"
            ))
            .unwrap();
            let keys = &expected["exchanges"]["bybit"];
            assert!(
                keys.as_array().is_some_and(|list| !list.is_empty()),
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(&vault.doc.exchanges.bybit).unwrap(),
                *keys,
                "{name}"
            );
        }
    }

    /// Opening runs the invariant pass until it settles: another pass finds nothing to fix.
    #[test]
    fn open_leaves_nothing_for_another_repair_pass() {
        for (name, bytes) in V4_FIXTURES {
            let dir = tempdir().unwrap();
            let path = v4_vault_at(dir.path(), bytes);
            let mut vault = Session::open(path, V4_PASSWORD).unwrap();
            assert!(
                crate::usecases::sync::repair::repair_invariants(&mut vault, None)
                    .unwrap()
                    .is_empty(),
                "{name}"
            );
        }
    }

    /// A second open changes neither the vault file nor the backup (bytes compared).
    #[test]
    fn v4_transition_is_idempotent() {
        for (name, bytes) in V4_FIXTURES {
            let dir = tempdir().unwrap();
            let path = v4_vault_at(dir.path(), bytes);
            drop(Session::open(path.clone(), V4_PASSWORD).unwrap());
            let vault_bytes = std::fs::read(&path).unwrap();
            drop(Session::open(path.clone(), V4_PASSWORD).unwrap());
            assert_eq!(std::fs::read(&path).unwrap(), vault_bytes, "{name}");
            assert_eq!(std::fs::read(bak_of(&path)).unwrap(), bytes, "{name}");
        }
    }

    /// The payload of a v4 fixture, with its queue filed under the fixture's exchange wallet so
    /// that the transition converts it, sealed without a password in container 1.
    fn v4_file_with_a_convertible_queue() -> Vec<u8> {
        let (plaintext, fmt) =
            crate::crypto::open_bytes(V4_FIXTURES[0].1, V4_PASSWORD.as_bytes()).unwrap();
        let mut doc: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
        let exchange = doc["stores"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|store| store["kind"] == "exchange")
            .unwrap();
        // The fixture's exchange wallet sits under another wallet; the queue converts under a root.
        exchange["parent_id"] = serde_json::Value::Null;
        let exchange = exchange["id"].clone();
        for entry in doc["pending_imports"].as_array_mut().unwrap() {
            entry["store_id"] = exchange.clone();
        }
        let payload = serde_json::to_vec(&doc).unwrap();
        crate::crypto::seal_plain_in_container(1, fmt, &payload)
    }

    #[test]
    fn v4_transition_refolds_the_holdings_of_the_converted_queue() {
        let dir = tempdir().unwrap();
        let path = v4_vault_at(dir.path(), &v4_file_with_a_convertible_queue());
        let (decoded, _) = VaultFile::open_versioned(&path, "").unwrap();
        let converted = decoded.carry.as_ref().unwrap().adoption.converted;
        assert!(converted > 0);
        let stale = decoded.doc.holdings.clone();
        let vault = Session::open(path.clone(), "").unwrap();
        assert!(vault.doc.crypto_events.len() as u32 >= converted);
        assert_ne!(vault.doc.holdings, stale);
        let (reread, _) = VaultFile::open(&path, "").unwrap();
        assert_eq!(reread.holdings, vault.doc.holdings);
    }

    #[test]
    fn open_copy_and_probe_read_a_container_1_copy_by_its_own_container() {
        let (_dir, vault) = setup();
        for (name, bytes) in V4_FIXTURES {
            let copy = peer_sync::open_copy(&vault, bytes, V4_PASSWORD).unwrap();
            assert_eq!(copy.doc.version, crate::VAULT_FORMAT_VERSION, "{name}");
            let (_, _, key) =
                crate::crypto::open_bytes_keyed(bytes, V4_PASSWORD.as_bytes()).unwrap();
            crate::vault_io::import::probe_open(bytes, &key).unwrap();
        }
        let plain = crate::crypto::SealKey::plain();
        let (plaintext, fmt) =
            crate::crypto::open_bytes(V4_FIXTURES[0].1, V4_PASSWORD.as_bytes()).unwrap();
        let mut old: serde_json::Value = serde_json::from_slice(&plaintext).unwrap();
        old["version"] = 1.into();
        let old_file =
            crate::crypto::seal_plain_in_container(1, fmt, &serde_json::to_vec(&old).unwrap());
        assert!(matches!(
            peer_sync::open_copy(&vault, &old_file, ""),
            Err(crate::Error::LegacyVault(_))
        ));
        assert!(matches!(
            crate::vault_io::import::probe_open(&old_file, &plain),
            Err(crate::Error::LegacyVault(_))
        ));
    }

    /// Interrupted after the backup, before the rewrite: the file is still v4.
    #[test]
    fn v4_transition_after_a_crash_following_the_backup_repeats_cleanly() {
        for (name, bytes) in V4_FIXTURES {
            let dir = tempdir().unwrap();
            let path = v4_vault_at(dir.path(), bytes);
            VaultFile::backup_version(&path, 4).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), bytes, "{name}");
            drop(Session::open(path.clone(), V4_PASSWORD).unwrap());
            assert_eq!(std::fs::read(bak_of(&path)).unwrap(), bytes, "{name}");
            assert_eq!(container_of(&path), 2, "{name}");
        }
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::exchange::Execution;
    use crate::exchange::staging::PendingImportExt;
    use tempfile::tempdir;

    fn setup() -> (tempfile::TempDir, Session) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wallet");
        let mut vault = Session::create(path, "pw").unwrap();
        crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        let count = crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![staged(&store, "e1", "buy")],
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(vault.doc.transactions.len(), 0);
        assert_eq!(
            crate::cryptotrack::exchange_import::pending_imports(&vault.doc).len(),
            1
        );
    }

    /// Re-running an import must not duplicate fills: execId is the key.
    #[test]
    fn staging_is_idempotent_per_exec_id() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::stage_imports(
                &mut vault,
                vec![staged(&store, "e1", "buy")]
            )
            .unwrap(),
            1
        );
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::stage_imports(
                &mut vault,
                vec![staged(&store, "e1", "buy")]
            )
            .unwrap(),
            0
        );
        // A batch that repeats asset known id alongside asset new one adds only the new one.
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::stage_imports(
                &mut vault,
                vec![staged(&store, "e1", "buy"), staged(&store, "e2", "sell"),]
            )
            .unwrap(),
            1
        );
        assert_eq!(
            crate::cryptotrack::exchange_import::pending_imports(&vault.doc).len(),
            2
        );
    }

    const ACCOUNT_NAMES: [&str; 3] = ["Funding", "Unified", "Earn"];

    #[test]
    fn bybit_accounts_are_created_once() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        let first = crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts(
            &mut vault,
            &store,
            ACCOUNT_NAMES,
        )
        .unwrap();
        let again = crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts(
            &mut vault,
            &store,
            ACCOUNT_NAMES,
        )
        .unwrap();
        assert_eq!(first, again);
        assert_eq!(crate::money::tree::children_of(&vault.doc, &store).len(), 3);
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
        let ids = crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts_for(
            &mut vault,
            &store,
            ACCOUNT_NAMES,
            wanted,
        )
        .unwrap();
        assert!(ids[0].is_none() && ids[2].is_none());
        assert_eq!(crate::money::tree::children_of(&vault.doc, &store).len(), 1);
        assert_eq!(
            crate::cryptotrack::exchange_import::unified_account_of(&vault.books(), &store),
            ids[1]
        );
        // Trades on the wallet or any of its accounts go to the unified one.
        assert_eq!(
            crate::cryptotrack::exchange_import::unified_account_of(
                &vault.books(),
                ids[1].as_deref().unwrap()
            ),
            ids[1]
        );
    }

    #[test]
    fn linked_operations_share_a_chain_and_fail_together() {
        use crate::service::test_support::{amount, empty_vault, holding_of, income, usd};
        let (_dir, mut vault) = empty_vault();
        let cash = vault.doc.stores[0].id.clone();
        let saved = crate::usecases::money::transactions::add_linked_transactions(
            &mut vault,
            vec![income(&cash, "10", "a"), income(&cash, "5", "b")],
            None,
        )
        .unwrap();
        assert_eq!(saved.len(), 2);
        let flow = saved[0].meta().flow_id.clone();
        assert!(flow.is_some());
        assert_eq!(saved[1].meta().flow_id, flow);
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("15"));

        let mut broken = income(&cash, "1", "c");
        broken.meta_mut().categories = vec!["no-such-category".into()];
        let before = vault.doc.transactions.len();
        assert!(
            crate::usecases::money::transactions::add_linked_transactions(
                &mut vault,
                vec![income(&cash, "3", "d"), broken],
                None
            )
            .is_err()
        );
        assert_eq!(
            vault.doc.transactions.len(),
            before,
            "none of them is saved"
        );
        assert_eq!(holding_of(&vault, &cash, &usd()), amount("15"));
    }

    #[test]
    fn a_bank_comes_with_its_card_and_the_opening_balance_on_it() {
        use crate::service::test_support::{amount, empty_vault, holding_of};
        let (_dir, mut vault) = empty_vault();
        let bank = crate::usecases::money::stores::create_bank_with_card(
            &mut vault,
            "Sber",
            "#0a0",
            "",
            StoreExtras {
                opening: Some(OpeningBalance {
                    asset: AssetRef::fiat("RUB", "RUB"),
                    amount: amount("1500"),
                    at: None,
                }),
                ..StoreExtras::default()
            },
            FirstCard {
                name: String::new(),
                currency: "rub".into(),
            },
        )
        .unwrap();
        let cards = crate::money::tree::children_of(&vault.doc, &bank.id);
        assert_eq!(cards.len(), 1);
        let card = cards[0].clone();
        assert_eq!(card.sub_type, Some(SubType::Card));
        assert_eq!(card.name, "Sber RUB");
        assert_eq!(
            holding_of(&vault, &card.id, &AssetRef::fiat("RUB", "RUB")),
            amount("1500")
        );

        // A card that cannot hold the opening currency leaves nothing behind.
        let stores = vault.doc.stores.len();
        assert!(
            crate::usecases::money::stores::create_bank_with_card(
                &mut vault,
                "Tinkoff",
                "#ff0",
                "",
                StoreExtras {
                    opening: Some(OpeningBalance {
                        asset: AssetRef::fiat("USD", "USD"),
                        amount: amount("10"),
                        at: None,
                    }),
                    ..StoreExtras::default()
                },
                FirstCard {
                    name: "Card".into(),
                    currency: "rub".into(),
                }
            )
            .is_err()
        );
        assert_eq!(vault.doc.stores.len(), stores);
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
        let [funding, unified, earn] =
            crate::usecases::cryptotrack::exchange_import::ensure_bybit_accounts(
                &mut vault,
                &store,
                ACCOUNT_NAMES,
            )
            .unwrap();
        let usdt = AssetRef::fiat("USDT", "USDT");
        let mut seed =
            crate::service::test_support::income_of(&funding, usdt, "100", "1", "salary");
        seed.meta_mut().timestamp = "2023-01-01T00:00:00Z".into();
        crate::usecases::money::transactions::add_transaction(&mut vault, seed).unwrap();

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
            crate::usecases::cryptotrack::exchange_import::stage_imports(
                &mut vault,
                vec![transfer, payout, staged(&store, "e1", "buy")]
            )
            .unwrap(),
            3
        );
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
                .unwrap(),
            3
        );

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
            // The payout's own category, plus the mark of an imported operation.
            .find(|tx| {
                tx.tag() == "topup"
                    && tx.meta().categories == ["deposit_interest", "exchange_import"]
            })
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
        let report = crate::usecases::cryptotrack::exchange_import::bybit_reconcile(
            &vault.books(),
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
        let count = crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![staged("ghost-store", "e1", "buy")],
        )
        .unwrap();
        assert_eq!(count, 0);
        assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
    }

    /// A buy becomes a two-leg trade: quote out (negative), base in (positive).
    #[test]
    fn confirming_a_buy_creates_a_balanced_trade() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![staged(&store, "e1", "buy")],
        )
        .unwrap();

        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
                .unwrap(),
            1
        );
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
        assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
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
        crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![staged(&store, "e1", "buy"), staged(&store, "e2", "sell")],
        )
        .unwrap();

        let confirmed = crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(
            &mut vault,
            &["e1".to_string()],
        )
        .unwrap();
        assert_eq!(confirmed, 1);
        assert_eq!(vault.doc.transactions.len(), 1);
        let left: Vec<String> = crate::cryptotrack::exchange_import::pending_imports(&vault.doc)
            .into_iter()
            .map(|pending| pending.exec_id)
            .collect();
        assert_eq!(left, vec!["e2".to_string()]);
    }

    #[test]
    fn discarding_creates_nothing_and_drops_the_fill() {
        let (_dir, mut vault) = setup();
        let store = vault.doc.stores[1].id.clone();
        crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![staged(&store, "e1", "buy")],
        )
        .unwrap();
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::discard_pending_imports(&mut vault, &[])
                .unwrap(),
            1
        );
        assert!(crate::cryptotrack::exchange_import::pending_imports(&vault.doc).is_empty());
        assert_eq!(vault.doc.transactions.len(), 0);
    }

    #[test]
    fn confirmed_fills_reject_a_missing_exec_id() {
        let (_dir, mut vault) = setup();
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(
                &mut vault,
                &["nope".to_string()]
            )
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
        let futures = crate::usecases::money::stores::create_store_with(
            &mut vault,
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
        vault.raise_to_level3_for_tests();
        crate::usecases::cryptotrack::exchange_import::set_exchange_key(
            &mut vault, &bybit, "key", "secret",
        )
        .unwrap();
        crate::usecases::cryptotrack::exchange_import::set_exchange_targets(
            &mut vault,
            &bybit,
            None,
            Some(futures.clone()),
        )
        .unwrap();
        assert!(
            crate::usecases::cryptotrack::exchange_import::set_exchange_targets(
                &mut vault,
                &bybit,
                Some("elsewhere".into()),
                None
            )
            .is_err()
        );

        let mut open = fill("o1", "Buy", "0.5", "65000");
        open.is_maker = true;
        let mut close = fill("c1", "Sell", "0.5", "66000");
        close.closed_size = Some("0.5".into());
        let mut closing = PendingImport::from_execution(&bybit, "linear", &close).unwrap();
        closing.closed_pnl = Some(Amount::parse("500").unwrap());
        crate::usecases::cryptotrack::exchange_import::stage_imports(
            &mut vault,
            vec![
                PendingImport::from_execution(&bybit, "linear", &open).unwrap(),
                closing,
            ],
        )
        .unwrap();
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::confirm_pending_imports(&mut vault, &[])
                .unwrap(),
            2
        );

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
            crate::money::portfolio::realized_pnl(&vault.books()).total_base,
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
        let mut vault = Session::create(path.clone(), "pw").unwrap();

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
                period: Some(crate::model::vault::DashPeriod::Quarter),
                scenario_id: Some("scenario".into()),
            }],
            fx_refresh_interval_sec: 900,
            minimize_to_tray: false,
            savings_rate: Amount::parse("0.25").unwrap(),
            daily_budget_mode: crate::model::vault::DailyBudgetMode::UntilIncome,
            budget_currency: "thb".into(),
            notifications: crate::model::notify::NotificationSettings::default(),
            budget_configured: true,
            onboarding_skipped: vec!["debts".into()],
            inflation_source: [("thb".into(), "src-1".into())].into_iter().collect(),
            inflation_mode: [(
                "rub".to_string(),
                crate::model::inflation::InflationMode::Manual,
            )]
            .into(),
            inflation_manual: [("rub".to_string(), Amount::parse("9.5").unwrap())].into(),
            inflation_adopted: true,
            manual_income: Some(Amount::parse("1500").unwrap()),
            manual_expenses: Some(Amount::parse("1100").unwrap()),
            categories_rev: 7,
            freedom_rate: Some(Amount::parse("12").unwrap()),
            carry_since: None,
            pinned: vec![crate::model::entity_ref::Pinned::Entity {
                target: crate::household::relations::EntityRef::new(
                    crate::household::relations::EntityKind::Goal,
                    "g1",
                ),
            }],
            dashboard_period: crate::model::vault::DashPeriod::Year,
            onboarding_checklist_hidden: true,
            schedule_timezone: Some("Asia/Bangkok".into()),
            crypto_raise_pending: true,
            migrations_done: vec!["m6".into()],
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

    #[test]
    fn saving_the_budget_marks_it_configured() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        assert!(!vault.doc.settings.budget_configured);
        let settings = vault
            .update_settings(crate::ipc_contract::UpdateSettingsInput {
                savings_rate: Some(Amount::parse("0.10").unwrap()),
                ..Default::default()
            })
            .unwrap();
        // The default 10 % saved on purpose still counts.
        assert!(settings.budget_configured);
    }

    #[test]
    fn manual_income_is_cleared_by_zero_and_never_negative() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let set = |income: &str| crate::ipc_contract::UpdateSettingsInput {
            manual_income: Some(Amount::parse(income).unwrap()),
            ..Default::default()
        };
        let settings = vault.update_settings(set("2000")).unwrap();
        assert_eq!(settings.manual_income, Some(Amount::parse("2000").unwrap()));
        assert!(vault.update_settings(set("-1")).is_err());
        let settings = vault.update_settings(set("0")).unwrap();
        assert_eq!(settings.manual_income, None);
    }

    #[test]
    fn skipped_checklist_steps_are_kept_once() {
        let (_dir, mut vault) = crate::service::test_support::empty_vault();
        let settings = vault
            .update_settings(crate::ipc_contract::UpdateSettingsInput {
                onboarding_skipped: Some(vec!["debts".into(), " debts ".into(), "goal".into()]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(settings.onboarding_skipped, vec!["debts", "goal"]);
        assert!(!settings.budget_configured);
    }

    /// The same, but through `replace_doc` — the path the raw JSON editor uses.
    /// Editing one field in the editor must not drop the others.
    #[test]
    fn settings_survive_a_json_editor_replace() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("editor.wallet");
        let mut vault = Session::create(path.clone(), "pw").unwrap();
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

    #[test]
    fn editing_a_transaction_keeps_its_utility_link() {
        use super::test_support::{empty_vault, income, store};
        use crate::model::store::StoreKind;
        let (_dir, mut vault) = empty_vault();
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        let mut payment = income(&cash, "100", "water");
        payment.meta_mut().utility_item_id = Some("util-1".into());
        let saved =
            crate::usecases::money::transactions::add_transaction(&mut vault, payment).unwrap();
        let id = saved.meta().id.clone();

        // The form sends no link: the stored one stays.
        let edited = income(&cash, "120", "water");
        let updated =
            crate::usecases::money::transactions::update_transaction(&mut vault, &id, edited)
                .unwrap();
        assert_eq!(updated.meta().utility_item_id.as_deref(), Some("util-1"));
    }

    #[test]
    fn import_schedule_skips_an_archived_store() {
        let (_dir, mut vault) = super::test_support::empty_vault();
        let bybit = crate::usecases::money::stores::create_store_with(
            &mut vault,
            "Bybit",
            StoreKind::Exchange,
            "",
            "",
            None,
            StoreExtras {
                provider: Some(crate::model::store::ExchangeProvider::Bybit),
                ..StoreExtras::default()
            },
        )
        .unwrap()
        .id;
        vault.raise_to_level3_for_tests();
        vault
            .store_device_key(&bybit, "KEYKEYKEY123", "SECRETSECRET456", true)
            .unwrap();
        crate::usecases::cryptotrack::exchange_import::set_import_interval(&mut vault, &bybit, 900)
            .unwrap();
        assert_eq!(
            crate::usecases::cryptotrack::exchange_import::import_schedule(&vault).len(),
            1
        );
        crate::usecases::money::stores::archive_store(&mut vault, &bybit, true).unwrap();
        assert!(crate::usecases::cryptotrack::exchange_import::import_schedule(&vault).is_empty());
    }
}
