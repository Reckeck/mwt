//! Linux: sleep and screen lock as the desktop announces them over D-Bus.
//!
//! The system bus carries logind's `PrepareForSleep` and the session's `Lock`;
//! the session bus carries the screensaver's `ActiveChanged` (GNOME, KDE).
//! Without a bus, or without logind (a container, a Flatpak without the
//! permission), nothing arrives and the sleep fallback of `Watch` (a gap
//! between ticks) is what locks after a sleep; a screen lock is then not seen.

use std::time::Duration;

use dbus::Message;
use dbus::blocking::Connection;
use tauri::AppHandle;

use super::{classify_signal, on_os_event};

const LOGIND: &str = "org.freedesktop.login1";

/// A listener thread per bus. Failures are only logged: the app works without.
pub fn listen(app: AppHandle) {
    let system = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = watch(&system, true) {
            eprintln!("autolock: no logind signals: {err}");
        }
    });
    std::thread::spawn(move || {
        if let Err(err) = watch(&app, false) {
            eprintln!("autolock: no screensaver signals: {err}");
        }
    });
}

/// Our session's object path, so another user's `Lock` does not lock us.
fn own_session_path(conn: &Connection) -> Option<String> {
    let call = Message::new_method_call(
        LOGIND,
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "GetSessionByPID",
    )
    .ok()?
    .append1(std::process::id());
    let reply = conn
        .channel()
        .send_with_reply_and_block(call, Duration::from_secs(2))
        .ok()?;
    reply
        .read1::<dbus::Path>()
        .ok()
        .map(|path| path.to_string())
}

fn watch(app: &AppHandle, system: bool) -> Result<(), dbus::Error> {
    let conn = if system {
        Connection::new_system()?
    } else {
        Connection::new_session()?
    };
    if system {
        conn.add_match_no_cb(&format!(
            "type='signal',sender='{LOGIND}',interface='org.freedesktop.login1.Manager',member='PrepareForSleep'"
        ))?;
        if let Some(path) = own_session_path(&conn) {
            conn.add_match_no_cb(&format!(
                "type='signal',sender='{LOGIND}',interface='org.freedesktop.login1.Session',member='Lock',path='{path}'"
            ))?;
        }
    } else {
        for interface in ["org.freedesktop.ScreenSaver", "org.gnome.ScreenSaver"] {
            conn.add_match_no_cb(&format!(
                "type='signal',interface='{interface}',member='ActiveChanged'"
            ))?;
        }
    }
    let channel = conn.channel();
    loop {
        channel
            .read_write(Some(Duration::from_secs(30)))
            .map_err(|()| dbus::Error::new_failed("the bus connection closed"))?;
        while let Some(message) = channel.pop_message() {
            let (Some(interface), Some(member)) = (message.interface(), message.member()) else {
                continue;
            };
            let flag = message.read1::<bool>().ok();
            if let Some(reason) = classify_signal(&interface, &member, flag) {
                on_os_event(app, reason);
            }
        }
    }
}
