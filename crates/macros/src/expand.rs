//! Token output of `#[dto]`, kept apart from the proc-macro entry so it can be unit tested.

use proc_macro2::TokenStream;
use quote::quote;

use crate::options::Options;

pub(crate) fn expand(args: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let options = Options::parse(args)?;
    let serde_derive = options.serde_derive.then(|| {
        quote! { #[derive(::serde::Serialize, ::serde::Deserialize)] }
    });
    let ts_derive = options.ts_derive.then(|| {
        quote! { #[cfg_attr(feature = "ts", derive(::ts_rs::TS))] }
    });
    let schema_derive = options.schema_derive.then(|| {
        quote! { #[cfg_attr(feature = "schema", derive(::schemars::JsonSchema))] }
    });
    let serde_args = &options.serde_args;
    let ts_args = &options.ts_args;
    let schema_args = &options.schema_args;
    Ok(quote! {
        #serde_derive
        #ts_derive
        #schema_derive
        #( #[serde(#serde_args)] )*
        #( #[cfg_attr(feature = "ts", ts(#ts_args))] )*
        #( #[cfg_attr(feature = "schema", schemars(#schema_args))] )*
        #item
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(args: TokenStream, item: TokenStream) -> String {
        expand(args, item).expect("expands").to_string()
    }

    fn squeeze(tokens: TokenStream) -> String {
        tokens.to_string()
    }

    #[test]
    fn plain_struct_gets_the_three_derives() {
        let item = quote! { struct Plain { value: u8 } };
        let want = quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            #[cfg_attr(feature = "ts", derive(::ts_rs::TS))]
            #[cfg_attr(feature = "schema", derive(::schemars::JsonSchema))]
            struct Plain { value: u8 }
        };
        assert_eq!(expanded(quote!(), item), squeeze(want));
    }

    #[test]
    fn enum_gets_the_same_derives() {
        let item = quote! { enum Side { Left, Right } };
        let want = quote! {
            #[derive(::serde::Serialize, ::serde::Deserialize)]
            #[cfg_attr(feature = "ts", derive(::ts_rs::TS))]
            #[cfg_attr(feature = "schema", derive(::schemars::JsonSchema))]
            enum Side { Left, Right }
        };
        assert_eq!(expanded(quote!(), item), squeeze(want));
    }

    #[test]
    fn ts_export_and_rename_become_attributes() {
        let item = quote! { struct Named; };
        let out = expanded(quote!(ts(export), rename = "other"), item);
        assert!(out.contains(r#"# [serde (rename = "other")]"#), "{out}");
        assert!(
            out.contains(r#"# [cfg_attr (feature = "ts" , ts (export))]"#),
            "{out}"
        );
    }

    #[test]
    fn manual_leaves_out_the_named_derives() {
        let out = expanded(quote!(manual(serde, schema)), quote! { struct Hand; });
        assert!(!out.contains("Serialize"), "{out}");
        assert!(!out.contains("JsonSchema"), "{out}");
        assert!(out.contains("ts_rs :: TS"), "{out}");
    }

    #[test]
    fn unknown_option_is_an_error() {
        assert!(expand(quote!(bogus), quote! { struct Bad; }).is_err());
        assert!(expand(quote!(manual(other)), quote! { struct Bad; }).is_err());
    }
}
