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

    #[error("pricing: {0}")]
    Pricing(String),

    #[error("not found: {0}")]
    Missing(String),

    #[error("store is archived: {0}")]
    Archived(String),

    #[error("vault is locked")]
    RequiresUnlock,
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
            Error::Pricing(_) => "PRICING",
            Error::Missing(_) => "MISSING",
            Error::Archived(_) => "ARCHIVED",
            Error::RequiresUnlock => "REQUIRES_UNLOCK",
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

impl From<argon2::password_hash::Error> for Error {
    fn from(e: argon2::password_hash::Error) -> Self {
        Error::Crypto(e.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serialize(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
