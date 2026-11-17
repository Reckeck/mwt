//! Limits that `error`, `model` and `service` all rely on. They live in base
//! so that `error` does not reach up into `service` or `model`.

/// A new level-2 PIN: at least this many digits, digits only.
pub const MIN_PIN_DIGITS: usize = 6;

/// Deepest allowed node depth; a root is depth 0, so the tree has two levels:
/// wallet → sub-account.
pub const MAX_STORE_DEPTH: usize = 1;

#[cfg(test)]
mod tests {
    use crate::{limits, model, service};

    #[test]
    fn limits_keep_their_old_paths() {
        assert_eq!(
            service::security_level::MIN_PIN_DIGITS,
            limits::MIN_PIN_DIGITS
        );
        assert_eq!(model::store::MAX_STORE_DEPTH, limits::MAX_STORE_DEPTH);
        assert_eq!(limits::MIN_PIN_DIGITS, 6);
        assert_eq!(limits::MAX_STORE_DEPTH, 1);
    }
}
