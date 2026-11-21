#![warn(unreachable_pub)]
//! `#[dto]`: one attribute for a data-transfer type.
//!
//! It expands to `#[derive(Serialize, Deserialize)]` plus
//! `#[cfg_attr(feature = "ts", derive(ts_rs::TS))]` and
//! `#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]`. The features are
//! those of the crate that uses the attribute; it declares `ts` and `schema`.
//!
//! Options (comma separated):
//!
//! * `ts(args)` becomes `#[cfg_attr(feature = "ts", ts(args))]`, for example `ts(export)`;
//! * `schema(args)` becomes `#[cfg_attr(feature = "schema", schemars(args))]`;
//! * `serde(args)` becomes `#[serde(args)]`;
//! * `rename = "name"` and `rename_all = "rule"` become `#[serde(rename = ...)]`;
//! * `manual(serde, ts, schema)` leaves out the named derives because the type
//!   implements those traits by hand.
//!
//! ```
//! use mwt_macros::dto;
//!
//! #[dto(rename_all = "camelCase", ts(export))]
//! struct Point {
//!     left_edge: i32,
//! }
//!
//! #[dto]
//! enum Side {
//!     Left,
//!     Right,
//! }
//!
//! fn needs_serde<T: serde::Serialize + for<'de> serde::Deserialize<'de>>() {}
//! needs_serde::<Point>();
//! needs_serde::<Side>();
//! ```

mod expand;
mod options;

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn dto(args: TokenStream, item: TokenStream) -> TokenStream {
    match expand::expand(args.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
