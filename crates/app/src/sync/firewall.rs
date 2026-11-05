//! Letting other devices reach this one through its firewall: only the sync
//! port and the one a vault is received on, only from the local network (docs/sync.md, "Firewall").
//!
//! The rule is added by the system tool behind the system's own password
//! prompt (`pkexec` on Linux, UAC on Windows). A Flatpak cannot reach the
//! host firewall, so it only shows the commands.

use std::net::IpAddr;

use trade_core::error::{Error, PeerSyncFailure, Result};
use trade_core::ipc_contract::SyncFirewallFix;

use super::discovery::local_address;
#[cfg(all(target_os = "linux", not(target_os = "android")))]
use super::lan::PRIVATE_RANGES;
use super::lan::{Candidate, network_of, pick_lan};
// The rule covers this range: the listeners stay inside it (`ports.rs`).
#[cfg(any(windows, test, all(target_os = "linux", not(target_os = "android"))))]
use super::ports::{RULE_FIRST, RULE_LAST};

/// The local network of this device, as `192.168.1.0/24`; `None` when no
/// interface looks like a LAN (only tunnels, or no private address).
fn subnet() -> Option<String> {
    let route = match local_address() {
        Some(IpAddr::V4(here)) => Some(here),
        _ => None,
    };
    let interfaces: Vec<Candidate> = if_addrs::get_if_addrs()
        .ok()?
        .into_iter()
        .filter_map(|interface| match &interface.addr {
            if_addrs::IfAddr::V4(addr) => Some(Candidate {
                name: interface.name.clone(),
                ip: addr.ip,
                prefix: addr.prefixlen,
                loopback: interface.is_loopback(),
            }),
            _ => None,
        })
        .collect();
    pick_lan(&interfaces, route).map(|(ip, prefix)| network_of(ip, prefix))
}

/// How the firewall can be opened here, and the commands to do it by hand.
/// With no LAN address found the commands cover the three private ranges and
/// the fix is manual.
pub fn fix() -> (SyncFirewallFix, Vec<String>) {
    match subnet() {
        Some(found) => platform::fix(&[found]),
        None => platform::fix_unknown(),
    }
}

/// Add the rule (asks for the system password).
pub async fn open() -> Result<()> {
    let subnet = subnet();
    tauri::async_runtime::spawn_blocking(move || platform::open(subnet.as_deref()))
        .await
        .map_err(|err| Error::Network(err.to_string()))?
}

/// The Windows rule name; one rule, replaced on every press.
#[cfg(any(windows, test))]
const RULE_NAME: &str = "Multi Wallet Tracker sync";

/// The `netsh` arguments that add the rule.
#[cfg(any(windows, test))]
fn add_rule() -> String {
    format!(
        "advfirewall firewall add rule name=\"{RULE_NAME}\" dir=in action=allow protocol=TCP localport={RULE_FIRST}-{RULE_LAST} remoteip=localsubnet profile=private"
    )
}

/// The PowerShell text that adds the rule behind UAC. Stops on any error and
/// exits non-zero when UAC is refused (no process) or `netsh` fails; the old
/// rule is deleted first so a second press leaves one rule. ASCII only.
#[cfg(any(windows, test))]
fn open_script() -> String {
    let args = format!(
        "/c netsh advfirewall firewall delete rule name=\"{RULE_NAME}\" >nul 2>&1 & netsh {}",
        add_rule()
    );
    format!(
        "$ErrorActionPreference = 'Stop'; try {{ $p = Start-Process cmd -Verb RunAs -Wait -PassThru -WindowStyle Hidden -ArgumentList '{args}'; if (-not $p) {{ exit 1 }}; exit $p.ExitCode }} catch {{ exit 1 }}"
    )
}

fn refused() -> Error {
    Error::PeerSync(PeerSyncFailure::Firewall)
}

#[cfg(all(target_os = "linux", not(target_os = "android")))]
mod platform {
    use std::path::Path;
    use std::process::Command;

    use super::{PRIVATE_RANGES, RULE_FIRST, RULE_LAST, SyncFirewallFix, refused};
    use trade_core::error::Result;

    const COMMENT: &str = "Multi Wallet Tracker sync";

