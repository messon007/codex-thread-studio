//! Bounded, credential-free HTTP(S) image reads on the Rust I/O lane.
//!
//! Resolve and pin every hop before connecting. Model/document URLs never gain
//! access to loopback, LAN services, proxies, browser cookies or gateway tokens.
use futures_util::StreamExt;
use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use url::Url;

const MAX_BYTES: usize = 10 * 1024 * 1024;
const CACHE_BYTES: usize = 32 * 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(10);
const TTL: Duration = Duration::from_secs(300);
type Image = Arc<[u8]>;

/// Syntactic projection only; DNS/address checks run immediately before I/O.
pub fn reference(raw: &str) -> Result<String, String> {
    Ok(validate(raw)?.into())
}
fn validate(raw: &str) -> Result<Url, String> {
    if raw.is_empty() || raw.len() > 8192 || raw.chars().any(char::is_control) {
        return Err("Invalid remote image URL".into());
    }
    let mut url = Url::parse(raw).map_err(|_| "Invalid remote image URL")?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Remote images require a credential-free HTTP(S) URL".into());
    }
    match url.host() {
        Some(url::Host::Domain(host))
            if host.ends_with('.')
                || !host.contains('.')
                || host.ends_with(".localhost")
                || host.ends_with(".local")
                || host.ends_with(".internal")
                || host.ends_with(".invalid")
                || host.ends_with(".test")
                || host.ends_with(".example")
                || host == "home.arpa"
                || host.ends_with(".home.arpa") =>
        {
            return Err("Remote images require a public network address".into())
        }
        Some(url::Host::Ipv4(ip)) if !public(IpAddr::V4(ip)) => {
            return Err("Remote images require a public network address".into())
        }
        Some(url::Host::Ipv6(ip)) if !public(IpAddr::V6(ip)) => {
            return Err("Remote images require a public network address".into())
        }
        _ => {}
    }
    url.set_fragment(None);
    Ok(url)
}
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            // Restrict to ordinary global unicast. This also rejects mapped IPv4,
            // NAT64, link/site-local, multicast, 6to4 and Teredo translation paths.
            let s = ip.segments();
            s[0] & 0xe000 == 0x2000
                && s[0] != 0x2002
                && !(s[0] == 0x2001 && (s[1] < 0x0200 || s[1] == 0x0db8))
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}
fn addresses(rows: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, String> {
    if rows.is_empty() || rows.len() > 32 || rows.iter().any(|a| !public(a.ip())) {
        return Err("Remote images require a public network address".into());
    }
    Ok(rows)
}
async fn client(url: &Url) -> Result<reqwest::Client, String> {
    let host = url.host_str().ok_or("Invalid remote image host")?;
    let port = url
        .port_or_known_default()
        .ok_or("Invalid remote image port")?;
    let resolved = match url.host() {
        Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(ip.into(), port)],
        Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(ip.into(), port)],
        _ => tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| "Unable to resolve remote image host")?
            .take(33)
            .collect(),
    };
    let resolved = addresses(resolved)?;
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &resolved)
        .connect_timeout(Duration::from_secs(3))
        .read_timeout(Duration::from_secs(3))
        .timeout(DEADLINE)
        .build()
        .map_err(|_| "Unable to initialize remote image connection".into())
}
async fn fetch_with<F, Fut>(mut url: Url, connect: F, limit: usize) -> Result<Vec<u8>, String>
where
    F: Fn(Url) -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Client, String>>,
{
    for hop in 0..=4 {
        let response = connect(url.clone())
            .await?
            .get(url.clone())
            .header(
                reqwest::header::ACCEPT,
                "image/png,image/jpeg,image/gif,image/webp,image/svg+xml",
            )
            .send()
            .await
            .map_err(|_| "Remote image connection failed")?;
        if response.status().is_redirection() {
            if hop == 4 {
                return Err("Remote image exceeded four redirects".into());
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .filter(|s| s.len() <= 8192)
                .ok_or("Invalid remote image redirect")?;
            let next = url
                .join(location)
                .map_err(|_| "Invalid remote image redirect")?;
            let next = validate(next.as_str())?;
            if url.scheme() == "https" && next.scheme() != "https" {
                return Err("Remote image redirect would downgrade HTTPS".into());
            }
            url = next;
            continue;
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(format!(
                "Remote image returned HTTP {}",
                response.status().as_u16()
            ));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err("Remote image exceeds 10 MiB".into());
        }
        // No compression feature is enabled, so compressed transfer bodies never
        // bypass decoded-size checks. Image codecs apply their own pixel limits.
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "Remote image transfer failed")?;
            if chunk.len() > limit.saturating_sub(bytes.len()) {
                return Err("Remote image exceeds 10 MiB".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err("Remote image is empty".into());
        }
        return Ok(bytes);
    }
    unreachable!()
}
#[derive(Default)]
struct Cache {
    entries: std::collections::VecDeque<(String, Instant, Image)>,
    bytes: usize,
}
impl Cache {
    fn get(&mut self, url: &str) -> Option<Image> {
        self.entries.retain(|(_, at, _)| at.elapsed() < TTL);
        self.bytes = self.entries.iter().map(|(_, _, i)| i.len()).sum();
        let index = self.entries.iter().position(|(key, _, _)| key == url)?;
        let entry = self.entries.remove(index)?;
        let image = entry.2.clone();
        self.entries.push_back(entry);
        Some(image)
    }
    fn put(&mut self, url: String, image: Image) {
        if let Some(index) = self.entries.iter().position(|(key, _, _)| key == &url) {
            if let Some((_, _, old)) = self.entries.remove(index) {
                self.bytes -= old.len();
            }
        }
        while self.bytes + image.len() > CACHE_BYTES || self.entries.len() >= 32 {
            let Some((_, _, old)) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= old.len();
        }
        self.bytes += image.len();
        self.entries.push_back((url, Instant::now(), image));
    }
}
fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}
/// The UI renderer admits two image jobs. An independent semaphore also bounds
/// callers outside that renderer; the deadline includes DNS, redirects and body.
pub async fn load(raw: &str) -> Result<Image, String> {
    static WORKERS: OnceLock<tokio::sync::Semaphore> = OnceLock::new();
    let url = validate(raw)?;
    let key = url.as_str().to_owned();
    if let Some(image) = cache()
        .lock()
        .map_err(|_| "Remote image cache unavailable")?
        .get(&key)
    {
        return Ok(image);
    }
    let _permit = WORKERS
        .get_or_init(|| tokio::sync::Semaphore::new(2))
        .try_acquire()
        .map_err(|_| "Remote image workers are busy; retry after scrolling")?;
    let bytes = tokio::time::timeout(
        DEADLINE,
        fetch_with(url, |url| async move { client(&url).await }, MAX_BYTES),
    )
    .await
    .map_err(|_| "Remote image timed out")??;
    // MIME is derived by the shared decoder; it does not trust HTTP headers.
    let image: Image = Arc::from(bytes);
    cache()
        .lock()
        .map_err(|_| "Remote image cache unavailable")?
        .put(key, image.clone());
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_addresses_cannot_reach_local_or_translation_networks() {
        for raw in [
            "http://localhost/x",
            "http://service.local/x",
            "http://user:pass@example.org/x",
            "file:///etc/passwd",
            "http://127.1/x",
            "http://2130706433/x",
            "http://[::ffff:127.0.0.1]/x",
            "http://[64:ff9b::7f00:1]/x",
            "http://169.254.169.254/x",
            "http://100.64.0.1/x",
            "http://[2002:7f00:1::]/x",
            "http://[2001::1]/x",
            "http://[3fff::1]/x",
        ] {
            assert!(validate(raw).is_err(), "{raw}");
        }
        assert!(validate("https://example.org/image.svg#fragment")
            .unwrap()
            .fragment()
            .is_none());
        assert!(public("1.1.1.1".parse().unwrap()));
        assert!(public("2606:4700:4700::1111".parse().unwrap()));
        assert!(addresses(vec![
            "1.1.1.1:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap()
        ])
        .is_err());
        assert!(addresses(vec![]).is_err());
        assert!(addresses(vec!["1.1.1.1:443".parse().unwrap(); 33]).is_err());
    }
    fn server(responses: Vec<&'static str>) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let thread = std::thread::spawn(move || {
            for response in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut data = [0u8; 4096];
                let n = socket.read(&mut data).unwrap();
                let request = String::from_utf8_lossy(&data[..n]).to_lowercase();
                assert!(
                    !request.contains("authorization:")
                        && !request.contains("cookie:")
                        && !request.contains("referer:")
                );
                let _ = socket.write_all(response.as_bytes());
            }
        });
        (addr, thread)
    }
    async fn fixture(responses: Vec<&'static str>, limit: usize) -> Result<Vec<u8>, String> {
        let (addr, thread) = server(responses);
        // Test transport only: public-address validation is independently checked
        // above. Production pins admitted DNS results, never this fixture address.
        let result = fetch_with(
            Url::parse("http://images.example.org/a").unwrap(),
            move |_| async move {
                reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .resolve("images.example.org", addr)
                    .build()
                    .map_err(|e| e.to_string())
            },
            limit,
        )
        .await;
        thread.join().unwrap();
        result
    }
    #[tokio::test]
    async fn transfer_limits_and_redirects_are_enforced_without_credentials() {
        let data=fixture(vec!["HTTP/1.1 302 Found\r\nLocation: /b\r\nContent-Length: 0\r\nConnection: close\r\n\r\n","HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc"],8).await.unwrap();
        assert_eq!(data, b"abc");
        for response in ["HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n", "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n", "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1/private\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"] {
            assert!(fixture(vec![response],8).await.is_err());
        }
    }
    #[tokio::test]
    #[ignore = "Explicit public HTTPS fixture; ordinary tests never access the internet"]
    async fn public_https_image_uses_production_dns_and_tls() {
        use sha2::Digest;
        let url = std::env::var("STUDIO_GNOME_REMOTE_IMAGE_TEST_URL")
            .expect("explicit public fixture URL");
        let expected = std::env::var("STUDIO_GNOME_REMOTE_IMAGE_TEST_SHA256")
            .expect("expected fixture digest");
        let bytes = load(&url).await.unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        let actual = sha2::Sha256::digest(bytes.as_ref())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
        assert!(Arc::ptr_eq(&bytes, &load(&url).await.unwrap()));
    }
}
