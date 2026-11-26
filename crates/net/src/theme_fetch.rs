//! Fetch the text of a custom theme from an https URL (themes CS2, CS3).
//!
//! The URL comes from the user, so the fetch is a small SSRF-safe client: https
//! only on the input and on every hop, at most three redirects, a host that is
//! or resolves to a loopback, private or link-local address is refused, the
//! whole exchange has a 10 s deadline and the body is cut off at 64 KiB while it
//! streams. No cookies, no credentials, no proxy, one fixed `User-Agent`, TLS
//! verification on. Errors never carry the URL: it can hold a token in its query.
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use mwt_base::{Error, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION};
use reqwest::redirect::Policy;
use reqwest::{Client, Url};

/// The largest body that is read.
const BODY_CAP: usize = 64 * 1024;
/// The deadline for the whole exchange, redirects and body included.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Redirects followed before the fetch fails.
const MAX_REDIRECTS: usize = 3;
const USER_AGENT: &str = "multi-wallet-tracker-theme-fetch";

/// The text of a fetched theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeCssFetch {
    /// The body, valid UTF-8, at most 64 KiB.
    pub text: String,
    /// The server did not call the body `text/css` or `text/plain`; the caller
    /// shows a warning, the text is still usable.
    pub content_type_warning: bool,
}

/// Fetch the CSS text behind an `https://` URL, once, with no side effects.
pub async fn fetch_theme_css(url: &str) -> Result<ThemeCssFetch> {
    fetch(url, &Limits::PRODUCTION).await
}

/// What a fetch is allowed; tests loosen it to reach a local server.
struct Limits {
    timeout: Duration,
    /// An origin exempt from the scheme and host checks (a local test server).
    #[cfg(test)]
    trusted: Option<(String, u16)>,
}

impl Limits {
    const PRODUCTION: Self = Self {
        timeout: TIMEOUT,
        #[cfg(test)]
        trusted: None,
    };

    #[cfg(test)]
    fn is_trusted(&self, url: &Url) -> bool {
        self.trusted.as_ref().is_some_and(|(host, port)| {
            url.host_str() == Some(host) && url.port_or_known_default() == Some(*port)
        })
    }

    #[cfg(not(test))]
    fn is_trusted(&self, _url: &Url) -> bool {
        false
    }
}

fn refused(reason: &str) -> Error {
    Error::InvalidInput(format!("theme URL refused: {reason}"))
}

fn network(reason: &str) -> Error {
    Error::Network(format!("theme fetch failed: {reason}"))
}

async fn fetch(url: &str, limits: &Limits) -> Result<ThemeCssFetch> {
    let first = Url::parse(url).map_err(|_| refused("not a valid URL"))?;
    tokio::time::timeout(limits.timeout, follow(first, limits))
        .await
        .map_err(|_| network("timed out"))?
}

/// Walk the hops; every one is checked and resolved before a byte is sent.
async fn follow(first: Url, limits: &Limits) -> Result<ThemeCssFetch> {
    let mut current = first;
    for hop in 0..=MAX_REDIRECTS {
        let client = client_for(&current, limits).await?;
        let mut response = client
            .get(current.clone())
            .header(ACCEPT, "text/css, text/plain;q=0.9")
            .send()
            .await
            .map_err(|_| network("request failed"))?;
        let status = response.status();
        if status.is_redirection() {
            if hop == MAX_REDIRECTS {
                return Err(refused("too many redirects"));
            }
            let target = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| network("redirect without a location"))?;
            current = current
                .join(target)
                .map_err(|_| refused("bad redirect target"))?;
            continue;
        }
        if !status.is_success() {
            return Err(network(&format!("server answered {}", status.as_u16())));
        }
        let content_type_warning = !is_css_or_text(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
        );
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| network("body read failed"))?
        {
            if body.len() + chunk.len() > BODY_CAP {
                return Err(refused("the file is over 64 KiB"));
            }
            body.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(body).map_err(|_| refused("the file is not UTF-8 text"))?;
        return Ok(ThemeCssFetch {
            text,
            content_type_warning,
        });
    }
    Err(refused("too many redirects"))
}

fn is_css_or_text(content_type: Option<&str>) -> bool {
    let essence = content_type
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    matches!(essence.as_deref(), Some("text/css" | "text/plain"))
}

