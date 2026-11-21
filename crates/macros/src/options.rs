//! Parsing of the `#[dto(...)]` argument list.

use proc_macro2::TokenStream;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, Meta, Token};

/// What the argument list asks for.
#[derive(Default)]
pub(crate) struct Options {
    pub serde_derive: bool,
    pub ts_derive: bool,
    pub schema_derive: bool,
    /// Arguments of `#[serde(...)]`.
    pub serde_args: Vec<TokenStream>,
    /// Arguments of `#[cfg_attr(feature = "ts", ts(...))]`.
    pub ts_args: Vec<TokenStream>,
    /// Arguments of `#[cfg_attr(feature = "schema", schemars(...))]`.
    pub schema_args: Vec<TokenStream>,
}

impl Options {
    pub(crate) fn parse(args: TokenStream) -> syn::Result<Self> {
        let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(args)?;
        let mut options = Options {
            serde_derive: true,
            ts_derive: true,
            schema_derive: true,
            ..Options::default()
        };
        for meta in metas {
            options.apply(&meta)?;
        }
        Ok(options)
    }

    fn apply(&mut self, meta: &Meta) -> syn::Result<()> {
        let name = meta.path().get_ident().map(ToString::to_string);
        match (name.as_deref(), meta) {
            (Some("ts"), Meta::List(list)) => self.ts_args.push(list.tokens.clone()),
            (Some("schema"), Meta::List(list)) => self.schema_args.push(list.tokens.clone()),
            (Some("serde"), Meta::List(list)) => self.serde_args.push(list.tokens.clone()),
            (Some("manual"), Meta::List(list)) => self.apply_manual(list)?,
            (Some(key @ ("rename" | "rename_all")), Meta::NameValue(pair)) => {
                let key = syn::Ident::new(key, pair.path.segments[0].ident.span());
                let value: &Expr = &pair.value;
                self.serde_args.push(quote::quote!(#key = #value));
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "unknown #[dto] option; expected ts(..), schema(..), serde(..), \
                     rename = .., rename_all = .. or manual(..)",
                ));
            }
        }
        Ok(())
    }

    fn apply_manual(&mut self, list: &syn::MetaList) -> syn::Result<()> {
        let names = list.parse_args_with(Punctuated::<syn::Ident, Token![,]>::parse_terminated)?;
        for name in names {
            match name.to_string().as_str() {
                "serde" => self.serde_derive = false,
                "ts" => self.ts_derive = false,
                "schema" => self.schema_derive = false,
                _ => {
                    return Err(syn::Error::new_spanned(
                        name,
                        "manual(..) takes serde, ts or schema",
                    ));
                }
            }
        }
        Ok(())
    }
}
