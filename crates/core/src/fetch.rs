//! Outbound HTTP for the importer, hardened against SSRF: every connection (including each
//! redirect hop) resolves through a resolver that refuses private, loopback, link-local,
//! CGNAT, multicast and unspecified addresses. Bodies are size-capped.

use anyhow::{Result, anyhow, bail};
use futures_util::StreamExt;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || o[0] == 0
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // CGNAT 100.64/10
                || (o[0] == 192 && o[1] == 0 && o[2] == 0)
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public_ip(IpAddr::V4(v4));
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local
                || (s[0] & 0xffc0) == 0xfe80) // link local
        }
    }
}

struct SafeResolver;

impl Resolve for SafeResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::task::spawn_blocking(move || (host.as_str(), 0).to_socket_addrs())
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)??
                .filter(|a| public_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err("that address is on a private network and can't be imported from".into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .dns_resolver(Arc::new(SafeResolver))
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() >= 5 {
                return a.error("too many redirects");
            }
            if !matches!(a.url().scheme(), "http" | "https") {
                return a.error("redirect to a non-web address");
            }
            // IP literals skip DNS, so check them here
            if let Some(url::Host::Ipv4(ip)) = a.url().host() {
                if !public_ip(IpAddr::V4(ip)) {
                    return a.error("redirect to a private address");
                }
            }
            if let Some(url::Host::Ipv6(ip)) = a.url().host() {
                if !public_ip(IpAddr::V6(ip)) {
                    return a.error("redirect to a private address");
                }
            }
            a.follow()
        }))
        .user_agent(UA)
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .expect("http client")
});

pub fn check_url(raw: &str) -> Result<url::Url> {
    let u = url::Url::parse(raw.trim()).map_err(|_| anyhow!("that doesn't look like a web address"))?;
    if !matches!(u.scheme(), "http" | "https") {
        bail!("only http and https addresses can be imported");
    }
    match u.host() {
        Some(url::Host::Ipv4(ip)) if !public_ip(IpAddr::V4(ip)) => bail!("that address is on a private network"),
        Some(url::Host::Ipv6(ip)) if !public_ip(IpAddr::V6(ip)) => bail!("that address is on a private network"),
        None => bail!("that address has no host"),
        _ => {}
    }
    if u.username() != "" || u.password().is_some() {
        bail!("addresses with passwords are not supported");
    }
    Ok(u)
}

pub struct Fetched {
    pub url: url::Url,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// GET with browser-like headers and a body cap.
pub async fn get(u: &url::Url, referer: Option<&str>, max: usize, accept: &str) -> Result<Fetched> {
    let mut req = CLIENT
        .get(u.clone())
        .header("Accept", accept)
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("Sec-Fetch-Dest", if accept.starts_with("image") { "image" } else { "document" })
        .header("Sec-Fetch-Mode", if accept.starts_with("image") { "no-cors" } else { "navigate" })
        .header("Sec-Fetch-Site", if referer.is_some() { "cross-site" } else { "none" })
        .header("Upgrade-Insecure-Requests", "1");
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    let res = req.send().await.map_err(|e| anyhow!("could not reach {}: {}", u.host_str().unwrap_or(""), root_cause(&e)))?;
    let status = res.status();
    if status == 401 || status == 403 || status == 429 || status == 503 {
        bail!(
            "{} blocked the automatic download ({status}). In your browser, save the page (File → Save Page As…) and import the saved file, or copy the recipe text and use “Paste text”.",
            u.host_str().unwrap_or("The site")
        );
    }
    if !status.is_success() {
        bail!("{} answered {status}", u.host_str().unwrap_or("the site"));
    }
    let url = res.url().clone();
    let content_type = res.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if res.content_length().is_some_and(|l| l as usize > max) {
        bail!("that file is too large");
    }
    let mut bytes = Vec::new();
    let mut stream = res.bytes_stream();
    while let Some(chunk) = stream.next().await {
        bytes.extend_from_slice(&chunk?);
        if bytes.len() > max {
            bail!("that file is too large");
        }
    }
    Ok(Fetched { url, content_type, bytes })
}

fn root_cause(e: &(dyn std::error::Error + 'static)) -> String {
    let mut cur: &dyn std::error::Error = e;
    while let Some(s) = cur.source() {
        cur = s;
    }
    cur.to_string()
}

/// Image type from magic bytes (never trust the server's content-type).
pub fn image_type(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(("image/jpeg", "jpg"))
    } else if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some(("image/png", "png"))
    } else if b.starts_with(b"GIF8") {
        Some(("image/gif", "gif"))
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if b.len() > 12 && &b[4..8] == b"ftyp" && (&b[8..12] == b"avif" || &b[8..12] == b"avis") {
        Some(("image/avif", "avif"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_addresses_refused() {
        for ip in ["127.0.0.1", "10.1.2.3", "192.168.1.1", "172.20.0.1", "169.254.169.254", "100.64.0.1", "0.0.0.0", "::1", "fe80::1", "fd00::1", "::ffff:192.168.0.1"] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip} treated as public");
        }
        for ip in ["93.184.216.34", "2606:4700::1111"] {
            assert!(public_ip(ip.parse().unwrap()));
        }
        assert!(check_url("http://192.168.1.1/x").is_err());
        assert!(check_url("file:///etc/passwd").is_err());
        assert!(check_url("http://[::1]/").is_err());
        assert!(check_url("https://example.com/recipe").is_ok());
    }

    #[tokio::test]
    async fn hostname_resolving_to_loopback_is_refused() {
        let u = check_url("http://localhost:9/").unwrap();
        assert!(get(&u, None, 1000, "*/*").await.is_err());
    }
}
