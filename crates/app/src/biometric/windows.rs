//! Windows: Windows Hello through `KeyCredentialManager`. The private key lives in
//! the Hello container (the TPM when there is one) and signs only after the Hello
//! gesture (face, fingerprint or the Windows PIN), which Windows itself enforces.
//! RSA PKCS#1 v1.5 signs the same challenge to the same bytes every time, so the
//! signature is a secret only a successful gesture can reproduce: the wrap key is
//! `HKDF-SHA256(signature, random salt)`, and XChaCha20-Poly1305 seals the vault
//! password with it. The record holds `salt || nonce || ciphertext`.
//!
//! Limits, by the documentation: adding a fingerprint does not touch the key; only
//! a Hello or PIN reset removes it (`NotFound`). Offline there is no proof that
//! the key sits in the TPM (attestation needs a network), so the platform is
//! available only with Hello set up (`IsSupportedAsync`) and a hardware TPM seen by
//! the TPM Base Services; without a TPM Hello keeps the key in software, and that
//! is refused. Not compiled by `cargo test` on Linux and unverified on real Windows.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::RngCore;
use sha2::{Digest, Sha256};
use windows::Security::Credentials::{
    KeyCredential, KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::TpmBaseServices::{
    TPM_DEVICE_INFO, TPM_IFTYPE_EMULATOR, Tbsi_GetDeviceInfo,
};
use windows::core::{Array, HSTRING};
use zeroize::Zeroizing;

use super::{BioError, BiometricVault};

const CHALLENGE: &[u8] = b"mwt-biometric-challenge-v1";
const HKDF_INFO: &[u8] = b"mwt-biometric-wrap-v1";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 24;

#[derive(Default)]
pub struct Platform;

/// The credential name: a hash of the vault id, so a path never becomes part of a name.
fn credential_name(vault_id: &str) -> HSTRING {
    let digest = Sha256::digest(vault_id.as_bytes());
    let hex: String = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    HSTRING::from(format!("mwt-{hex}"))
}

/// A hardware TPM that is not an emulator, from the TPM Base Services (offline).
fn tpm_present() -> bool {
    let mut info = TPM_DEVICE_INFO::default();
    let size = std::mem::size_of::<TPM_DEVICE_INFO>() as u32;
    // SAFETY: `info` is a live `TPM_DEVICE_INFO` of the size passed; 0 is `TBS_SUCCESS`.
    let status = unsafe { Tbsi_GetDeviceInfo(size, (&mut info as *mut TPM_DEVICE_INFO).cast()) };
    status == 0 && info.tpmInterfaceType != TPM_IFTYPE_EMULATOR
}

/// Windows reports "no key" and "user said no" as statuses, not errors.
fn map_status(status: KeyCredentialStatus) -> BioError {
    if status == KeyCredentialStatus::NotFound {
        BioError::Invalidated
    } else if status == KeyCredentialStatus::UserCanceled
        || status == KeyCredentialStatus::UserPrefersPassword
    {
        BioError::Cancelled
    } else {
        BioError::Failed
    }
}

/// One Hello prompt: the signature over the fixed challenge.
fn sign(credential: &KeyCredential) -> Result<Zeroizing<Vec<u8>>, BioError> {
    let challenge =
        CryptographicBuffer::CreateFromByteArray(CHALLENGE).map_err(|_| BioError::Failed)?;
    let signed = credential
        .RequestSignAsync(&challenge)
        .and_then(|operation| operation.join())
        .map_err(|_| BioError::Failed)?;
    let status = signed.Status().map_err(|_| BioError::Failed)?;
    if status != KeyCredentialStatus::Success {
        return Err(map_status(status));
    }
    let buffer = signed.Result().map_err(|_| BioError::Failed)?;
    let mut bytes = Array::<u8>::new();
    CryptographicBuffer::CopyToByteArray(&buffer, &mut bytes).map_err(|_| BioError::Failed)?;
    Ok(Zeroizing::new(bytes.to_vec()))
}

fn wrap_key(signature: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, BioError> {
    let mut key = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(Some(salt), signature)
        .expand(HKDF_INFO, key.as_mut())
        .map_err(|_| BioError::Failed)?;
    Ok(key)
}

impl BiometricVault for Platform {
    fn available(&self) -> bool {
        KeyCredentialManager::IsSupportedAsync()
            .and_then(|operation| operation.join())
            .unwrap_or(false)
            && tpm_present()
    }

    fn wrap(&self, vault_id: &str, secret: &[u8]) -> Result<Vec<u8>, BioError> {
        let name = credential_name(vault_id);
        let created = KeyCredentialManager::RequestCreateAsync(
            &name,
            KeyCredentialCreationOption::ReplaceExisting,
        )
        .and_then(|operation| operation.join())
        .map_err(|_| BioError::Unavailable)?;
        let status = created.Status().map_err(|_| BioError::Unavailable)?;
        if status != KeyCredentialStatus::Success {
            return Err(match map_status(status) {
                BioError::Failed => BioError::Unavailable,
                other => other,
            });
        }
        let credential = created.Credential().map_err(|_| BioError::Unavailable)?;
        let sealed = (|| {
            let first = sign(&credential)?;
            // The wrap key is only as good as the signature: a provider that does not
            // repeat itself cannot be used.
            if *first != *sign(&credential)? {
                return Err(BioError::Unavailable);
            }
            let mut salt = [0u8; SALT_LEN];
            let mut nonce = [0u8; NONCE_LEN];
            rand::thread_rng().fill_bytes(&mut salt);
            rand::thread_rng().fill_bytes(&mut nonce);
            let key = wrap_key(&first, &salt)?;
            let cipher = XChaCha20Poly1305::new(key.as_ref().into());
            let ciphertext = cipher
                .encrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: secret,
                        aad: vault_id.as_bytes(),
                    },
                )
                .map_err(|_| BioError::Failed)?;
            Ok([salt.as_slice(), nonce.as_slice(), ciphertext.as_slice()].concat())
        })();
        if sealed.is_err() {
            self.invalidate(vault_id).ok();
        }
        sealed
    }

    fn unwrap(&self, vault_id: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, BioError> {
        if blob.len() <= SALT_LEN + NONCE_LEN {
            return Err(BioError::Invalidated);
        }
        let (salt, rest) = blob.split_at(SALT_LEN);
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let opened = KeyCredentialManager::OpenAsync(&credential_name(vault_id))
            .and_then(|operation| operation.join())
            .map_err(|_| BioError::Failed)?;
        let status = opened.Status().map_err(|_| BioError::Failed)?;
        if status != KeyCredentialStatus::Success {
            return Err(map_status(status));
        }
        let credential = opened.Credential().map_err(|_| BioError::Failed)?;
        let key = wrap_key(&sign(&credential)?, salt)?;
        let cipher = XChaCha20Poly1305::new(key.as_ref().into());
        // A signature that no longer opens the record means the key was recreated.
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad: vault_id.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| BioError::Invalidated)
    }

    fn invalidate(&self, vault_id: &str) -> Result<(), BioError> {
        // A missing credential is already what is wanted.
        KeyCredentialManager::DeleteAsync(&credential_name(vault_id))
            .and_then(|operation| operation.join())
            .ok();
        Ok(())
    }
}