/// Check one hop and build a client that can only connect to the addresses
/// that were checked (no second DNS answer between check and connect).
async fn client_for(url: &Url, limits: &Limits) -> Result<Client> {
    let builder = Client::builder()
        .redirect(Policy::none())
        .no_proxy()
        .user_agent(USER_AGENT)
        .referer(false);
    let build = |builder: reqwest::ClientBuilder| {
        builder.build().map_err(|_| network("client setup failed"))
    };
    if limits.is_trusted(url) {
        return build(builder);
    }
    if url.scheme() != "https" {
        return Err(refused("only https is fetched"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(refused("credentials in the URL"));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let Some(host) = url.host_str() else {
        return Err(refused("no host"));
    };
    // An address literal is judged as it is; `[::1]` comes with its brackets.
    if let Ok(ip) = host.trim_matches(['[', ']']).parse::<IpAddr>() {
        check_ip(ip)?;
        return build(builder);
    }
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| network("host not found"))?
        .collect();
    if addrs.is_empty() {
        return Err(network("host not found"));
    }
    for addr in &addrs {
        check_ip(addr.ip())?;
    }
    build(builder.resolve_to_addrs(host, &addrs))
}

fn check_ip(ip: IpAddr) -> Result<()> {
    if is_internal(ip) {
        Err(refused("a local or private address"))
    } else {
        Ok(())
    }
}

/// Loopback, private, link-local, shared (CGNAT), unspecified, multicast and the
/// IPv6 unique-local range; an IPv4-mapped IPv6 address is judged as its IPv4.
fn is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_internal_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_internal_v4(v4),
            None => is_internal_v6(v6),
        },
    }
}

fn is_internal_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || (octets[0] == 100 && (64..128).contains(&octets[1]))
}

fn is_internal_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || first & 0xfe00 == 0xfc00
        || first & 0xffc0 == 0xfe80
        || first & 0xffc0 == 0xfec0
        || embedded_v4(ip).is_some_and(is_internal_v4)
}

