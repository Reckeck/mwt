//! The error a command returns. On the wire it stays the string
//! `"CODE: message"`, the code, a colon, the message.

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

#[cfg(test)]
mod tests {

    use super::*;

    fn wire(error: &IpcError) -> String {
        serde_json::to_value(error)
            .expect("serializes")
            .as_str()
            .expect("a string")
            .to_owned()
    }

    /// One sample per `Error` family with its wire string.
    fn samples() -> Vec<(Error, &'static str)> {
        vec![
            (
                Error::NotFound("wallet".into()),
                "NOT_FOUND: vault file not found: wallet",
            ),
            (
                Error::Sandbox("blocked".into()),
                "SANDBOX: sandbox: blocked",
            ),
            (Error::RequiresUnlock, "REQUIRES_UNLOCK: vault is locked"),
        ]
    }

    #[test]
    fn ipc_error_serializes_as_code_and_message() {
        for (sample, expected) in samples() {
            assert_eq!(wire(&IpcError::from(sample)), expected);
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
}
