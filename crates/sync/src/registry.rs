//! The sync registry lives in [`mwt_model`]; this path stays for callers.

pub use mwt_model::{
    COLLECTIONS, Collection, DEVICE_SETTINGS, InnerEntry, LOCAL, Rule, SETTINGS_SCOPE, Scope,
    UNION_DAYS, find, is_secret, is_secret_field,
};

#[cfg(test)]
mod tests {
    #[test]
    fn registry_keeps_its_old_path() {
        assert!(super::find("workflows").is_some());
        assert!(!super::COLLECTIONS.is_empty());
        let _: super::Rule = super::Rule::Single;
        let _ = (super::LOCAL, super::SETTINGS_SCOPE);
    }
}
