//! Tests of the vault file codec that need the full fixture document or `UnlockedVault`: both
//! live in the facade, so these moved here from `mwt-vault` with their assertions unchanged.

use std::path::{Path, PathBuf};

use serde_json::Value as Json;

use crate::crypto::kdf_profile::KdfParams;
use crate::crypto::level::RaiseStage;
use crate::crypto::{self, SealKey};
use crate::error::{Error, Result};
use crate::model::VaultDoc;
use crate::model::fixtures::full_doc;
use crate::service::UnlockedVault;
use crate::storage::{
    ReencryptStep, SnapshotFormat, VaultFile, decode, decode_in_container, encode,
};
use crate::vault_io::local::reencrypt_tmp_path;

/// A temp name left by another process (this one is never swept).
fn foreign_reencrypt_leftover(vault: &Path) -> PathBuf {
    let mine = reencrypt_tmp_path(vault);
    let name = mine.file_name().unwrap().to_str().unwrap();
    let own = format!(".{}.", std::process::id());
    let foreign = name.replacen(&own, &format!(".{}.", std::process::id() ^ 1), 1);
    vault.with_file_name(foreign)
}

fn assert_refused_as_legacy(err: Error, format_text: &str) {
    let Error::LegacyVault(message) = &err else {
        panic!("expected LegacyVault, got {err:?}");
    };
    assert_eq!(err.code(), "UNSUPPORTED");
    assert!(message.contains(format_text), "{message}");
    assert!(message.contains("no longer reads"), "{message}");
    assert!(
        message.contains(crate::crypto::format::LAST_LEGACY_BUILD),
        "{message}"
    );
}

/// The header as builds before the crypto level wrote it, field by field:
/// nothing here follows the current constants. `container` is the container
/// version written into it.
fn old_build_header(container: u32, fmt: SnapshotFormat) -> crypto::header::VaultHeader {
    crypto::header::VaultHeader {
        version: container,
        kdf_id: 0,
        kdf_mem_kib: 64 * 1024,
        kdf_iters: 3,
        kdf_par: 1,
        salt: [0x11; crypto::SALT_LEN],
        nonce: [0x22; crypto::NONCE_LEN],
        payload_fmt: fmt.as_u8(),
    }
}
/// Plan D11: every persisted type of phases 2–7 exists now. A document
/// with every collection filled must survive both encodings unchanged —
/// the round trip catches a type serde can write but not read back.
#[test]
fn a_full_document_round_trips_in_every_format() {
    let doc = full_doc();
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let out = decode(&encode(&doc, fmt).unwrap(), fmt).unwrap();
        assert_eq!(out, doc, "{fmt:?}");
    }
}

/// An old-v1 payload (the counter before the reset) in container 1 is not read as the new
/// v1, which is the whole point of the container version.
#[test]
fn an_old_v1_payload_is_refused() {
    let doc = full_doc();
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let mut json: Json = serde_json::to_value(&doc).unwrap();
        json["version"] = Json::from(1);
        let old = match fmt {
            SnapshotFormat::Pack => rmp_serde::to_vec_named(&json).unwrap(),
            _ => serde_json::to_vec(&json).unwrap(),
        };
        let err = decode_in_container(1, &old, fmt).unwrap_err();
        assert_refused_as_legacy(err, "format 1");
        // The same bytes in container 2 are the new v1.
        assert_eq!(decode_in_container(2, &old, fmt).unwrap().from_version, 1);
    }
}

/// v2 carries field names: a field missing from an older v2 file takes its
/// default instead of breaking the read.
#[test]
fn a_v2_pack_payload_tolerates_a_missing_defaulted_field() {
    let doc = full_doc();
    let mut json: Json = serde_json::to_value(&doc).unwrap();
    json.as_object_mut().unwrap().remove("notification_log");
    let bytes = rmp_serde::to_vec_named(&json).unwrap();
    let out = decode(&bytes, SnapshotFormat::Pack).unwrap();
    assert!(out.notification_log.is_empty());
    assert_eq!(out.contacts, doc.contacts);
}

#[test]
fn a_full_document_from_an_old_build_opens_in_every_format() {
    let dir = tempfile::tempdir().unwrap();
    let doc = full_doc();
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let path = dir.path().join(format!("{fmt:?}.wallet"));
        let header = old_build_header(2, fmt);
        let file = crypto::seal(&header, b"pw", &encode(&doc, fmt).unwrap()).unwrap();
        std::fs::write(&path, file).unwrap();

        let (decoded, read, key) = VaultFile::open_keyed(&path, "pw").unwrap();
        assert_eq!((decoded.doc, read), (doc.clone(), fmt));
        assert_eq!(key.kdf(), KdfParams::STANDARD);
        assert!(matches!(
            VaultFile::open(&path, "wrong"),
            Err(Error::Secret)
        ));
    }
}

