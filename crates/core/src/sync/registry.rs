//! The sync registry lives in [`crate::model::registry`]; this path stays for callers.

pub use crate::model::registry::*;

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
