//! `Secret<T>`: a value that never prints (rule `N5`). A minimal stub; it moves
//! to `mwt-base` in phase 6.
//!
//! `Debug` shows `Secret(..)` and nothing of the value, the value is wiped from
//! memory on drop, and the only way to read it is the explicit [`Secret::expose`].
//! It (de)serializes exactly as the inner value, so wrapping a field does not
//! change a stored file or a schema.

//!
//! # Audit of key-, token- and secret-holding types (N5)
//!
//! Method: `grep -rnEi "(password|secret|api_key|token)\b *: *(Option<)?String" crates`,
//! then each hit's `derive` line. "Debug prints it" is yes when `Debug` is derived over a
//! plain `String` field.
//!
//! | type | Debug prints the text | state |
//! |---|---|---|
//! | `cryptotrack::rpc::client` api key (`ApiKey`) | no | `Secret<String>`; test `the_api_key_never_prints` |
//! | `model::vault::ExchangeKeys` (key, secret) | no | hand `Debug` prints `Secret(..)` for the key and `<redacted>` for the secret (the frozen test `storage::tests::exchange_keys_debug_redacts_the_secret` asserts `<redacted>`); fields stay `String` (T148) |
//! | `storage::legacy_v4::ExchangeKeysV4` | no | hand `Debug` prints `<redacted>`; parse-only copy of the v4 file, outside T50 files |
//! | `ipc_contract::SetExchangeKeyInput` (`api_key`, `api_secret`) | no | fixed: hand `Debug` prints `Secret(..)`; test `set_exchange_key_input_debug_hides_key_and_secret` |
//! | `ipc_contract::{CreateVaultInput, UnlockInput, ChangePasswordInput, SetVaultPasswordInput, RemoveVaultPasswordInput, ResolveVaultConflictInput, TransferSendInput}` (passwords) | no | fixed: hand `Debug` prints `Secret(..)` for passwords; one `*_debug_hides_*` test each in `ipc_contract::secret_debug_tests` |
//! | `ipc_contract::security::{RaiseToCryptoInput, ValidatePasswordStrengthInput, BiometricEnableInput}` | no (no `Debug` derive, no manual impl) | nothing to print |
//! | `ipc_contract::cryptotrack::operations::CryptoOpFilter::token` | no | not a secret: a token symbol filter (false positive of the grep) |

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::{Zeroize, Zeroizing};

/// Stands in for a secret field in a hand-written `Debug`: prints `Secret(..)`.
pub(crate) struct Hidden;

impl fmt::Debug for Hidden {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Secret(..)")
    }
}

/// A value that is wiped on drop and prints as `Secret(..)`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret<T: Zeroize>(Zeroizing<T>);

impl<T: Zeroize> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(Zeroizing::new(value))
    }

    /// The value. The name is the point: every read is greppable.
    pub fn expose(&self) -> &T {
        &self.0
    }
}

impl<T: Zeroize> fmt::Debug for Secret<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Secret(..)")
    }
}

impl<T: Zeroize + Default> Default for Secret<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: Zeroize> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Zeroize + Serialize> Serialize for Secret<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (*self.0).serialize(serializer)
    }
}

impl<'de, T: Zeroize + Deserialize<'de>> Deserialize<'de> for Secret<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        T::deserialize(deserializer).map(Self::new)
    }
}

impl<T: Zeroize + JsonSchema> JsonSchema for Secret<T> {
    fn inline_schema() -> bool {
        T::inline_schema()
    }

    fn schema_name() -> Cow<'static, str> {
        T::schema_name()
    }

    fn schema_id() -> Cow<'static, str> {
        T::schema_id()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> schemars::Schema {
        T::json_schema(generator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text() -> String {
        ["fake", "-", "secret", "-", "7Hq2"].concat()
    }

    #[test]
    fn secret_debug_prints_no_value() {
        let secret = Secret::new(text());
        assert_eq!(format!("{secret:?}"), "Secret(..)");
        assert_eq!(format!("{secret:#?}"), "Secret(..)");
        assert_eq!(secret.expose(), &text());
    }

    #[test]
    fn secret_serializes_as_the_inner_value() {
        let secret = Secret::new(text());
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, serde_json::to_string(&text()).unwrap());
        let back: Secret<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, secret);
    }
}
