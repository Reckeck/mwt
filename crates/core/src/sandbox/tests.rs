use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use tempfile::tempdir;

use super::*;
use crate::Amount;
use crate::model::transaction::Transaction;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 10, 25).unwrap()
}

/// The document without the sync stamps: those carry the wall clock of the save.
fn document(vault: &UnlockedVault) -> serde_json::Value {
    let mut value = serde_json::to_value(&vault.doc).unwrap();
    value.as_object_mut().unwrap().remove("sync");
    value
}

/// Income and spending per month (`YYYY-MM`), in whole dollars.
fn months(vault: &UnlockedVault) -> BTreeMap<String, (Amount, Amount)> {
    let mut sums: BTreeMap<String, (Amount, Amount)> = BTreeMap::new();
    for tx in &vault.doc.transactions {
        let meta = tx.meta();
        let month = meta.timestamp[..7].to_string();
        let entry = sums.entry(month).or_default();
        match tx {
            Transaction::Topup(inner)
                if !meta.categories.contains(&"opening_balance".to_string()) =>
            {
                entry.0 += inner.amount;
            }
            Transaction::Payment(inner) => entry.1 += inner.amount,
            _ => {}
        }
    }
    sums
}

#[test]
fn the_seed_is_deterministic() {
    let (first_dir, second_dir) = (tempdir().unwrap(), tempdir().unwrap());
    let first = SandboxDir::in_cache(first_dir.path())
        .create(today())
        .unwrap();
    let second = SandboxDir::in_cache(second_dir.path())
        .create(today())
        .unwrap();
    assert_eq!(document(&first), document(&second));
    assert!(first.doc.transactions.len() > 200);
    // Another day is another document.
    let other_dir = tempdir().unwrap();
    let other = SandboxDir::in_cache(other_dir.path())
        .create(today() - chrono::Duration::days(40))
        .unwrap();
    assert_ne!(document(&first), document(&other));
}

#[test]
fn twelve_months_of_a_plausible_year() {
    let cache = tempdir().unwrap();
    let vault = SandboxDir::in_cache(cache.path()).create(today()).unwrap();
    let sums = months(&vault);
    assert_eq!(sums.len(), 12);
    assert_eq!(sums.keys().next().unwrap(), "2025-11");
    assert_eq!(sums.keys().last().unwrap(), "2026-10");
    for (month, (income, _)) in &sums {
        // The salary comes on the 5th; this month's is in too.
        assert!(
            *income >= Amount::from(1500) && *income <= Amount::from(1650),
            "{month}: salary {income}"
        );
    }
    // August is in the red; December spends more than a usual month.
    let (income, spent) = sums["2026-08"];
    assert!(spent > income, "August must be in the red");
    let usual = sums["2026-03"].1;
    assert!(sums["2025-12"].1 > usual + Amount::from(200));
    // The rest of the year is in the black.
    assert!(sums["2026-05"].0 > sums["2026-05"].1);
    let categories: BTreeSet<_> = vault
        .doc
        .transactions
        .iter()
        .flat_map(|tx| tx.meta().categories.clone())
        .collect();
    assert!(categories.len() >= 13, "{categories:?}");
    let kinds: BTreeSet<_> = vault
        .doc
        .stores
        .iter()
        .map(|store| format!("{:?}", store.kind))
        .collect();
    assert!(kinds.len() >= 3, "{kinds:?}");
    // Nothing from today on: the future-date rule is not tested by a clock.
    let limit = "2026-10-25T";
    assert!(
        vault
            .doc
            .transactions
            .iter()
            .all(|tx| tx.meta().timestamp.as_str() < limit)
    );
}

#[test]
fn the_vault_has_no_password_and_lives_in_the_folder() {
    let cache = tempdir().unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    let vault = dir.create(today()).unwrap();
    assert!(!vault.is_encrypted());
    assert!(!crate::crypto::file_is_encrypted(&dir.file()));
    assert!(dir.contains(&vault.path).is_ok());
    drop(vault);
    let reopened = UnlockedVault::open(dir.file(), "").unwrap();
    assert!(reopened.doc.transactions.len() > 200);
}

#[test]
fn erasing_removes_what_a_crash_left() {
    let cache = tempdir().unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    let vault = dir.create(today()).unwrap();
    std::mem::forget(vault); // as after a crash: the lock and the file stay
    std::fs::create_dir_all(dir.root().join("nested")).unwrap();
    std::fs::write(dir.root().join("nested/stray.bin"), b"x").unwrap();
    std::fs::write(cache.path().join("neighbour.txt"), b"keep").unwrap();
    dir.erase().unwrap();
    assert!(!dir.root().exists());
    assert!(cache.path().join("neighbour.txt").exists());
    // Nothing to erase is fine.
    dir.erase().unwrap();
}

