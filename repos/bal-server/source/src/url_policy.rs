//! Canonical executor URL policy for v3 WeTime pings.
//!
//! The v3 signature covers the exact URL string, so the service and the pusher
//! must agree on a single canonical form. The canonical form is an origin:
//! `scheme://host` or `scheme://host:port`, with a lowercase ASCII host, no
//! trailing dot, no userinfo, no path, no query, and no fragment.
//!
//! IDN policy: raw Unicode is rejected. Operators must publish the ASCII
//! `xn--` A-label form. This avoids a Unicode form and its punycode form both
//! being accepted as different executors.

use std::fmt;
use std::net::IpAddr;

use url::{Host, Url};

/// Minimum and maximum accepted URL length, matching the legacy validator.
const MIN_URL_LEN: usize = 10;
const MAX_URL_LEN: usize = 256;
/// Maximum DNS hostname length.
const MAX_HOST_LEN: usize = 253;
/// Maximum DNS label length.
const MAX_LABEL_LEN: usize = 63;

/// Reasons a URL cannot be used as a canonical v3 executor URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UrlPolicyError {
    /// The URL is empty after trimming.
    Empty,
    /// The URL is shorter or longer than the accepted bounds.
    Length,
    /// The URL contains non-ASCII bytes.
    NonAscii,
    /// The URL cannot be parsed.
    Invalid,
    /// The scheme is not `http` or `https`.
    UnsupportedScheme,
    /// The URL carries a username or password.
    UserInfo,
    /// The URL carries a query string.
    Query,
    /// The URL carries a fragment.
    Fragment,
    /// The URL carries a path other than `/`.
    Path,
    /// The URL contains an explicit default port.
    DefaultPort,
    /// The URL has no host.
    MissingHost,
    /// The URL uses an IP literal instead of a hostname.
    IpLiteral,
    /// The URL points at localhost.
    Localhost,
    /// The hostname is not a public DNS name.
    NonPublicHost,
    /// The hostname is not a valid ASCII DNS name.
    InvalidHost,
}

impl fmt::Display for UrlPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "url is empty",
            Self::Length => "url length is out of range",
            Self::NonAscii => "url must be ASCII; use the punycode xn-- form",
            Self::Invalid => "url cannot be parsed",
            Self::UnsupportedScheme => "url scheme must be http or https",
            Self::UserInfo => "url must not contain a username or password",
            Self::Query => "url must not contain a query string",
            Self::Fragment => "url must not contain a fragment",
            Self::Path => "url must not contain a path",
            Self::DefaultPort => "url must not contain an explicit default port",
            Self::MissingHost => "url must contain a host",
            Self::IpLiteral => "url must use a hostname, not an IP address",
            Self::Localhost => "url must not use localhost",
            Self::NonPublicHost => "url host must be a public DNS name",
            Self::InvalidHost => "url host is not a valid ASCII DNS name",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for UrlPolicyError {}

