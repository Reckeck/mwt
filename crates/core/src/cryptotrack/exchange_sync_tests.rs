//! Pins of the pure rules of `exchange_sync`, on a bare `VaultDoc`.

use super::exchange_sync::{
    IMPORT_ACCOUNTS, decided_import_ids, keep_import_run, known_import_ids, normalize_settings,
    remember_discarded,
};
use crate::error::Error;
use crate::model::store::{ImportAccountRoute, ImportSettings};
use crate::model::vault::{ImportRun, MAX_IMPORT_RUNS, PendingImport, VaultDoc};

fn route(account: &str) -> ImportAccountRoute {
    ImportAccountRoute {
        account: account.into(),
        store_id: None,
    }
}

#[test]
fn settings_are_normalized_and_a_bad_account_is_refused() {
    let doc = VaultDoc::default();
    let settings = ImportSettings {
        coins: vec![" sol ".into(), "SOL".into(), "".into()],
        accounts: vec![route("spot")],
        ..ImportSettings::default()
    };
    let kept = normalize_settings(&doc, settings, |_| Ok(())).unwrap();
    assert_eq!(kept.coins, ["SOL"]);
    assert_eq!(
        kept.accounts[0].account,
        crate::exchange::bybit::ACCOUNT_UNIFIED
    );

    for accounts in [
        vec![route("nowhere")],
        vec![route("SPOT"), route("UNIFIED")],
    ] {
        let bad = ImportSettings {
            accounts,
            ..ImportSettings::default()
        };
        assert!(matches!(
            normalize_settings(&doc, bad, |_| Ok(())),
            Err(Error::InvalidInput(_))
        ));
    }
    assert_eq!(IMPORT_ACCOUNTS.len(), 3);
}

#[test]
fn what_was_staged_or_refused_is_known_and_only_the_refused_is_decided() {
    let mut doc = VaultDoc::default();
    doc.pending_imports.push(PendingImport {
        exec_id: "staged".into(),
        ..PendingImport::default()
    });
    remember_discarded(&mut doc, ["refused".to_owned()]);
    let known = known_import_ids(&doc);
    assert!(known.contains("staged") && known.contains("refused"));
    let decided = decided_import_ids(&doc);
    assert!(decided.contains("refused") && !decided.contains("staged"));
}

#[test]
fn the_history_keeps_the_newest_runs() {
    let mut doc = VaultDoc::default();
    for _ in 0..MAX_IMPORT_RUNS + 3 {
        keep_import_run(&mut doc, ImportRun::default());
    }
    assert_eq!(doc.import_runs.len(), MAX_IMPORT_RUNS);
}
