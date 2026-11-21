//! Tests of the vault files that need `UnlockedVault`: it lives in the facade, so this one moved
//! here from `mwt-vault` with its assertions unchanged.

use crate::model::VaultDoc;
use crate::service::UnlockedVault;
use crate::storage::{SnapshotFormat, encode};

fn sealed(password: &str) -> Vec<u8> {
    let plaintext = encode(&VaultDoc::default(), SnapshotFormat::Json).unwrap();
    let (header, _, _) = crate::crypto::random_header(SnapshotFormat::Json);
    crate::crypto::seal(&header, password.as_bytes(), &plaintext).unwrap()
}

#[test]
fn probe_external_uses_the_open_vaults_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.wallet");
    let vault = UnlockedVault::create(path.clone(), "a").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(vault.probe_external(&bytes).is_ok());
    assert!(vault.probe_external(&bytes[..bytes.len() - 8]).is_err());
    assert!(vault.probe_external(&sealed("b")).is_err());
}
