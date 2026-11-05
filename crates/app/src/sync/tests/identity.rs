//! Fix C, slice 8: run generations and vault identity.

use super::*;
use crate::sync::teardown;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn teardown_invalidates_a_start_that_is_still_in_flight() {
    let dir = tempfile::tempdir().unwrap();
    let (left, _right) = same_vault(dir.path());
    let (host, _events) = device("Laptop", left, SyncRole::Main);
    let before = host.hub().generation;
    teardown(&host);
    assert_ne!(
        host.hub().generation,
        before,
        "a stop must void the generation of a pending start"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_vault_switched_under_a_live_session_sends_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (left, right) = same_vault(dir.path());
    let (main, main_events) = device("Laptop", left, SyncRole::Main);
    let (client, client_events) = device("Phone", right, SyncRole::Client);
    let _main_seen = answer(main.clone(), main_events, true);
    let _client_seen = answer(client.clone(), client_events, true);
    connect(&main, &client).await;
    until("both links live", || live(&main) && live(&client)).await;

    // Another vault takes the phone's place under the running session.
    let mut other = UnlockedVault::create(dir.path().join("other.wallet"), "pw").unwrap();
    other.persist().unwrap();
    other.set_device(&client.settings().device_id);
    other.set_sync_tap(Some(Arc::new(Tap(client.clone()))));
    *client.0.vault.lock().unwrap() = Some(other);
    seed(&client, "stray from another vault");

    until("the session of the old vault ends", || {
        client.hub().links.is_empty()
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !comments(&main).iter().any(|text| text.contains("stray")),
        "another vault's record reached the laptop"
    );
}
