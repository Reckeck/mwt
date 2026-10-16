//! An uninstall must not leave the app starting with the system. On Linux the
//! autostart entry is a file in the user's home that no package manager
//! removes, so it carries `TryExec=` with the program's path: sessions skip
//! the entry once that file is gone (an AppImage deleted by hand included).
//! Installers on Windows remove their registry value themselves
//! (`crates/app/windows/`), packages on Linux run `packaging/linux/postrm.sh`.

use tauri::AppHandle;

/// Mark the autostart entry, if there is one, with the program it starts.
/// Called after every enable and on each start (entries written before).
#[cfg(target_os = "linux")]
pub fn guard_entry(app: &AppHandle) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    // The path tauri-plugin-autostart (auto-launch) writes to.
    let file = home
        .join(".config/autostart")
        .join(format!("{}.desktop", app.package_info().name));
    let Ok(text) = std::fs::read_to_string(&file) else {
        return;
    };
    if let Some(updated) = with_try_exec(&text)
        && let Err(err) = std::fs::write(&file, updated)
    {
        eprintln!("autostart entry {}: {err}", file.display());
    }
}

#[cfg(not(target_os = "linux"))]
pub fn guard_entry(_app: &AppHandle) {}

/// The entry with `TryExec=` set to the program of its `Exec=` line, or
/// `None` when it already has the right one (or no `Exec=`).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn with_try_exec(text: &str) -> Option<String> {
    let exec = text.lines().find_map(|line| line.strip_prefix("Exec="))?;
    // auto-launch writes «Exec=<path> <args>»; the only argument is --hidden,
    // and the path itself may contain spaces.
    let program = exec.strip_suffix(" --hidden").unwrap_or(exec).trim_end();
    let wanted = format!("TryExec={program}");
    if text.lines().any(|line| line == wanted) {
        return None;
    }
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with("TryExec="))
        .collect();
    let at = lines
        .iter()
        .position(|line| line.starts_with("Exec="))
        .map_or(lines.len(), |index| index + 1);
    lines.insert(at, &wanted);
    Some(lines.join("\n") + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = "[Desktop Entry]\nType=Application\nName=Multi Wallet Tracker\n\
        Exec=/usr/bin/multi-wallet-tracker --hidden\nTerminal=false";

    #[test]
    fn adds_try_exec_after_exec() {
        let updated = with_try_exec(ENTRY).unwrap();
        assert!(updated.contains(
            "Exec=/usr/bin/multi-wallet-tracker --hidden\nTryExec=/usr/bin/multi-wallet-tracker\n"
        ));
        assert_eq!(with_try_exec(&updated), None);
    }

    #[test]
    fn replaces_a_stale_try_exec() {
        let stale = ENTRY.replace("Terminal=false", "TryExec=/old/path\nTerminal=false");
        let updated = with_try_exec(&stale).unwrap();
        assert!(!updated.contains("/old/path"));
        assert_eq!(updated.matches("TryExec=").count(), 1);
    }

    #[test]
    fn keeps_spaces_in_the_program_path() {
        let entry = "Exec=/home/me/My Apps/mwt.AppImage --hidden";
        assert_eq!(
            with_try_exec(entry).unwrap(),
            "Exec=/home/me/My Apps/mwt.AppImage --hidden\nTryExec=/home/me/My Apps/mwt.AppImage\n"
        );
    }

    #[test]
    fn leaves_an_entry_without_exec() {
        assert_eq!(with_try_exec("[Desktop Entry]\nName=x"), None);
    }
}
