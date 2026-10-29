//! Files and links kept with an operation or a goal (Full-New-Attach 1–7).
//!
//! A link lives in the record itself. A file's bytes are sealed next to the
//! vault, in `<vault>.files/<id>`, each with a key of its own that is kept in
//! the vault ([`AttachmentKey`]): a new password does not touch the files, and
//! sync carries the keys with the records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::model::entity_ref::EntityRef;

/// The biggest file one attachment may hold.
pub const MAX_ATTACHMENT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    File,
    Link,
}

/// One file or link of an entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS, JsonSchema)]
pub struct Attachment {
    pub id: String,
    /// The operation or goal it belongs to.
    pub owner: EntityRef,
    pub kind: AttachmentKind,
    /// The file name, or the link's caption (the address when none was given).
    pub title: String,
    /// The link's address; `None` for a file.
    #[serde(default)]
    pub url: Option<String>,
    /// A file's media type, as the system named it.
    #[serde(default)]
    pub mime: Option<String>,
    /// A file's size in bytes, before sealing.
    #[serde(default)]
    #[ts(type = "number")]
    pub size: u64,
    /// SHA-256 of the file, hex: the same check is added once.
    #[serde(default)]
    pub sha256: Option<String>,
    pub created_at: String,
    /// Removed, waiting for "Undo": hidden from every list, its file kept
    /// until the vault is opened next.
    #[serde(default)]
    pub removed_at: Option<String>,
}

/// The key a file is sealed with, base64. Never leaves the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AttachmentKey {
    pub id: String,
    pub key: String,
}
