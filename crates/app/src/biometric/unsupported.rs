//! No hardware-bound biometric storage that was verified: Linux. Everything is `Unavailable`.

use zeroize::Zeroizing;

use super::{BioError, BiometricVault};

#[derive(Default)]
pub struct Platform;

impl BiometricVault for Platform {
    fn available(&self) -> bool {
        false
    }

    fn wrap(&self, _vault_id: &str, _secret: &[u8]) -> Result<Vec<u8>, BioError> {
        Err(BioError::Unavailable)
    }

    fn unwrap(&self, _vault_id: &str, _blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError> {
        Err(BioError::Unavailable)
    }

    fn invalidate(&self, _vault_id: &str) -> Result<(), BioError> {
        Ok(())
    }
}
