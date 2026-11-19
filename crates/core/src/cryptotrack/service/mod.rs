//! The crypto module's pure rules: accounts, tokens, links, relations and the
//! recompute family (cursors, folds, totals, the capital) over a `VaultDoc` or
//! the `Books` of a request (spec core-strangler, D1). The scenarios that
//! write sit in `usecases::cryptotrack`.

pub mod accounts;
pub mod apply;
pub mod links;
pub mod recompute;
pub mod relations;
pub mod tokens;
pub mod totals;
#[cfg(test)]
mod volume;
