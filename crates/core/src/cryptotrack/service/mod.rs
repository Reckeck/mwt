//! The crypto module's pure rules: accounts, tokens, links and relations over a
//! `VaultDoc` (spec core-strangler, D1). The scenarios that write sit in
//! `usecases::cryptotrack`.

pub mod accounts;
pub mod links;
pub mod relations;
pub mod tokens;
