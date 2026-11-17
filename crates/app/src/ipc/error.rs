//! The error a command returns. On the wire it stays the string
//! `"CODE: message"`, exactly what `err_to_string` makes.

use std::fmt;

use serde::{Serialize, Serializer};
use trade_core::Error;

/// Code reported for an error that did not come from core.
const UNKNOWN_CODE: &str = "UNKNOWN";

#[derive(Debug)]
pub enum IpcError {
    /// A core error: serialized as `"CODE: message"`.
    Core(Error),
    /// A hand-made message: serialized as the plain text.
    Message(String),
}

impl IpcError {
    /// Stable machine-friendly code; `"UNKNOWN"` for a plain message.
    pub fn code(&self) -> &str {
        match self {
            Self::Core(error) => error.code(),
            Self::Message(_) => UNKNOWN_CODE,
        }
    }
}

impl fmt::Display for IpcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => write!(formatter, "{}: {error}", error.code()),
            Self::Message(text) => formatter.write_str(text),
        }
    }
}

impl Serialize for IpcError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl From<Error> for IpcError {
    fn from(error: Error) -> Self {
        Self::Core(error)
    }
}

impl From<String> for IpcError {
    fn from(text: String) -> Self {
        Self::Message(text)
    }
}

impl From<&str> for IpcError {
    fn from(text: &str) -> Self {
        Self::Message(text.to_owned())
    }
}

/// Temporary shim (removed in T24): a converted helper still `?`s into an
/// unconverted `Result<_, String>` command.
impl From<IpcError> for String {
    fn from(error: IpcError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use trade_core::error::{PeerSyncFailure, StoreTreeViolation, WalletRule};

    use super::*;
    use crate::commands::err_to_string;

    fn wire(error: &IpcError) -> String {
        serde_json::to_value(error)
            .expect("serializes")
            .as_str()
            .expect("a string")
            .to_owned()
    }

    /// One sample per `Error` family; built twice, as `Error` is not `Clone`.
    fn samples() -> Vec<Error> {
        vec![
            Error::NotFound("wallet".into()),
            Error::Sandbox("blocked".into()),
            Error::RequiresUnlock,
            Error::WalletRule(WalletRule::CashSingleton),
            Error::StoreTree(StoreTreeViolation::Cycle),
            Error::PeerSync(PeerSyncFailure::Channel),
        ]
    }

    #[test]
    fn ipc_error_serializes_like_err_to_string() {
        for (sample, legacy) in samples().into_iter().zip(samples()) {
            assert_eq!(wire(&IpcError::from(sample)), err_to_string(legacy));
        }
    }

    #[test]
    fn a_message_error_serializes_as_the_plain_text() {
        let error = IpcError::from("internal error");
        assert_eq!(wire(&error), "internal error");
        assert_eq!(error.code(), "UNKNOWN");
    }

    #[test]
    fn a_core_error_converts_with_question_mark() {
        fn core_call() -> Result<(), Error> {
            Err(Error::RequiresUnlock)
        }
        fn command() -> Result<(), IpcError> {
            core_call()?;
            Ok(())
        }
        let error = command().expect_err("fails");
        assert_eq!(error.code(), "REQUIRES_UNLOCK");
    }

    #[test]
    fn the_string_shim_round_trips() {
        fn unconverted() -> Result<(), String> {
            Err(IpcError::from(Error::RequiresUnlock))?;
            Ok(())
        }
        assert_eq!(
            unconverted().expect_err("fails"),
            err_to_string(Error::RequiresUnlock)
        );
    }
}
