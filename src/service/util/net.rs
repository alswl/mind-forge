//! Network classification for `--offline`.
//!
//! Loopback access is not "network access": under `--offline`, loopback
//! endpoints are always permitted and only external hosts are blocked. Every
//! network call site routes its offline gate through these predicates so the
//! rule can never diverge again (spec 069 #27).

/// Returns true when `host` is a literal loopback address or `localhost`.
///
/// Classification is by the literal host string only — no DNS resolution — so a
/// public hostname that happens to resolve to `127.0.0.1` is still treated as
/// external. `std::net::IpAddr::is_loopback` covers `127.0.0.0/8` and `::1`.
pub fn is_loopback_host(host: &str) -> bool {
    let host = host.trim();
    // Strip optional IPv6 brackets, e.g. "[::1]".
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    matches!(host.parse::<std::net::IpAddr>(), Ok(ip) if ip.is_loopback())
}

/// Returns true when `url` targets an external (non-loopback) host — the case
/// `--offline` must block. A URL that cannot be parsed, or that has no host, is
/// treated as external (fail closed).
pub fn is_external_url(url: &str) -> bool {
    match reqwest::Url::parse(url) {
        Ok(parsed) => match parsed.host_str() {
            Some(host) => !is_loopback_host(host),
            None => true,
        },
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts_are_recognized() {
        for host in
            ["127.0.0.1", "127.0.0.5", "127.255.255.254", "::1", "[::1]", "localhost", "LocalHost", " localhost "]
        {
            assert!(is_loopback_host(host), "expected loopback: {host:?}");
        }
    }

    #[test]
    fn external_hosts_are_not_loopback() {
        for host in ["example.com", "8.8.8.8", "192.168.1.10", "10.0.0.1", "::2", "", "not a host"] {
            assert!(!is_loopback_host(host), "expected external: {host:?}");
        }
    }

    #[test]
    fn loopback_urls_are_internal() {
        for url in [
            "http://localhost:11434/v1/embeddings",
            "http://127.0.0.1:11434/v1/embeddings",
            "https://127.0.0.1/x",
            "http://[::1]:8080/",
        ] {
            assert!(!is_external_url(url), "expected internal: {url}");
        }
    }

    #[test]
    fn external_and_malformed_urls_are_external() {
        for url in ["https://api.example.com/v1/embeddings", "http://8.8.8.8/", "not-a-url", "mailto:x@y.z"] {
            assert!(is_external_url(url), "expected external: {url}");
        }
    }
}
