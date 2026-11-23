//! The preview and the import intake of `exchange_sync` count a repeated id alike (needs the vault).

use crate::cryptotrack::exchange_sync::{intake, known_import_ids, preview_exchange_sync};
use crate::model::store::ImportSettings;
use crate::model::vault::{ImportKind, PendingImport};
use crate::service::Session;

fn fill(exec_id: &str) -> PendingImport {
    PendingImport {
        exec_id: exec_id.into(),
        kind: ImportKind::Fill,
        ..PendingImport::default()
    }
}

#[test]
fn a_preview_counts_a_repeated_exec_id_once_like_the_import() {
    let dir = tempfile::tempdir().unwrap();
    let vault = Session::create(dir.path().join("test.wallet"), "pw").unwrap();
    let books = vault.books();
    let batch = vec![fill("twice"), fill("twice"), fill(""), fill("other")];
    let settings = ImportSettings::default();
    let preview = preview_exchange_sync(&books, batch.clone(), &settings);
    let taken = intake(&books, batch, &settings, known_import_ids(books.doc()));
    assert_eq!(
        (
            preview.fetched,
            preview.known,
            preview.filtered,
            preview.new
        ),
        (4, 2, 0, 2)
    );
    assert_eq!(preview.items, taken.fresh);
}
