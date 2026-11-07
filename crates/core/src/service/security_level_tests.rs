use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::super::StoreExtras;
use super::super::test_support::{empty_vault, setup};
use super::*;
use crate::crypto::kdf_profile::KDF_MEM_KIB_MIN;
use crate::model::store::{ExchangeProvider, StoreKind};

const STRONG: &str = "Tr0ub4dor&3-Horse-Battery!";

fn cheap_kdf() -> KdfParams {
    KdfParams::hardened(KDF_MEM_KIB_MIN, 1, 1)
}

fn wallet(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("test.wallet")
}

fn copies_of(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|file| file.to_string_lossy().ends_with(".bak"))
        .collect();
    found.sort();
    found
}

fn opens(file: &Path, password: &str) -> bool {
    VaultFile::open(file, password).is_ok()
}

#[derive(Default)]
struct RecordingMirror(Mutex<Vec<Vec<u8>>>);

impl crate::vault_io::VaultMirror for RecordingMirror {
    fn push(&self, sealed: &[u8]) {
        self.0.lock().unwrap().push(sealed.to_vec());
    }
}

// -- strength rules -----------------------------------------------------

#[test]
fn a_new_pin_takes_six_digits_or_more() {
    assert!(pin_ok("123456"));
    assert!(pin_ok("0000000000"));
    for refused in ["", "1234", "12345", "12345a", "１２３４５６", "123 456"] {
        assert!(!pin_ok(refused), "{refused:?}");
    }
    assert!(check_new_secret(SecurityLevel::Pin, "482913").is_ok());
    assert!(matches!(
        check_new_secret(SecurityLevel::Pin, "1234"),
        Err(Error::PinFormat)
    ));
    assert!(check_new_secret(SecurityLevel::Ledger, "").is_ok());
}

#[test]
fn the_crypto_password_is_graded_weak_medium_strong() {
    let grade_of = |password: &str| password_strength(password).strength;
    for weak in [
        "short1A!",
        "aaaaaaaaaaaaaaaaaaaa",
        "abcdefghijklmnopqrstu",
        "12345678901234567890",
        "abababababababababab",
    ] {
        assert_eq!(grade_of(weak), PasswordStrength::Weak, "{weak:?}");
    }
    assert_eq!(grade_of("correcthorsebatter"), PasswordStrength::Medium);
    assert_eq!(grade_of(STRONG), PasswordStrength::Strong);
    assert_eq!(
        grade_of("correct horse battery staple"),
        PasswordStrength::Strong
    );
    assert!(!password_strength("fifteen chars!!").long_enough);
    assert!(password_strength("sixteen chars!!!").long_enough);
    assert!(crypto_password_ok("correcthorsebatter"));
    assert!(!crypto_password_ok("aaaaaaaaaaaaaaaaaaaa"));
    assert!(matches!(
        check_new_secret(SecurityLevel::Crypto, "123456"),
        Err(Error::PasswordTooWeak)
    ));
}

#[test]
fn an_old_four_digit_password_still_opens_at_level_two() {
    let dir = tempfile::tempdir().unwrap();
    let path = wallet(&dir);
    let vault = UnlockedVault::create(path.clone(), "1234").unwrap();
    drop(vault);
    let reopened = UnlockedVault::open(path.clone(), "1234").unwrap();
    assert_eq!(reopened.security_level(), SecurityLevel::Pin);
    assert_eq!(level_of_vault_file(&path).unwrap(), SecurityLevel::Pin);
}

// -- the raise ----------------------------------------------------------