/// The IPv4 address a transition format carries: NAT64 (64:ff9b::/96),
/// IPv4-compatible (::/96) and 6to4 (2002::/16). The local-use NAT64 prefix
/// (64:ff9b:1::/48) is never public, so it yields an internal address.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let seg = ip.segments();
    let join = |upper: u16, lower: u16| {
        let [first, second] = upper.to_be_bytes();
        let [third, fourth] = lower.to_be_bytes();
        Ipv4Addr::new(first, second, third, fourth)
    };
    match seg {
        [0x64, 0xff9b, 0, 0, 0, 0, high, tail] | [0, 0, 0, 0, 0, 0, high, tail] => {
            Some(join(high, tail))
        }
        [0x64, 0xff9b, 1, ..] => Some(Ipv4Addr::LOCALHOST),
        [0x2002, high, tail, ..] => Some(join(high, tail)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    /// How the stub answers one request: the path decides.
    type Handler = dyn Fn(&str) -> Vec<u8> + Send + Sync;

    /// Serve `handler` on a loopback port; returns the base URL and its limits.
    async fn stub(
        timeout: Duration,
        handler: impl Fn(&str) -> Vec<u8> + Send + Sync + 'static,
    ) -> (String, Limits, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&hits);
        let handler: Arc<Handler> = Arc::new(handler);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let handler = Arc::clone(&handler);
                let counter = Arc::clone(&counter);
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let Ok(read) = stream.read(&mut buf).await else {
                        return;
                    };
                    counter.fetch_add(1, Ordering::SeqCst);
                    let request = String::from_utf8_lossy(&buf[..read]).into_owned();
                    let path = request.split_whitespace().nth(1).unwrap_or("/");
                    let _ = stream.write_all(&handler(path)).await;
                    // Keep the socket open a moment so a slow reply is not cut.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                });
            }
        });
        let limits = Limits {
            timeout,
            trusted: Some(("127.0.0.1".into(), addr.port())),
        };
        (format!("http://{addr}"), limits, hits)
    }

    fn ok(content_type: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn redirect_to(target: &str) -> Vec<u8> {
        format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .into_bytes()
    }

    const SECS: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn a_css_body_is_returned_without_a_warning() {
        let (base, limits, _) = stub(SECS, |_| ok("text/css; charset=utf-8", ":root{}")).await;
        let got = fetch(&format!("{base}/t.css"), &limits).await.unwrap();
        assert_eq!(got.text, ":root{}");
        assert!(!got.content_type_warning);
    }

    #[tokio::test]
    async fn another_content_type_is_a_warning_not_an_error() {
        let (base, limits, _) = stub(SECS, |_| ok("application/json", ":root{}")).await;
        let got = fetch(&format!("{base}/t"), &limits).await.unwrap();
        assert_eq!(got.text, ":root{}");
        assert!(got.content_type_warning);
        let (base, limits, _) = stub(SECS, |_| ok("text/plain", "x")).await;
        assert!(!fetch(&base, &limits).await.unwrap().content_type_warning);
    }

    #[tokio::test]
    async fn plain_http_is_refused_before_any_request() {
        let (base, _, hits) = stub(SECS, |_| ok("text/css", "x")).await;
        // Production limits: nothing is trusted, so the stub must not be reached.
        let err = fetch(&format!("{base}/t.css"), &Limits::PRODUCTION).await;
        assert!(matches!(err, Err(Error::InvalidInput(_))));
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn loopback_private_and_link_local_hosts_are_refused() {
        for url in [
            "https://127.0.0.1/t.css",
            "https://localhost/t.css",
            "https://10.0.0.5/t.css",
            "https://192.168.1.1/t.css",
            "https://172.16.0.1/t.css",
            "https://169.254.169.254/latest",
            "https://100.64.0.1/t.css",
            "https://0.0.0.0/t.css",
            "https://[::1]/t.css",
            "https://[fe80::1]/t.css",
            "https://[fd00::1]/t.css",
            "https://[::ffff:10.0.0.1]/t.css",
            "https://[64:ff9b::a00:1]/t.css",
            "https://[::10.0.0.1]/t.css",
            "https://[2002:0a00:0001::]/t.css",
            "https://[fec0::1]/t.css",
            "https://user:pass@example.com/t.css",
            "ftp://example.com/t.css",
            "not a url",
        ] {
            let err = fetch(url, &Limits::PRODUCTION).await.unwrap_err();
            assert!(matches!(err, Error::InvalidInput(_)), "{url}: {err}");
        }
    }

    #[test]
    fn embedded_ipv4_and_site_local_addresses_are_internal() {
        for ip in [
            "64:ff9b::a00:1",
            "::10.0.0.1",
            "2002:a00:1::",
            "fec0::1",
            "64:ff9b:1::1",
        ] {
            assert!(is_internal(ip.parse().unwrap()), "{ip}");
        }
    }

    #[test]
    fn public_addresses_pass_the_address_check() {
        for ip in [
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700::1111",
            "64:ff9b::808:808",
            "2002:808:808::",
        ] {
            assert!(!is_internal(ip.parse().unwrap()), "{ip}");
        }
    }

    #[tokio::test]
    async fn a_redirect_to_http_fails() {
        let (base, limits, _) = stub(SECS, |_| redirect_to("http://example.com/t.css")).await;
        let err = fetch(&format!("{base}/a"), &limits).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    }

    #[tokio::test]
    async fn a_redirect_to_a_private_host_fails() {
        for target in [
            "https://10.0.0.5/t.css",
            "https://169.254.169.254/x",
            "https://localhost/x",
            "https://[64:ff9b::a00:1]/x",
            "https://[::10.0.0.1]/x",
            "https://[2002:0a00:0001::]/x",
            "https://[fec0::1]/x",
        ] {
            let (base, limits, _) = stub(SECS, move |_| redirect_to(target)).await;
            let err = fetch(&format!("{base}/a"), &limits).await.unwrap_err();
            assert!(matches!(err, Error::InvalidInput(_)), "{target}: {err}");
        }
    }

    #[tokio::test]
    async fn redirects_within_the_limit_are_followed() {
        let (base, limits, _) = stub(SECS, |path| match path {
            "/a" => redirect_to("/b"),
            "/b" => redirect_to("/c"),
            "/c" => redirect_to("/d"),
            _ => ok("text/css", "done"),
        })
        .await;
        assert_eq!(
            fetch(&format!("{base}/a"), &limits).await.unwrap().text,
            "done"
        );
    }

    #[tokio::test]
    async fn a_fourth_redirect_fails() {
        let (base, limits, _) = stub(SECS, |path| match path {
            "/a" => redirect_to("/b"),
            "/b" => redirect_to("/c"),
            "/c" => redirect_to("/d"),
            "/d" => redirect_to("/e"),
            _ => ok("text/css", "done"),
        })
        .await;
        let err = fetch(&format!("{base}/a"), &limits).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    }

    #[tokio::test]
    async fn a_body_over_the_cap_fails_even_without_content_length() {
        let (base, limits, _) = stub(SECS, |_| {
            let mut reply =
                b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nConnection: close\r\n\r\n".to_vec();
            reply.extend(std::iter::repeat_n(b'a', BODY_CAP + 1));
            reply
        })
        .await;
        let err = fetch(&base, &limits).await.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)), "{err}");
    }

    #[tokio::test]
    async fn a_body_at_the_cap_is_read() {
        let body = "a".repeat(BODY_CAP);
        let (base, limits, _) = stub(SECS, move |_| ok("text/css", &body)).await;
        assert_eq!(fetch(&base, &limits).await.unwrap().text.len(), BODY_CAP);
    }

    #[tokio::test]
    async fn a_slow_body_times_out() {
        let (base, limits, _) = stub(Duration::from_millis(300), |_| {
            // The head promises a body that never comes.
            b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 100\r\n\r\nabc".to_vec()
        })
        .await;
        let err = fetch(&base, &limits).await.unwrap_err();
        assert!(matches!(err, Error::Network(_)), "{err}");
    }

    #[tokio::test]
    async fn a_server_error_and_bad_utf8_fail() {
        let (base, limits, _) = stub(SECS, |_| {
            b"HTTP/1.1 500 Oops\r\nContent-Length: 0\r\n\r\n".to_vec()
        })
        .await;
        assert!(matches!(
            fetch(&base, &limits).await,
            Err(Error::Network(_))
        ));
        let (base, limits, _) = stub(SECS, |_| {
            let mut reply =
                b"HTTP/1.1 200 OK\r\nContent-Type: text/css\r\nContent-Length: 2\r\n\r\n".to_vec();
            reply.extend([0xff, 0xfe]);
            reply
        })
        .await;
        assert!(matches!(
            fetch(&base, &limits).await,
            Err(Error::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn errors_never_carry_the_url() {
        let err = fetch_theme_css("https://10.0.0.5/t.css?token=secret")
            .await
            .unwrap_err();
        assert!(!err.to_string().contains("secret"));
    }
}
