use thiserror::Error;

/// Errors surfaced across core (and mapped into Tauri errors by the app).
#[derive(Debug, Error)]
pub enum Error {
    #[error("wrong password or corrupted vault file")]
    Secret,

    #[error("vault file already exists: {0}")]
    AlreadyExists(String),

    #[error("vault file not found: {0}")]
    NotFound(String),

    #[error("unsupported vault file/version")]
    Unsupported,

    #[error("serialization error: {0}")]
    Serialize(String),

    #[error("deserialization error: {0}")]
    Deserialize(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("io error: {0}")]
    Io(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("tx hash malformed: {0}")]
    TxHashMalformed(String),

    #[error("tx hash network mismatch: {0}")]
    TxHashNetworkMismatch(String),

    #[error("pricing: {0}")]
    Pricing(String),

    #[error("not found: {0}")]
    Missing(String),

    #[error("store is archived: {0}")]
    Archived(String),

    #[error("vault is locked")]
    RequiresUnlock,

    #[error("{0}")]
    StoreTree(StoreTreeViolation),
}

/// A wallet-tree rule a write would break. Each one has its own IPC code so
/// the UI can say *which* rule, in the user's language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StoreTreeViolation {
    #[error("a store cannot be nested inside itself or its own sub-accounts")]
    Cycle,
    #[error("the tree is limited to {} levels", crate::model::store::MAX_STORE_DEPTH + 1)]
    TooDeep,
    #[error("a sub-account cannot be added to an archived store")]
    ParentArchived,
    #[error("this name is already taken at this level")]
    NameTaken,
    #[error("a top-level store cannot become a sub-account")]
    RootMove,
    #[error("restore the parent store first")]
    RestoreParentFirst,
}

impl StoreTreeViolation {
    pub fn code(&self) -> &'static str {
        match self {
            StoreTreeViolation::Cycle => "STORE_CYCLE",
            StoreTreeViolation::TooDeep => "STORE_TOO_DEEP",
            StoreTreeViolation::ParentArchived => "STORE_PARENT_ARCHIVED",
            StoreTreeViolation::NameTaken => "STORE_NAME_TAKEN",
            StoreTreeViolation::RootMove => "STORE_ROOT_MOVE",
            StoreTreeViolation::RestoreParentFirst => "STORE_RESTORE_PARENT_FIRST",
        }
    }
}

impl Error {
    /// Stable machine-friendly code for IPC (`err.code` on the JS side).
    pub fn code(&self) -> &'static str {
        match self {
            Error::Secret => "SECRET",
            Error::AlreadyExists(_) => "ALREADY_EXISTS",
            Error::NotFound(_) => "NOT_FOUND",
            Error::Unsupported => "UNSUPPORTED",
            Error::Serialize(_) => "SERIALIZE",
            Error::Deserialize(_) => "DESERIALIZE",
            Error::Crypto(_) => "CRYPTO",
            Error::Io(_) => "IO",
            Error::Network(_) => "NETWORK",
            Error::InvalidInput(_) => "INVALID_INPUT",
            Error::TxHashMalformed(_) => "TX_HASH_MALFORMED",
            Error::TxHashNetworkMismatch(_) => "TX_HASH_NETWORK_MISMATCH",
            Error::Pricing(_) => "PRICING",
            Error::Missing(_) => "MISSING",
            Error::Archived(_) => "ARCHIVED",
            Error::RequiresUnlock => "REQUIRES_UNLOCK",
            Error::StoreTree(violation) => violation.code(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(source: std::io::Error) -> Self {
        Error::Io(source.to_string())
    }
}

impl From<argon2::password_hash::Error> for Error {
    fn from(source: argon2::password_hash::Error) -> Self {
        Error::Crypto(source.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(source: serde_json::Error) -> Self {
        Error::Serialize(source.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