/// Return the canonical origin form of `raw`, or a policy error.
///
/// The output is safe to sign and to look up in `willexecutors.host`.
pub fn canonical_executor_url(raw: &str) -> Result<String, UrlPolicyError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(UrlPolicyError::Empty);
    }
    if !(MIN_URL_LEN..=MAX_URL_LEN).contains(&raw.len()) {
        return Err(UrlPolicyError::Length);
    }
    if !raw.is_ascii() {
        return Err(UrlPolicyError::NonAscii);
    }

    let parsed = Url::parse(raw).map_err(|_| UrlPolicyError::Invalid)?;
    let scheme = match parsed.scheme() {
        "http" | "https" => parsed.scheme(),
        _ => return Err(UrlPolicyError::UnsupportedScheme),
    };
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(UrlPolicyError::UserInfo);
    }
    if parsed.query().is_some() {
        return Err(UrlPolicyError::Query);
    }
    if parsed.fragment().is_some() {
        return Err(UrlPolicyError::Fragment);
    }
    let path = parsed.path();
    if !(path.is_empty() || path == "/") {
        return Err(UrlPolicyError::Path);
    }
    if has_explicit_default_port(raw, scheme) {
        return Err(UrlPolicyError::DefaultPort);
    }

    let host = match parsed.host() {
        Some(Host::Domain(domain)) => domain,
        Some(Host::Ipv4(_) | Host::Ipv6(_)) => return Err(UrlPolicyError::IpLiteral),
        None => return Err(UrlPolicyError::MissingHost),
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    validate_dns_host(&host)?;
    if host == "localhost" || host.ends_with(".localhost") {
        return Err(UrlPolicyError::Localhost);
    }
    // Onion addresses are a separate v3 identity path. All other v3 hosts
    // must be public DNS names so the service never queries internal zones.
    if !host.ends_with(".onion") && !is_public_dns_name(&host) {
        return Err(UrlPolicyError::NonPublicHost);
    }
    // Guard against a parser that returns an IP-like domain.
    if host.parse::<IpAddr>().is_ok() {
        return Err(UrlPolicyError::IpLiteral);
    }

    let mut canonical = format!("{scheme}://{host}");
    if let Some(port) = parsed.port() {
        canonical.push(':');
        canonical.push_str(&port.to_string());
    }
    Ok(canonical)
}

/// Validate an ASCII DNS hostname using LDH labels.
fn validate_dns_host(host: &str) -> Result<(), UrlPolicyError> {
    if host.is_empty() || host.len() > MAX_HOST_LEN {
        return Err(UrlPolicyError::InvalidHost);
    }

    for label in host.split('.') {
        if label.is_empty() || label.len() > MAX_LABEL_LEN {
            return Err(UrlPolicyError::InvalidHost);
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err(UrlPolicyError::InvalidHost);
        }
        if !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(UrlPolicyError::InvalidHost);
        }
    }

    Ok(())
}

/// Return whether `host` is a public DNS name.
///
/// This is intentionally a small, deterministic policy rather than a full
/// public-suffix lookup: the name must have at least two labels and must not
/// end in a known private or special-use suffix. IP literals and `localhost`
/// are rejected by the canonical URL checks before this function is reached.
pub fn is_public_dns_name(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if !host.contains('.') {
        return false;
    }

    const NON_PUBLIC_SUFFIXES: &[&str] = &[
        ".local",
        ".localhost",
        ".localdomain",
        ".internal",
        ".intranet",
        ".lan",
        ".home",
        ".corp",
        ".arpa",
        ".test",
        ".invalid",
        ".example",
        ".alt",
    ];
    !NON_PUBLIC_SUFFIXES
        .iter()
        .any(|suffix| host.ends_with(suffix))
}

