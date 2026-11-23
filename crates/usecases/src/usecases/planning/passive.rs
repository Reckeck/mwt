//! The one scenario of the passive-income area: the default categories a
//! vault gets when it opens. The rules are in `planning::passive`.

use crate::planning::passive;
use crate::service::Session;

/// Add the built-in categories of every batch the vault has not had yet,
/// skipping keys it already has. Returns whether the doc changed; the caller
/// saves.
pub(crate) fn ensure_default_categories(vault: &mut Session) -> bool {
    passive::ensure_default_categories(&mut vault.doc)
}