#[test]
fn a_plain_file_keeps_its_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.wallet");
    let doc = full_doc();
    let fmt = SnapshotFormat::Pack;
    std::fs::write(&path, crypto::seal_plain(fmt, &encode(&doc, fmt).unwrap())).unwrap();
    let (decoded, _, key) = VaultFile::open_keyed(&path, "").unwrap();
    assert_eq!(decoded.doc, doc);
    assert!(key.is_plain());
}

// -- re-encryption ------------------------------------------------------

/// The cheapest cost the limits allow: these tests run the KDF a lot.
const TEST_KDF: KdfParams = KdfParams::hardened(64 * 1024, 1, 1);

fn old_vault(dir: &Path) -> (PathBuf, VaultDoc, Vec<u8>) {
    let path = dir.join("v.wallet");
    let doc = full_doc();
    let key = SealKey::fresh(b"pw").unwrap();
    let fmt = SnapshotFormat::Pack;
    VaultFile::seal_and_write(&path, &key, &encode(&doc, fmt).unwrap(), fmt).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    (path, doc, bytes)
}

fn entries(dir: &Path) -> usize {
    std::fs::read_dir(dir).unwrap().count()
}

fn assert_old_vault_intact(path: &Path, doc: &VaultDoc, bytes: &[u8]) {
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(&VaultFile::open(path, "pw").unwrap().0, doc);
    assert_eq!(entries(path.parent().unwrap()), 1, "no temp file left");
}

fn faulted(
    path: &Path,
    doc: &VaultDoc,
    fault: impl FnMut(ReencryptStep, &Path) -> Result<()>,
) -> Result<SealKey> {
    VaultFile::reencrypt_with_faults(
        path,
        doc,
        SnapshotFormat::Pack,
        "pw",
        TEST_KDF,
        |_| {},
        fault,
    )
}

#[test]
fn reencrypting_moves_the_vault_to_the_new_cost_with_its_data() {
    for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, _) = old_vault(dir.path());
        let mut stages = Vec::new();
        let key = VaultFile::reencrypt_verified(&path, &doc, fmt, "pw", TEST_KDF, |stage| {
            stages.push(stage);
        })
        .unwrap();

        assert_eq!(stages, [RaiseStage::Reencrypt, RaiseStage::Verify]);
        assert_eq!(key.kdf(), TEST_KDF);
        let (decoded, read, reopened) = VaultFile::open_keyed(&path, "pw").unwrap();
        assert_eq!((decoded.doc, read), (doc, fmt));
        assert_eq!(reopened.kdf(), TEST_KDF);
        assert_eq!(entries(dir.path()), 1, "no temp file left");
        let level = crypto::header::level_of_container(&std::fs::read(&path).unwrap());
        assert_eq!(level.unwrap(), mwt_crypto::level::SecurityLevel::Crypto);
    }
}

#[test]
fn a_failure_before_the_rename_leaves_the_old_file() {
    for step in [ReencryptStep::Written, ReencryptStep::Verified] {
        let dir = tempfile::tempdir().unwrap();
        let (path, doc, bytes) = old_vault(dir.path());
        let result = faulted(&path, &doc, |at, _| {
            if at == step {
                Err(Error::Io("the power went out".into()))
            } else {
                Ok(())
            }
        });
        assert!(matches!(result, Err(Error::Io(_))), "{step:?}");
        assert_old_vault_intact(&path, &doc, &bytes);
    }
}

#[test]
fn a_failed_rename_leaves_the_old_file() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Verified {
            std::fs::remove_file(tmp).unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[test]
fn a_new_file_that_does_not_open_never_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Written {
            let mut file = std::fs::read(tmp).unwrap();
            let last = file.len() - 1;
            file[last] ^= 1;
            std::fs::write(tmp, file).unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(Error::Secret)));
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[test]
fn a_new_file_with_another_document_never_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Written {
            let key = SealKey::fresh_with(b"pw", TEST_KDF).unwrap();
            let fmt = SnapshotFormat::Pack;
            let empty = encode(&VaultDoc::default(), fmt).unwrap();
            std::fs::write(tmp, crypto::seal_with(&key, fmt, &empty).unwrap()).unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(Error::Crypto(_))));
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[test]
fn a_new_file_at_a_lower_cost_never_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Written {
            let key = SealKey::fresh(b"pw").unwrap();
            let fmt = SnapshotFormat::Pack;
            let same = encode(&doc, fmt).unwrap();
            std::fs::write(tmp, crypto::seal_with(&key, fmt, &same).unwrap()).unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(Error::Crypto(_))));
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[test]
fn a_temp_file_left_by_a_crash_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let leftover = foreign_reencrypt_leftover(&path);
    std::fs::write(&leftover, b"half a file").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    VaultFile::reencrypt_verified(&path, &doc, SnapshotFormat::Pack, "pw", TEST_KDF, |_| {})
        .unwrap();
    assert_eq!(std::fs::read(&leftover).unwrap(), b"half a file");
    assert_eq!(VaultFile::open(&path, "pw").unwrap().0, doc);
    assert_eq!(entries(dir.path()), 2, "the vault and the old leftover");
}