#[test]
fn the_raise_reaches_level_three_and_leaves_no_cheaper_copy() {
    let (dir, mut vault) = setup();
    let stores = vault.doc.stores.len();
    // Copies of every kind at level 2: the slot, a migration copy, one from
    // before a sync, and one under a password this raise does not know.
    std::fs::copy(&vault.path, dir.path().join("test.wallet.bak")).unwrap();
    std::fs::copy(&vault.path, dir.path().join("test.wallet.v1.bak")).unwrap();
    vault.sync_backup().unwrap();
    let stranger = tempfile::tempdir().unwrap();
    let other = UnlockedVault::create(stranger.path().join("o.wallet"), "9999").unwrap();
    std::fs::copy(&other.path, dir.path().join("test.wallet.old.bak")).unwrap();

    let mirror = Arc::new(RecordingMirror::default());
    vault.set_mirror(Some(mirror.clone()));
    let mut stages = Vec::new();
    let report = vault
        .raise_to_crypto("pw", STRONG, cheap_kdf(), |stage| stages.push(stage))
        .unwrap();

    assert_eq!(
        stages,
        [
            RaiseStage::Backup,
            RaiseStage::Reencrypt,
            RaiseStage::Verify
        ]
    );
    assert_eq!(vault.security_level(), SecurityLevel::Crypto);
    assert_eq!(vault.session_key().unwrap().kdf(), cheap_kdf());
    assert!(report.mirrored);
    let on_disk = std::fs::read(&vault.path).unwrap();
    assert_eq!(mirror.0.lock().unwrap().last(), Some(&on_disk));
    assert_eq!(report.deleted, ["test.wallet.old.bak"]);
    // The wizard's backup was re-sealed: the old PIN opens nothing now.
    assert!(opens(&report.backup, STRONG));
    for copy in copies_of(dir.path()) {
        assert_eq!(
            level_of_vault_file(&copy).unwrap(),
            SecurityLevel::Crypto,
            "{copy:?}"
        );
        assert!(opens(&copy, STRONG), "{copy:?}");
        assert!(!opens(&copy, "pw"), "{copy:?}");
    }
    drop(vault);
    assert!(matches!(
        UnlockedVault::open(wallet(&dir), "pw"),
        Err(Error::Secret)
    ));
    let reopened = UnlockedVault::open(wallet(&dir), STRONG).unwrap();
    assert_eq!(reopened.security_level(), SecurityLevel::Crypto);
    assert_eq!(reopened.doc.stores.len(), stores);
}

