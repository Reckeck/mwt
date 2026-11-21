//! Which documents hold crypto-module data (the rule that ties a vault to level 3).

use mwt_model::VaultDoc;

/// Whether the document holds crypto-module data, which ties it to level 3.
///
/// One non-empty `crypto_*` collection is enough: the module's addresses and
/// operations are what level 3 exists for, so while any of them is stored the
/// vault may not be protected more cheaply (`check_not_below_min`).
///
/// `crypto_wiped_at` is the one exception, and it has to be: wiping the
/// module (spec E12) empties every collection and leaves exactly that mark
/// behind, so that the other devices learn about the wipe through an ordinary
/// sync. Counting the mark as data would mean a wiped vault could never go
/// back below level 3.
pub fn doc_carries_crypto_data(doc: &VaultDoc) -> bool {
    !doc.crypto_accounts.is_empty()
        || !doc.crypto_events.is_empty()
        || !doc.crypto_overrides.is_empty()
        || !doc.crypto_tokens.is_empty()
        || !doc.crypto_positions.is_empty()
        || !doc.crypto_snapshots.is_empty()
        || !doc.crypto_cursors.is_empty()
        || !doc.crypto_prices.is_empty()
}