#[test]
fn opening_the_vault_sweeps_what_a_crashed_reencryption_left() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _, _) = old_vault(dir.path());
    let leftover = foreign_reencrypt_leftover(&path);
    std::fs::write(&leftover, b"half a file").unwrap();
    let wrong = UnlockedVault::open(path.clone(), "wrong");
    assert!(matches!(wrong, Err(Error::Secret)));
    assert!(leftover.exists(), "a wrong password touches nothing");
    let vault = UnlockedVault::open(path.clone(), "pw").unwrap();
    assert!(!leftover.exists());
    drop(vault);
}

#[test]
fn a_temp_file_changed_after_verification_never_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    // Same document and cost, so only the swap check can catch it.
    let other = SealKey::fresh_with(b"pw", TEST_KDF).unwrap();
    let fmt = SnapshotFormat::Pack;
    let impostor = crypto::seal_with(&other, fmt, &encode(&doc, fmt).unwrap()).unwrap();
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Verified {
            std::fs::write(tmp, &impostor).unwrap();
        }
        Ok(())
    });
    assert!(matches!(result, Err(Error::Io(_))), "{:?}", result.err());
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[cfg(unix)]
#[test]
fn a_temp_file_swapped_for_a_symlink_never_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let elsewhere = tempfile::tempdir().unwrap();
    let target = elsewhere.path().join("planted");
    std::fs::write(&target, b"planted").unwrap();
    let result = faulted(&path, &doc, |at, tmp| {
        if at == ReencryptStep::Verified {
            std::fs::remove_file(tmp).unwrap();
            std::os::unix::fs::symlink(&target, tmp).unwrap();
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_old_vault_intact(&path, &doc, &bytes);
    assert_eq!(std::fs::read(&target).unwrap(), b"planted");
}

#[test]
fn reencrypting_with_an_empty_password_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, bytes) = old_vault(dir.path());
    let mut stages = Vec::new();
    let result =
        VaultFile::reencrypt_verified(&path, &doc, SnapshotFormat::Pack, "", TEST_KDF, |stage| {
            stages.push(stage);
        });
    assert!(matches!(result, Err(Error::InvalidInput(_))));
    assert!(stages.is_empty());
    assert_old_vault_intact(&path, &doc, &bytes);
}

#[test]
fn a_vault_stays_at_its_cost_through_saves_and_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let (path, doc, _) = old_vault(dir.path());
    VaultFile::reencrypt_verified(&path, &doc, SnapshotFormat::Pack, "pw", TEST_KDF, |_| {})
        .unwrap();

    let mut vault = UnlockedVault::open(path.clone(), "pw").unwrap();
    vault.switch_format(SnapshotFormat::Json).unwrap();
    drop(vault);

    let (decoded, read, key) = VaultFile::open_keyed(&path, "pw").unwrap();
    assert_eq!(read, SnapshotFormat::Json);
    assert_eq!(key.kdf(), TEST_KDF);
    // Opening repairs this fixture's tree and adds the cash wallet, so the
    // stores are not equal to the input; none of them goes.
    let kept: Vec<_> = decoded.doc.stores.iter().map(|store| &store.id).collect();
    assert!(doc.stores.iter().all(|store| kept.contains(&&store.id)));
}

/// The v4 fixture converted by `mwt-vault` carries graphs of the live workflow version (the
/// graph steps and the rest of the conversion are tested in `mwt-vault`).
#[test]
fn the_converted_fixture_carries_graphs_of_the_live_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v4.json.wallet");
    std::fs::write(
        &path,
        include_bytes!("../../../vault/src/storage/fixtures/v4.json.wallet"),
    )
    .unwrap();
    let (decoded, _) = VaultFile::open_versioned(&path, "v4-fixture-password").unwrap();
    let live = decoded.doc;
    assert!(!live.workflows.is_empty());
    for workflow in &live.workflows {
        let graph: serde_json::Value = serde_json::from_str(&workflow.graph).unwrap();
        assert_eq!(graph["version"], crate::workflow::graph::GRAPH_VERSION);
    }
}
