//! Letting other devices reach this one through its firewall: only the sync
//! port, only from the local network (docs/sync.md, «Фаервол»).
//!
//! The rule is added by the system tool behind the system's own password
//! prompt (`pkexec` on Linux, UAC on Windows). A Flatpak cannot reach the
//! host firewall, so it only shows the commands.

use std::net::{IpAddr, Ipv4Addr};

use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::ipc_contract::SyncFirewallFix;

use super::discovery::local_address;
// Only the systems with a firewall rule to offer name the port.
#[cfg(any(windows, all(target_os = "linux", not(target_os = "android"))))]
use super::discovery::SYNC_PORT;

/// The local network of this device, as `192.168.1.0/24`.
fn subnet() -> String {
    let Some(IpAddr::V4(here)) = local_address() else {
        return "192.168.0.0/16".into();
    };
    let prefix = if_addrs::get_if_addrs()
        .ok()
        .and_then(|interfaces| {
            interfaces
                .into_iter()
                .find_map(|interface| match interface.addr {
                    if_addrs::IfAddr::V4(addr) if addr.ip == here => Some(addr.prefixlen),
                    _ => None,
                })
        })
        .unwrap_or(24)
        .min(32);
    let mask = u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0);
    let network = Ipv4Addr::from(u32::from(here) & mask);
    format!("{network}/{prefix}")
}

/// How the firewall can be opened here, and the commands to do it by hand.
pub fn fix() -> (SyncFirewallFix, Vec<String>) {
    platform::fix(&subnet())
}

/// Add the rule (asks for the system password).
pub async fn open() -> Result<()> {
    let subnet = subnet();
    tauri::async_runtime::spawn_blocking(move || platform::open(&subnet))
        .await
        .map_err(|err| Error::Network(err.to_string()))?
}

fn refused() -> Error {
    Error::PeerSync(PeerSyncFailure::Firewall)
}

#[cfg(all(target_os = "linux", not(target_os = "android")))]
mod platform {
    use std::path::Path;
    use std::process::Command;

    use super::{SYNC_PORT, SyncFirewallFix, refused};
    use trade_core::error::Result;

    const COMMENT: &str = "Multi Wallet Tracker sync";

    fn ufw(subnet: &str) -> String {
        format!("ufw allow from {subnet} to any port {SYNC_PORT} proto tcp comment '{COMMENT}'")
    }

    fn firewalld(subnet: &str) -> String {
        format!(
            "firewall-cmd --permanent --add-rich-rule='rule family=\"ipv4\" source address=\"{subnet}\" port port=\"{SYNC_PORT}\" protocol=\"tcp\" accept' && firewall-cmd --reload"
        )
    }

    fn in_path(tool: &str) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
    }

    pub fn fix(subnet: &str) -> (SyncFirewallFix, Vec<String>) {
        let commands = vec![
            format!("sudo {}", ufw(subnet)),
            format!("sudo {}", firewalld(subnet).replace("&& ", "&& sudo ")),
        ];
        let sandboxed = Path::new("/.flatpak-info").exists();
        let how = if !sandboxed && in_path("pkexec") {
            SyncFirewallFix::Auto
        } else {
            SyncFirewallFix::Manual
        };
        (how, commands)
    }

    pub fn open(subnet: &str) -> Result<()> {
        // The firewall that is on: ufw, else firewalld; exit 3 = none.
        let script = format!(
            "if command -v ufw >/dev/null 2>&1 && ufw status | grep -q 'Status: active'; then {}; \
             elif command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then {}; \
             else exit 3; fi",
            ufw(subnet),
            firewalld(subnet)
        );
        let status = Command::new("pkexec")
            .args(["/bin/sh", "-c", &script])
            .status()
            .map_err(|_| refused())?;
        if status.success() {
            Ok(())
        } else {
            Err(refused())
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use super::{SYNC_PORT, SyncFirewallFix, refused};
    use trade_core::error::Result;

    /// Without a console window flashing up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn rule() -> String {
        format!(
            "advfirewall firewall add rule name=\"Multi Wallet Tracker sync\" dir=in action=allow protocol=TCP localport={SYNC_PORT} remoteip=localsubnet profile=private"
        )
    }

    pub fn fix(_subnet: &str) -> (SyncFirewallFix, Vec<String>) {
        (SyncFirewallFix::Auto, vec![format!("netsh {}", rule())])
    }

    pub fn open(_subnet: &str) -> Result<()> {
        let script = format!(
            "$p = Start-Process netsh -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '{}'; exit $p.ExitCode",
            rule()
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|_| refused())?;
        if status.success() {
            Ok(())
        } else {
            Err(refused())
        }
    }
}

/// macOS asks by itself the first time the app listens; phones have no
/// firewall of their own.
#[cfg(not(any(windows, all(target_os = "linux", not(target_os = "android")))))]
mod platform {
    use super::{SyncFirewallFix, refused};
    use trade_core::error::Result;

    pub fn fix(_subnet: &str) -> (SyncFirewallFix, Vec<String>) {
        (SyncFirewallFix::None, Vec::new())
    }

    pub fn open(_subnet: &str) -> Result<()> {
        Err(refused())
    }
}