    fn ufw(subnet: &str) -> String {
        format!(
            "ufw allow from {subnet} to any port {RULE_FIRST}:{RULE_LAST} proto tcp comment '{COMMENT}'"
        )
    }

    fn firewalld(subnet: &str) -> String {
        format!(
            "firewall-cmd --permanent --add-rich-rule='rule family=\"ipv4\" source address=\"{subnet}\" port port=\"{RULE_FIRST}-{RULE_LAST}\" protocol=\"tcp\" accept' && firewall-cmd --reload"
        )
    }

    fn in_path(tool: &str) -> bool {
        std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(tool).is_file()))
    }

    fn commands(subnets: &[String]) -> Vec<String> {
        subnets
            .iter()
            .flat_map(|subnet| {
                [
                    format!("sudo {}", ufw(subnet)),
                    format!("sudo {}", firewalld(subnet).replace("&& ", "&& sudo ")),
                ]
            })
            .collect()
    }

    fn can_pkexec() -> bool {
        !Path::new("/.flatpak-info").exists() && in_path("pkexec")
    }

    pub fn fix(subnets: &[String]) -> (SyncFirewallFix, Vec<String>) {
        let how = if can_pkexec() {
            SyncFirewallFix::Auto
        } else {
            SyncFirewallFix::Manual
        };
        (how, commands(subnets))
    }

    /// No LAN address found: all three private ranges, by hand.
    pub fn fix_unknown() -> (SyncFirewallFix, Vec<String>) {
        let ranges: Vec<String> = PRIVATE_RANGES.iter().map(|range| (*range).into()).collect();
        (SyncFirewallFix::Manual, commands(&ranges))
    }

    pub fn open(subnet: Option<&str>) -> Result<()> {
        let subnet = subnet.ok_or_else(refused)?;
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

    use super::{SyncFirewallFix, add_rule, open_script, refused};
    use trade_core::error::Result;

    /// Without a console window flashing up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn fix(_subnets: &[String]) -> (SyncFirewallFix, Vec<String>) {
        (SyncFirewallFix::Auto, vec![format!("netsh {}", add_rule())])
    }

    /// `localsubnet` needs no address, so the Windows fix does not depend on one.
    pub fn fix_unknown() -> (SyncFirewallFix, Vec<String>) {
        fix(&[])
    }

    pub fn open(_subnet: Option<&str>) -> Result<()> {
        let status = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &open_script()])
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

    pub fn fix(_subnets: &[String]) -> (SyncFirewallFix, Vec<String>) {
        (SyncFirewallFix::None, Vec::new())
    }

    pub fn fix_unknown() -> (SyncFirewallFix, Vec<String>) {
        fix(&[])
    }

    pub fn open(_subnet: Option<&str>) -> Result<()> {
        Err(refused())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_stops_on_error_and_fails_on_a_refused_uac() {
        let script = open_script();
        assert!(script.starts_with("$ErrorActionPreference = 'Stop'"));
        assert!(script.contains("if (-not $p) { exit 1 }"));
        assert!(script.contains("catch { exit 1 }"));
        assert!(script.contains("exit $p.ExitCode"));
        assert!(script.is_ascii());
    }

    #[test]
    fn script_deletes_the_named_rule_before_adding_it() {
        let script = open_script();
        let delete = script
            .find("delete rule name=\"Multi Wallet Tracker sync\"")
            .unwrap();
        let add = script
            .find("add rule name=\"Multi Wallet Tracker sync\"")
            .unwrap();
        assert!(delete < add);
    }

    #[test]
    fn the_rule_covers_the_whole_listener_range_on_the_private_profile() {
        let rule = add_rule();
        assert!(rule.contains("localport=53318-53327"));
        assert!(rule.contains("remoteip=localsubnet profile=private"));
    }

    #[cfg(all(target_os = "linux", not(target_os = "android")))]
    #[test]
    fn unknown_network_is_manual_with_all_three_private_ranges() {
        let (how, commands) = platform::fix_unknown();
        assert_eq!(how, SyncFirewallFix::Manual);
        for range in PRIVATE_RANGES {
            assert!(commands.iter().any(|command| command.contains(range)));
        }
        assert!(commands.iter().all(|command| command.contains("53318")));
    }
}
