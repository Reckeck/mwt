//! Tests of the vault files that need `Session`: it lives in the facade, so this one moved
//! here from `mwt-vault` with its assertions unchanged.

use crate::model::VaultDoc;
use crate::service::Session;
use crate::storage::{SnapshotFormat, encode};
use crate::usecases::sync::peer_sync;

fn sealed(password: &str) -> Vec<u8> {
    let plaintext = encode(&VaultDoc::default(), SnapshotFormat::Json).unwrap();
    let (header, _, _) = crate::crypto::random_header(SnapshotFormat::Json);
    crate::crypto::seal(&header, password.as_bytes(), &plaintext).unwrap()
}

#[test]
fn probe_external_uses_the_open_vaults_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.wallet");
    let vault = Session::create(path.clone(), "a").unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert!(peer_sync::probe_external(&vault, &bytes).is_ok());
    assert!(peer_sync::probe_external(&vault, &bytes[..bytes.len() - 8]).is_err());
    assert!(peer_sync::probe_external(&vault, &sealed("b")).is_err());
}
