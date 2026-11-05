//! Which of this device's addresses is the local network, for the firewall
//! rule's subnet.

use std::net::Ipv4Addr;

/// An IPv4 address of one network interface.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub ip: Ipv4Addr,
    pub prefix: u8,
    pub loopback: bool,
}

/// The three private ranges, for the commands shown when no address qualifies.
#[cfg(all(target_os = "linux", not(target_os = "android")))]
pub const PRIVATE_RANGES: [&str; 3] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];

const TUNNEL_PREFIXES: [&str; 7] = ["tun", "tap", "wg", "utun", "ppp", "tailscale", "zt"];

fn tunnel(name: &str) -> bool {
    let lower = name.to_lowercase();
    TUNNEL_PREFIXES
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn usable(candidate: &Candidate) -> bool {
    !candidate.loopback
        && !candidate.ip.is_link_local()
        && !candidate.ip.is_unspecified()
        && !tunnel(&candidate.name)
}

/// The LAN address and its prefix. A private address wins (the one the
/// multicast route uses first); a non-private one only when it is the route's.
pub fn pick_lan(interfaces: &[Candidate], route: Option<Ipv4Addr>) -> Option<(Ipv4Addr, u8)> {
    let usable: Vec<&Candidate> = interfaces.iter().filter(|each| usable(each)).collect();
    let private = usable.iter().filter(|each| each.ip.is_private());
    let by_route = private.clone().find(|each| Some(each.ip) == route);
    by_route
        .or_else(|| private.clone().next())
        .or_else(|| usable.iter().find(|each| Some(each.ip) == route))
        .map(|each| (each.ip, each.prefix.min(32)))
}

/// `192.168.1.0/24` for an address and a prefix.
pub fn network_of(ip: Ipv4Addr, prefix: u8) -> String {
    let mask = u32::MAX
        .checked_shl(32 - u32::from(prefix.min(32)))
        .unwrap_or(0);
    format!("{}/{prefix}", Ipv4Addr::from(u32::from(ip) & mask))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nic(name: &str, ip: [u8; 4], prefix: u8) -> Candidate {
        Candidate {
            name: name.into(),
            ip: Ipv4Addr::from(ip),
            prefix,
            loopback: false,
        }
    }

    #[test]
    fn loopback_link_local_and_tunnels_are_skipped() {
        let mut lo = nic("lo", [127, 0, 0, 1], 8);
        lo.loopback = true;
        let list = [
            lo,
            nic("eth1", [169, 254, 3, 4], 16),
            nic("tun0", [10, 8, 0, 2], 24),
            nic("wg0", [10, 9, 0, 2], 24),
            nic("Tailscale", [10, 7, 0, 2], 24),
            nic("wlan0", [192, 168, 1, 20], 24),
        ];
        assert_eq!(pick_lan(&list, None), Some(([192, 168, 1, 20].into(), 24)));
    }

    #[test]
    fn the_route_address_wins_among_private_ones() {
        let list = [
            nic("eth0", [10, 0, 0, 5], 8),
            nic("wlan0", [172, 16, 4, 9], 20),
        ];
        let route = Some(Ipv4Addr::new(172, 16, 4, 9));
        assert_eq!(pick_lan(&list, route), Some(([172, 16, 4, 9].into(), 20)));
        assert_eq!(pick_lan(&list, None), Some(([10, 0, 0, 5].into(), 8)));
    }

    #[test]
    fn a_public_address_counts_only_as_the_route() {
        let list = [nic("eth0", [100, 64, 1, 2], 10)];
        assert_eq!(pick_lan(&list, None), None);
        let route = Some(Ipv4Addr::new(100, 64, 1, 2));
        assert_eq!(pick_lan(&list, route), Some(([100, 64, 1, 2].into(), 10)));
    }

    #[test]
    fn nothing_usable_gives_none() {
        let list = [nic("tun0", [10, 8, 0, 2], 24)];
        assert_eq!(pick_lan(&list, Some([10, 8, 0, 2].into())), None);
        assert_eq!(pick_lan(&[], None), None);
    }

    #[test]
    fn network_is_masked() {
        assert_eq!(network_of([192, 168, 1, 77].into(), 24), "192.168.1.0/24");
        assert_eq!(network_of([172, 16, 4, 9].into(), 20), "172.16.0.0/20");
        assert_eq!(network_of([10, 1, 2, 3].into(), 0), "0.0.0.0/0");
    }
}