/// Return the authority portion of a raw URL (`host[:port]` without userinfo).
fn raw_authority(raw: &str) -> Option<&str> {
    let scheme_end = raw.find("://")?;
    let rest = &raw[scheme_end + 3..];
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Detect an explicit default port that `Url` may normalize away.
fn has_explicit_default_port(raw: &str, scheme: &str) -> bool {
    let default_port = if scheme == "http" { 80 } else { 443 };
    let Some(authority) = raw_authority(raw) else {
        return false;
    };
    // Userinfo is rejected separately, but strip it defensively.
    let host_port = authority.rsplit('@').next().unwrap_or(authority);

    if host_port.starts_with('[') {
        let Some(close) = host_port.find(']') else {
            return false;
        };
        return host_port[close + 1..]
            .strip_prefix(':')
            .and_then(|port| port.parse::<u16>().ok())
            == Some(default_port);
    }

    host_port
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse::<u16>().ok())
        == Some(default_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_origins() {
        assert_eq!(
            canonical_executor_url("https://we.example.com").unwrap(),
            "https://we.example.com"
        );
        assert_eq!(
            canonical_executor_url("  http://we.example.com/  ").unwrap(),
            "http://we.example.com"
        );
        assert_eq!(
            canonical_executor_url("https://we.example.com:8443").unwrap(),
            "https://we.example.com:8443"
        );
        assert_eq!(
            canonical_executor_url("https://xn--mnchen-3ya.de").unwrap(),
            "https://xn--mnchen-3ya.de"
        );
        assert_eq!(
            canonical_executor_url("https://we.example.com.").unwrap(),
            "https://we.example.com"
        );
        // Onion addresses are a separate v3 identity path.
        assert_eq!(
            canonical_executor_url("http://abcdefghijklmnop.onion").unwrap(),
            "http://abcdefghijklmnop.onion"
        );
    }

    #[test]
    fn returns_canonical_form_and_rejects_default_ports() {
        assert_eq!(
            canonical_executor_url("https://We.Example.com").unwrap(),
            "https://we.example.com"
        );
        assert_eq!(
            canonical_executor_url("https://we.example.com/").unwrap(),
            "https://we.example.com"
        );
        assert_eq!(
            canonical_executor_url("https://example.com:443"),
            Err(UrlPolicyError::DefaultPort)
        );
        assert_eq!(
            canonical_executor_url("http://example.com:80"),
            Err(UrlPolicyError::DefaultPort)
        );
    }

    #[test]
    fn rejects_invalid_urls() {
        assert_eq!(
            canonical_executor_url("ftp://example.com"),
            Err(UrlPolicyError::UnsupportedScheme)
        );
        assert_eq!(
            canonical_executor_url("https://user:pass@example.com"),
            Err(UrlPolicyError::UserInfo)
        );
        assert_eq!(
            canonical_executor_url("https://example.com/path"),
            Err(UrlPolicyError::Path)
        );
        assert_eq!(
            canonical_executor_url("https://example.com/?x=1"),
            Err(UrlPolicyError::Query)
        );
        assert_eq!(
            canonical_executor_url("https://example.com/#frag"),
            Err(UrlPolicyError::Fragment)
        );
        assert_eq!(
            canonical_executor_url("https://127.0.0.1"),
            Err(UrlPolicyError::IpLiteral)
        );
        assert_eq!(
            canonical_executor_url("https://[::1]"),
            Err(UrlPolicyError::IpLiteral)
        );
        assert_eq!(
            canonical_executor_url("https://localhost"),
            Err(UrlPolicyError::Localhost)
        );
        assert_eq!(
            canonical_executor_url("https://we"),
            Err(UrlPolicyError::NonPublicHost)
        );
        assert_eq!(
            canonical_executor_url("https://we.internal"),
            Err(UrlPolicyError::NonPublicHost)
        );
        assert_eq!(
            canonical_executor_url("https://we.local"),
            Err(UrlPolicyError::NonPublicHost)
        );
        assert_eq!(
            canonical_executor_url("https://we.example"),
            Err(UrlPolicyError::NonPublicHost)
        );
        assert_eq!(
            canonical_executor_url("https://we.home.arpa"),
            Err(UrlPolicyError::NonPublicHost)
        );
        assert_eq!(
            canonical_executor_url("https://example..com"),
            Err(UrlPolicyError::InvalidHost)
        );
        assert_eq!(
            canonical_executor_url("https://-example.com"),
            Err(UrlPolicyError::InvalidHost)
        );
        assert_eq!(
            canonical_executor_url("https://münchen.example"),
            Err(UrlPolicyError::NonAscii)
        );
    }

    #[test]
    fn public_dns_name_policy() {
        for host in [
            "example.com",
            "we.example.com",
            "a.b.c.de",
            "xn--mnchen-3ya.de",
        ] {
            assert!(is_public_dns_name(host), "{host} must be public");
        }
        for host in [
            "we",
            "localhost",
            "we.local",
            "we.localhost",
            "we.internal",
            "we.intranet",
            "we.lan",
            "we.home",
            "we.corp",
            "we.home.arpa",
            "we.example",
            "we.test",
            "we.invalid",
            "we.alt",
        ] {
            assert!(!is_public_dns_name(host), "{host} must be non-public");
        }
    }
}