#[cfg(unix)]
#[test]
fn erasing_never_follows_a_link_out() {
    let cache = tempdir().unwrap();
    let real = tempdir().unwrap();
    std::fs::write(real.path().join("vault.wallet"), b"real data").unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    std::os::unix::fs::symlink(real.path(), dir.root()).unwrap();
    dir.erase().unwrap();
    assert!(std::fs::symlink_metadata(dir.root()).is_err());
    assert!(real.path().join("vault.wallet").exists());
    // A link inside the folder is removed as a link too.
    std::fs::create_dir_all(dir.root()).unwrap();
    std::os::unix::fs::symlink(real.path(), dir.root().join("out")).unwrap();
    dir.erase().unwrap();
    assert!(real.path().join("vault.wallet").exists());
}

#[test]
fn a_path_outside_the_folder_is_refused() {
    let cache = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    let vault = dir.create(today()).unwrap();
    let real = outside.path().join("real.wallet");
    std::fs::write(&real, b"real").unwrap();
    for path in [
        real.clone(),
        outside.path().join("new.wallet"),
        cache.path().to_path_buf(),
        dir.root().to_path_buf(),
        dir.root().join("../neighbour.wallet"),
        dir.root().join("..").join(DIR_NAME).join(FILE_NAME),
    ] {
        let err = dir.contains(&path).unwrap_err();
        assert_eq!(err.code(), "SANDBOX", "{path:?}");
    }
    assert!(dir.contains(&dir.file()).is_ok());
    assert!(dir.contains(&dir.root().join("new-file")).is_ok());
    drop(vault);
}

#[cfg(unix)]
#[test]
fn a_link_inside_the_folder_is_refused() {
    let cache = tempdir().unwrap();
    let outside = tempdir().unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    let _vault = dir.create(today()).unwrap();
    let real = outside.path().join("real.wallet");
    std::fs::write(&real, b"real").unwrap();
    let link = dir.root().join("link.wallet");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(dir.contains(&link).unwrap_err().code(), "SANDBOX");
    let folder_link = dir.root().join("folder");
    std::os::unix::fs::symlink(outside.path(), &folder_link).unwrap();
    assert_eq!(
        dir.contains(&folder_link.join("new.wallet"))
            .unwrap_err()
            .code(),
        "SANDBOX"
    );
}

#[test]
fn a_sandbox_folder_that_is_a_link_is_not_used() {
    let cache = tempdir().unwrap();
    let wrong = SandboxDir {
        root: cache.path().join("not-the-sandbox"),
    };
    assert_eq!(wrong.erase().unwrap_err().code(), "SANDBOX");
}

#[test]
fn a_reset_brings_back_the_original_data() {
    let cache = tempdir().unwrap();
    let dir = SandboxDir::in_cache(cache.path());
    let mut vault = dir.create(today()).unwrap();
    let original = document(&vault);
    let victim = vault.doc.transactions[10].meta().id.clone();
    vault.delete_transaction(&victim).unwrap();
    vault
        .create_category(
            None,
            "Мои траты",
            crate::model::category::CategoryGroup::Daily,
        )
        .unwrap();
    assert_ne!(document(&vault), original);
    drop(vault);
    let fresh = dir.create(today()).unwrap();
    assert_eq!(document(&fresh), original);
    assert_eq!(
        std::fs::read_dir(dir.root()).unwrap().count(),
        2,
        "the vault and its lock"
    );
}

#[test]
fn the_demo_has_limits_goals_and_automations() {
    let dir = tempdir().unwrap();
    let vault = SandboxDir::in_cache(dir.path()).create(today()).unwrap();
    assert_eq!(vault.doc.budgets.len(), 3);
    assert_eq!(vault.doc.goals.len(), 2);
    assert_eq!(vault.doc.workflows.len(), 3);
    // One limit is over: the demo shows the red card too.
    let over = vault
        .budget_status(
            today().and_hms_opt(12, 0, 0).unwrap().and_utc().timestamp(),
            "UTC",
        )
        .iter()
        .filter(|status| status.remaining.is_negative())
        .count();
    assert!(over >= 1, "no limit is over");
}
