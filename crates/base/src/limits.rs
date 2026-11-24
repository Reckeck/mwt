//! Limits that `error`, `model` and `service` all rely on. They live in base
//! so that `error` does not reach up into `service` or `model`.

/// A new level-2 PIN: at least this many digits, digits only.
pub const MIN_PIN_DIGITS: usize = 6;

/// Deepest allowed node depth; a root is depth 0, so the tree has two levels:
/// wallet → sub-account.
pub const MAX_STORE_DEPTH: usize = 1;

/// Longest `range` operation period, in days (op-period P2).
pub const MAX_PERIOD_DAYS: i64 = 366;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_keep_their_values() {
        assert_eq!(MIN_PIN_DIGITS, 6);
        assert_eq!(MAX_STORE_DEPTH, 1);
        assert_eq!(MAX_PERIOD_DAYS, 366);
    }
}
