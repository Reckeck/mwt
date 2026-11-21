//! Typed identifiers (spec core-strangler, ADR-03 `id_types!`).
//!
//! Each ID is a newtype over `String` that serializes as the plain string and
//! exports as TS `string`, so JSON, the vault and `ipc.gen.d.ts` do not change.
//! New signatures use these types; model struct fields keep `String` for now.

/// Defines one newtype per name. Every path in the expansion is fully
/// qualified (`::std::`, `::serde::`, `::ts_rs::`), so the macro expands the
/// same wherever it is invoked (ADR-03 rule 4).
#[macro_export]
macro_rules! id_types {
    ($($(#[$meta:meta])* $name:ident;)+) => {$(
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::serde::Serialize, ::serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(::std::string::String);

        impl $name {
            /// The ID as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        #[cfg(feature = "ts")]
        impl ::ts_rs::TS for $name {
            type WithoutGenerics = Self;
            type OptionInnerType = Self;

            fn name(_cfg: &::ts_rs::Config) -> ::std::string::String {
                "string".into()
            }

            fn ident(_cfg: &::ts_rs::Config) -> ::std::string::String {
                "string".into()
            }

            fn decl(_cfg: &::ts_rs::Config) -> ::std::string::String {
                ::std::concat!("type ", ::std::stringify!($name), " = string;").into()
            }

            fn decl_concrete(cfg: &::ts_rs::Config) -> ::std::string::String {
                <Self as ::ts_rs::TS>::decl(cfg)
            }

            fn inline(_cfg: &::ts_rs::Config) -> ::std::string::String {
                "string".into()
            }

            fn inline_flattened(cfg: &::ts_rs::Config) -> ::std::string::String {
                <Self as ::ts_rs::TS>::inline(cfg)
            }

            fn visit_dependencies(_: &mut impl ::ts_rs::TypeVisitor) {}
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, out: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                out.write_str(&self.0)
            }
        }

        impl ::std::convert::From<::std::string::String> for $name {
            fn from(raw: ::std::string::String) -> Self {
                Self(raw)
            }
        }

        impl ::std::convert::From<&str> for $name {
            fn from(raw: &str) -> Self {
                Self(::std::string::ToString::to_string(raw))
            }
        }

        impl ::std::convert::From<&::std::string::String> for $name {
            fn from(raw: &::std::string::String) -> Self {
                Self(raw.clone())
            }
        }

        impl ::std::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    )+};
}

id_types! {
    /// A store: wallet, account, card, deposit or bonus programme.
    StoreId;
}

#[cfg(all(test, feature = "ts"))]
mod tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn serializes_as_the_plain_string() {
        let id = StoreId::from("card-1");
        assert_eq!(serde_json::to_string(&id).unwrap(), r#""card-1""#);
        let back: StoreId = serde_json::from_str(r#""card-1""#).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn converts_displays_and_borrows_as_str() {
        let from_string = StoreId::from(String::from("a"));
        assert_eq!(from_string, StoreId::from("a"));
        assert_eq!(from_string.to_string(), "a");
        assert_eq!(AsRef::<str>::as_ref(&from_string), "a");
        assert_eq!(from_string.as_str(), "a");
    }

    #[test]
    fn exports_as_ts_string() {
        assert_eq!(StoreId::inline(&ts_rs::Config::default()), "string");
    }

    #[derive(TS)]
    #[allow(dead_code)]
    struct Holder {
        store_id: StoreId,
    }

    #[test]
    fn exported_struct_field_is_a_plain_string() {
        let decl = Holder::decl(&ts_rs::Config::default());
        assert!(decl.contains("store_id: string"), "{decl}");
    }
}
