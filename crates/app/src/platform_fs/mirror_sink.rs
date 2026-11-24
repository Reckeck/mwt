//! Copies every save of a linked vault to its external document.
//!
//! `persist` runs under the vault lock, and a cloud provider may take seconds
//! to write, so the copy happens on its own thread. Saves that pile up while
//! one is being written collapse into the newest: only the last state matters.

use crate::ipc::events::{AppEvent, VaultMirrorFailed};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use mwt_usecases::service::Session;
use mwt_usecases::vault_io::VaultMirror;
use tauri::AppHandle;

use super::links;

struct MirrorSink {
    queue: Sender<Vec<u8>>,
}

impl VaultMirror for MirrorSink {
    fn push(&self, sealed: &[u8]) {
        let _ = self.queue.send(sealed.to_vec());
    }
}

/// Give `vault` its mirror if it is linked, or take it away if not.
pub fn attach_mirror(app: &AppHandle, vault: &mut Session) {
    let mirror = links::get(app, &vault.path)
        .map(|_| Arc::new(spawn(app.clone(), vault.path.clone())) as Arc<dyn VaultMirror>);
    vault.set_mirror(mirror);
}

fn spawn(app: AppHandle, vault: PathBuf) -> MirrorSink {
    let (queue, saves) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        // Ends when the vault (and with it the sender) is dropped.
        while let Ok(mut bytes) = saves.recv() {
            while let Ok(newer) = saves.try_recv() {
                bytes = newer;
            }
            // Unlinked meanwhile: nothing more to copy.
            let Some(link) = links::get(&app, &vault) else {
                break;
            };
            links::push(&app, &vault, &link, &bytes);
            if let Some(error) = links::get(&app, &vault).and_then(|link| link.last_error) {
                let _ = VaultMirrorFailed::emit(&app, &error);
            }
        }
    });
    MirrorSink { queue }
}
