//! The ports sync listens on. Everything the firewall rule covers is one
//! range, so a busy preferred port still lands inside the rule.

use std::io;
use std::net::TcpListener;

/// First port of the firewall rule: the sync listener.
#[cfg(any(windows, test, all(target_os = "linux", not(target_os = "android"))))]
pub const RULE_FIRST: u16 = 53318;
/// Last port of the firewall rule.
pub const RULE_LAST: u16 = 53327;
/// Where a listener goes when its own port is busy: after the two preferred ones.
const SPARE_FIRST: u16 = 53320;

/// `preferred`, then the spare ports of the rule range, in order.
pub fn candidates(preferred: u16) -> Vec<u16> {
    std::iter::once(preferred)
        .chain(SPARE_FIRST..=RULE_LAST)
        .collect()
}

/// The first of `ports` that binds, else any free port (0), which the rule
/// does not cover.
pub fn bind_first(host: &str, ports: &[u16]) -> io::Result<TcpListener> {
    for port in ports {
        if let Ok(listener) = TcpListener::bind((host, *port)) {
            return Ok(listener);
        }
    }
    TcpListener::bind((host, 0))
}

/// Same as [`bind_first`], as a tokio listener.
pub fn listen_in_range(preferred: u16) -> io::Result<tokio::net::TcpListener> {
    let listener = bind_first("0.0.0.0", &candidates(preferred))?;
    listener.set_nonblocking(true)?;
    tokio::net::TcpListener::from_std(listener)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy() -> TcpListener {
        TcpListener::bind(("127.0.0.1", 0)).unwrap()
    }

    fn free_port() -> u16 {
        busy().local_addr().unwrap().port()
    }

    #[test]
    fn the_range_is_ten_ports_and_candidates_stay_inside() {
        assert_eq!(RULE_LAST - RULE_FIRST + 1, 10);
        let list = candidates(53319);
        assert_eq!(list[0], 53319);
        assert!(
            list.iter()
                .all(|port| (RULE_FIRST..=RULE_LAST).contains(port))
        );
        assert_eq!(list.len(), 9);
    }

    #[test]
    fn a_busy_first_port_moves_to_the_next_free_one() {
        let first = busy();
        let first_port = first.local_addr().unwrap().port();
        let second = free_port();
        let found = bind_first("127.0.0.1", &[first_port, second]).unwrap();
        assert_eq!(found.local_addr().unwrap().port(), second);
    }

    #[test]
    fn port_zero_only_when_every_port_is_busy() {
        let held: Vec<TcpListener> = (0..3).map(|_| busy()).collect();
        let ports: Vec<u16> = held
            .iter()
            .map(|listener| listener.local_addr().unwrap().port())
            .collect();
        let found = bind_first("127.0.0.1", &ports).unwrap();
        let port = found.local_addr().unwrap().port();
        assert!(!ports.contains(&port));
    }

    #[test]
    fn a_free_first_port_is_taken() {
        let port = free_port();
        let found = bind_first("127.0.0.1", &[port, free_port()]).unwrap();
        assert_eq!(found.local_addr().unwrap().port(), port);
    }
}
