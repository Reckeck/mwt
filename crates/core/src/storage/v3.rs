//! Payload v3, frozen: the document as every vault written before the crypto
//! module holds it.
//!
//! v4 adds the crypto module's own top-level collections (`crypto_accounts`,
//! `crypto_events`, …). They are additive and defaulted, so a v3 file reads
//! with the current types and nothing has to be converted: the step exists so
//! that the version on disk says whether a file may hold crypto data at all.
//! An older build then refuses a newer file at the handshake ("the versions
//! differ, update") instead of meeting an unknown sync collection halfway
//! through an exchange (decision P2 of 2026-11-10).
//!
//! The schema is therefore the live document itself, as `v2` already does for
//! everything outside the automations it converts. When a later version
//! changes a field of the document in a way a v3 file cannot be read with,
//! freeze the v3 shape here first.
//!
//! Never edit this definition: it describes files already on users' disks.

use serde::Deserialize;

use super::v2::VaultDocV2;
use crate::model::vault::VaultDoc;

/// The v3 document: today's one, whose crypto collections a v3 file simply
/// does not carry.
#[derive(Deserialize)]
#[serde(transparent)]
#[cfg_attr(test, derive(serde::Serialize))]
pub struct VaultDocV3(pub VaultDoc);

/// v3 → v4: nothing to convert. The crypto collections of a v3 file are
/// absent, so they read as empty, and the document is rewritten at v4.
impl From<VaultDocV3> for VaultDoc {
    fn from(old: VaultDocV3) -> Self {
        let mut doc = old.0;
        doc.version = 4;
        doc
    }
}

/// The last step of a v2 migration now lands on v3 instead of the live
/// document: `v2` hands over a document at version 3, and the step above
/// takes it from there.
impl From<VaultDocV2> for VaultDocV3 {
    fn from(old: VaultDocV2) -> Self {
        Self(VaultDoc::from(old))
    }
}

/// Where v3 sits in the chain (`storage::chain`): one step up is the live
/// document, so this is the last step of a migration.
impl super::chain::Legacy for VaultDocV3 {
    type Next = VaultDoc;
    const VERSION: u32 = 3;
}
