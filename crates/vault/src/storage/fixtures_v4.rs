//! The password and the version of the v4 vault fixtures: a real vault of the pre-reset build,
//! committed as bytes in `fixtures/`.
//!
//! `fixtures/v4.json.wallet` and `fixtures/v4.pack.wallet` are the same document sealed in the
//! two payload formats, `fixtures/v4.expected.json` is that document as JSON. Their password is
//! [`V4_PASSWORD`] (`v4-fixture-password`); it and the exchange keys inside are throwaway values,
//! never real secrets. The generator that made them needs the full fixture document of the
//! facade, so it lives in `mwt-usecases` (`vault_facade_tests/v4_fixtures_gen.rs`).

/// The password of both fixtures.
pub(super) const V4_PASSWORD: &str = "v4-fixture-password";

/// The version the fixtures claim.
pub(super) const V4_VERSION: u32 = 4;