#[test]
fn a_crash_at_any_step_leaves_one_level_and_the_data() {
    for step in [
        RaiseStep::BackedUp,
        RaiseStep::Reencrypted,
        RaiseStep::Adopted,
        RaiseStep::Persisted,
        RaiseStep::CopyDone,
    ] {
        let (dir, mut vault) = setup();
        std::fs::copy(&vault.path, dir.path().join("test.wallet.bak")).unwrap();
        let id = vault.doc.id.clone();
        let stores = vault.doc.stores.len();
        let result = vault.raise_inner(
            "pw",
            STRONG,
            cheap_kdf(),
            |_| {},
            |at| {
                if at == step {
                    Err(Error::Crypto("injected".into()))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "{step:?}");
        drop(vault);

        let path = wallet(&dir);
        let old = UnlockedVault::open(path.clone(), "pw");
        let new = UnlockedVault::open(path.clone(), STRONG);
        assert!(old.is_ok() != new.is_ok(), "{step:?}: opens at one level");
        let (reopened, level) = match (old, new) {
            (Ok(vault), _) => (vault, SecurityLevel::Pin),
            (_, Ok(vault)) => (vault, SecurityLevel::Crypto),
            _ => unreachable!(),
        };
        let expected = if step == RaiseStep::BackedUp {
            SecurityLevel::Pin
        } else {
            SecurityLevel::Crypto
        };
        assert_eq!(level, expected, "{step:?}");
        assert_eq!(reopened.security_level(), expected, "{step:?}");
        assert_eq!(reopened.doc.id, id, "{step:?}");
        assert_eq!(reopened.doc.stores.len(), stores, "{step:?}");
        // Every copy opens with one of the two secrets, none is half-done,
        // and the wizard's backup is there.
        let found = copies_of(dir.path());
        assert!(found.len() >= 2, "{step:?}: {found:?}");
        for copy in found {
            assert!(
                opens(&copy, "pw") != opens(&copy, STRONG),
                "{step:?}: {copy:?}"
            );
        }
    }
}

#[test]
fn after_an_interruption_the_backup_opens_with_the_old_pin() {
    let (dir, mut vault) = empty_vault();
    let result = vault.raise_inner(
        "pw",
        STRONG,
        cheap_kdf(),
        |_| {},
        |at| {
            if at == RaiseStep::Persisted {
                Err(Error::Crypto("injected".into()))
            } else {
                Ok(())
            }
        },
    );
    assert!(result.is_err());
    let found = copies_of(dir.path());
    assert_eq!(found.len(), 1);
    assert!(opens(&found[0], "pw"));
    assert_eq!(level_of_vault_file(&found[0]).unwrap(), SecurityLevel::Pin);
}

#[test]
fn a_wrong_pin_or_a_weak_password_changes_nothing() {
    let (dir, mut vault) = empty_vault();
    let before = std::fs::read(&vault.path).unwrap();
    assert!(matches!(
        vault.raise_to_crypto("nope", STRONG, cheap_kdf(), |_| {}),
        Err(Error::Secret)
    ));
    assert!(matches!(
        vault.raise_to_crypto("pw", "123456", cheap_kdf(), |_| {}),
        Err(Error::PasswordTooWeak)
    ));
    assert!(matches!(
        vault.raise_to_crypto("pw", STRONG, KdfParams::STANDARD, |_| {}),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    assert!(copies_of(dir.path()).is_empty());
    assert_eq!(vault.security_level(), SecurityLevel::Pin);
}

/// A plain (level-1) vault with a store and plain copies of every kind.
fn plain_vault_with_copies() -> (tempfile::TempDir, UnlockedVault) {
    let dir = tempfile::tempdir().unwrap();
    let mut vault = UnlockedVault::create_with(wallet(&dir), None).unwrap();
    vault
        .create_store("Cold", StoreKind::Crypto, "#555", "", None)
        .unwrap();
    vault.flush_pending().unwrap();
    std::fs::copy(&vault.path, dir.path().join("test.wallet.bak")).unwrap();
    std::fs::copy(&vault.path, dir.path().join("test.wallet.v1.bak")).unwrap();
    vault.sync_backup().unwrap();
    assert_eq!(vault.security_level(), SecurityLevel::Ledger);
    (dir, vault)
}

#[test]
fn a_plain_vault_goes_straight_to_level_three_and_keeps_no_plain_copy() {
    let (dir, mut vault) = plain_vault_with_copies();
    let stores = vault.doc.stores.len();
    let mut stages = Vec::new();
    // No current secret: whatever is typed there is ignored.
    let report = vault
        .raise_to_crypto("ignored", STRONG, cheap_kdf(), |stage| stages.push(stage))
        .unwrap();
    assert_eq!(
        stages,
        [
            RaiseStage::Backup,
            RaiseStage::Reencrypt,
            RaiseStage::Verify
        ]
    );
    assert_eq!(vault.security_level(), SecurityLevel::Crypto);
    assert!(opens(&report.backup, STRONG));
    let found = copies_of(dir.path());
    assert!(found.len() >= 4, "{found:?}");
    for copy in found {
        assert_eq!(
            level_of_vault_file(&copy).unwrap(),
            SecurityLevel::Crypto,
            "{copy:?}"
        );
        assert!(!opens(&copy, ""), "{copy:?}");
    }
    drop(vault);
    assert!(UnlockedVault::open(wallet(&dir), "").is_err());
    let reopened = UnlockedVault::open(wallet(&dir), STRONG).unwrap();
    assert_eq!(reopened.security_level(), SecurityLevel::Crypto);
    assert_eq!(reopened.doc.stores.len(), stores);
}

#[test]
fn a_crash_at_any_step_from_plain_leaves_the_vault_opening_with_its_data() {
    for step in [
        RaiseStep::BackedUp,
        RaiseStep::Reencrypted,
        RaiseStep::Adopted,
        RaiseStep::Persisted,
        RaiseStep::CopyDone,
    ] {
        let (dir, mut vault) = plain_vault_with_copies();
        let id = vault.doc.id.clone();
        let stores = vault.doc.stores.len();
        let copies_before = copies_of(dir.path()).len();
        let result = vault.raise_inner(
            "",
            STRONG,
            cheap_kdf(),
            |_| {},
            |at| {
                if at == step {
                    Err(Error::Crypto("injected".into()))
                } else {
                    Ok(())
                }
            },
        );
        assert!(result.is_err(), "{step:?}");
        drop(vault);

        let path = wallet(&dir);
        let expected = if step == RaiseStep::BackedUp {
            SecurityLevel::Ledger
        } else {
            SecurityLevel::Crypto
        };
        assert_eq!(level_of_vault_file(&path).unwrap(), expected, "{step:?}");
        let secret = if expected == SecurityLevel::Ledger {
            ""
        } else {
            STRONG
        };
        let reopened = UnlockedVault::open(path, secret).unwrap();
        assert_eq!(reopened.security_level(), expected, "{step:?}");
        assert_eq!(reopened.doc.id, id, "{step:?}");
        assert_eq!(reopened.doc.stores.len(), stores, "{step:?}");
        // The plain backup of step 1 is there; every copy is whole, plain
        // or level 3, none half-done.
        let found = copies_of(dir.path());
        assert!(found.len() > copies_before, "{step:?}: {found:?}");
        for copy in found {
            let whole = match level_of_vault_file(&copy).unwrap() {
                SecurityLevel::Ledger => opens(&copy, ""),
                SecurityLevel::Crypto => opens(&copy, STRONG) && !opens(&copy, ""),
                SecurityLevel::Pin => false,
            };
            assert!(whole, "{step:?}: {copy:?}");
        }
    }
}

#[test]
fn a_vault_already_at_three_is_not_raised() {
    let (_dir, mut vault) = empty_vault();
    vault.raise_to_level3_for_tests();
    assert!(matches!(
        vault.raise_to_crypto("pw", STRONG, cheap_kdf(), |_| {}),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn a_linked_vault_raises_with_the_phone_profile() {
    let fast = |_: &KdfParams| Ok(Duration::ZERO);
    let (_dir, mut vault) = empty_vault();
    vault.set_mirror(Some(Arc::new(RecordingMirror::default())));
    let linked = vault.raise_profile().calibrate(u32::MAX, fast);
    assert!(linked.mem_kib <= 256 * 1024, "{linked:?}");
    assert_eq!(vault.raise_profile(), KdfProfile::MOBILE);
    vault.set_mirror(None);
    assert_eq!(vault.raise_profile(), KdfProfile::for_this_device());
}

// -- no way down --------------------------------------------------------

fn level3_with_key() -> (tempfile::TempDir, UnlockedVault) {
    let (dir, mut vault) = empty_vault();
    vault.raise_to_level3_for_tests();
    let store = vault
        .create_store_with(
            "Bybit",
            StoreKind::Exchange,
            "#f0b90b",
            "",
            None,
            StoreExtras {
                provider: Some(ExchangeProvider::Bybit),
                ..Default::default()
            },
        )
        .unwrap()
        .id;
    vault
        .store_device_key(&store, "KEYKEYKEY123", "SECRETSECRET456", true)
        .unwrap();
    vault.flush_pending().unwrap();
    (dir, vault)
}

#[test]
fn a_vault_with_crypto_data_does_not_go_below_level_three() {
    let (_dir, vault) = empty_vault();
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Ledger);

    let (_dir, mut vault) = level3_with_key();
    assert_eq!(vault.min_allowed_level(), SecurityLevel::Crypto);
    let before = std::fs::read(&vault.path).unwrap();
    assert!(matches!(
        vault.remove_password("pw"),
        Err(Error::SecurityDowngradeRefused)
    ));
    assert!(matches!(
        vault.change_password("pw", "123456"),
        Err(Error::PasswordTooWeak)
    ));
    assert_eq!(std::fs::read(&vault.path).unwrap(), before);
    vault.change_password("pw", STRONG).unwrap();
    assert_eq!(vault.security_level(), SecurityLevel::Crypto);
}

#[test]
fn a_secrets_file_keeps_a_plain_vault_from_taking_a_pin() {
    let dir = tempfile::tempdir().unwrap();
    let mut plain = UnlockedVault::create_with(dir.path().join("p.wallet"), None).unwrap();
    std::fs::write(device_secrets::secrets_path(&plain.path), b"sealed").unwrap();
    assert_eq!(plain.min_allowed_level(), SecurityLevel::Crypto);
    assert!(matches!(
        plain.set_password("482913", false),
        Err(Error::SecurityDowngradeRefused)
    ));
    assert!(!plain.is_encrypted());
}

#[test]
fn a_cheaper_container_is_told_apart() {
    let (_dir, mut vault) = empty_vault();
    let pin_bytes = std::fs::read(&vault.path).unwrap();
    vault.raise_to_level3_for_tests();
    let crypto_bytes = std::fs::read(&vault.path).unwrap();
    assert!(container_is_cheaper(&crypto_bytes, &pin_bytes));
    assert!(!container_is_cheaper(&pin_bytes, &crypto_bytes));
    assert!(!container_is_cheaper(&crypto_bytes, &crypto_bytes));
    assert!(!container_is_cheaper(&crypto_bytes, b"garbage"));
}

// -- sync below level 3 -------------------------------------------------

#[test]
fn crypto_data_is_not_taken_in_below_level_three() {
    let _guard = crate::sync::crypto_gate::override_for_tests(&["stores"]);
    let (_dir, mut vault) = setup();
    let keys: Vec<String> = vault
        .doc
        .stores
        .iter()
        .map(|store| format!("stores:{}", store.id))
        .collect();
    let incoming = vault.sync_records(&keys).unwrap();
    assert!(!incoming.is_empty());
    assert!(matches!(
        vault.sync_preview(&incoming),
        Err(Error::SecurityLevelRequired)
    ));
    assert!(matches!(
        vault.sync_preview_after(&incoming),
        Err(Error::SecurityLevelRequired)
    ));
    vault.raise_to_level3_for_tests();
    assert!(vault.sync_preview(&incoming).is_ok());
}

#[test]
fn ordinary_records_sync_at_any_level() {
    let (_dir, mut vault) = setup();
    let keys: Vec<String> = vault
        .doc
        .stores
        .iter()
        .map(|store| format!("stores:{}", store.id))
        .collect();
    let incoming = vault.sync_records(&keys).unwrap();
    assert!(vault.sync_check_level(&incoming).is_ok());
}
