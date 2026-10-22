//! `multi-wallet-tracker --quick-add` opens «+ Операция» in an app that is already
//! running — what a Hyprland/Sway key binding runs, since Wayland gives apps
//! no global keys. The first instance listens on a socket in the runtime
//! directory; a launch with the flag pokes it and exits. Without the flag a
//! second instance starts as usual, so two windows can sit side by side.

#[cfg(all(unix, desktop))]
use std::io::{Read, Write};
#[cfg(all(unix, desktop))]
use std::path::PathBuf;

use tauri::AppHandle;

#[cfg(all(unix, desktop))]
const MESSAGE: &[u8] = b"quick-add";

#[cfg(all(unix, desktop))]
fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        // Per identifier: a dev build and a release each answer their own key.
        .join(format!("{}-quick-add.sock", crate::app_id::get()))
}

/// Called before the app starts: with `--quick-add` and an instance
/// listening, hand the request over and report `true` (the caller exits).
#[cfg(all(unix, desktop))]
pub fn forward_if_running() -> bool {
    if !std::env::args().any(|arg| arg == crate::shortcut::QUICK_ADD_FLAG) {
        return false;
    }
    match std::os::unix::net::UnixStream::connect(socket_path()) {
        Ok(mut stream) => stream.write_all(MESSAGE).is_ok(),
        Err(_) => false,
    }
}

/// Listen for `--quick-add` launches, unless another instance already does.
#[cfg(all(unix, desktop))]
pub fn listen(app: AppHandle) {
    use std::os::unix::net::{UnixListener, UnixStream};
    let path = socket_path();
    if UnixStream::connect(&path).is_ok() {
        return; // an older instance answers the binding
    }
    std::fs::remove_file(&path).ok(); // left by a crash
    let Ok(listener) = UnixListener::bind(&path) else {
        return;
    };
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 16];
            let read = stream.read(&mut buf).unwrap_or(0);
            if &buf[..read] == MESSAGE {
                crate::shortcut::show_quick_add(&app);
            }
        }
    });
}

#[cfg(not(all(unix, desktop)))]
pub fn forward_if_running() -> bool {
    false
}

#[cfg(not(all(unix, desktop)))]
pub fn listen(_app: AppHandle) {}
