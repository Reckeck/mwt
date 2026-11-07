//! macOS: a Keychain generic password with the access control
//! `BiometryCurrentSet` in the data-protection keychain. Touch ID is demanded by
//! the Secure Enclave on read, and the item stops opening when the set of enrolled
//! fingerprints changes.
//!
//! The data-protection keychain needs the `keychain-access-groups` entitlement,
//! which an ad-hoc signed build does not have: the add fails (`-34018`) and the
//! platform is `Unavailable`. An item is never stored without the access control.
//! Not compiled on the Linux dev machine; unverified on a real Mac.

use std::sync::OnceLock;

use security_framework::access_control::{ProtectionMode, SecAccessControl};
use security_framework::passwords::{
    AccessControlOptions, PasswordOptions, delete_generic_password_options, generic_password,
    set_generic_password_options,
};
use zeroize::Zeroizing;

use super::{BioError, BiometricVault};

const SERVICE: &str = "mwt.biometric";
const PROBE_ACCOUNT: &str = "availability-probe";
const ERR_USER_CANCELED: i32 = -128;
const ERR_ITEM_NOT_FOUND: i32 = -25300;
const ERR_AUTH_FAILED: i32 = -25293;

#[derive(Default)]
pub struct Platform;

fn options(account: &str) -> PasswordOptions {
    let mut options = PasswordOptions::new_generic_password(SERVICE, account);
    options.use_protected_keychain();
    options
}

fn bind(account: &str, secret: &[u8]) -> Result<(), BioError> {
    let control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenPasscodeSetThisDeviceOnly),
        AccessControlOptions::BIOMETRY_CURRENT_SET.bits(),
    )
    .map_err(|_| BioError::Unavailable)?;
    let mut item = options(account);
    item.set_access_control(control);
    // A failed add (no entitlement, no enrolled biometrics) leaves nothing behind.
    set_generic_password_options(secret, item).map_err(|_| BioError::Unavailable)
}

fn remove(account: &str) {
    delete_generic_password_options(options(account)).ok();
}

impl BiometricVault for Platform {
    /// Adding a protected item asks for no gesture, so a probe item tells whether
    /// the binding can exist at all; the answer is kept for the run.
    fn available(&self) -> bool {
        static AVAILABLE: OnceLock<bool> = OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            remove(PROBE_ACCOUNT);
            let bound = bind(PROBE_ACCOUNT, b"probe").is_ok();
            remove(PROBE_ACCOUNT);
            bound
        })
    }

    fn wrap(&self, vault_id: &str, secret: &[u8]) -> Result<Vec<u8>, BioError> {
        remove(vault_id);
        bind(vault_id, secret)?;
        Ok(Vec::new())
    }

    fn unwrap(&self, vault_id: &str, _blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError> {
        match generic_password(options(vault_id)) {
            Ok(secret) => Ok(Zeroizing::new(secret)),
            Err(err) => Err(match err.code() {
                ERR_USER_CANCELED => BioError::Cancelled,
                // The enrolled set changed (or the item is gone): the binding is dead.
                ERR_AUTH_FAILED | ERR_ITEM_NOT_FOUND => BioError::Invalidated,
                _ => BioError::Failed,
            }),
        }
    }

    fn invalidate(&self, vault_id: &str) -> Result<(), BioError> {
        remove(vault_id);
        Ok(())
    }
}
