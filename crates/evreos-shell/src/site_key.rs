//! Canonical site identity for permissions and blocking exceptions.
//!
//! # Architecture and Invariants
//!
//! - **Founder Decision 0007 (`decisions/0007`)**:
//!   The site key is the canonical **registrable domain** (eTLD+1) for domain
//!   names, falling back to the canonical host for IP addresses and single-label
//!   hostnames.
//! - **Bank Subdomain Invariant (Edge Cases)**:
//!   Subdomains share the same [`SiteKey`] (e.g. `login.bank.invalid` and
//!   `bank.invalid` produce identical site keys), preventing browser abandonment
//!   caused by broken login redirection flows.
//! - **Strict Privacy (FR-007a, Invariant A)**:
//!   Site keys are strictly local state. They are never transmitted off the
//!   machine or retained on any server.

#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// Error encountered when parsing or validating a [`SiteKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SiteKeyError {
    /// The input address or host string was empty.
    Empty,
    /// The address or URL could not be parsed.
    InvalidAddress(String),
    /// The host contains invalid or forbidden characters.
    InvalidHost(String),
}

impl fmt::Display for SiteKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "site key input cannot be empty"),
            Self::InvalidAddress(msg) => write!(f, "invalid address: {msg}"),
            Self::InvalidHost(msg) => write!(f, "invalid host: {msg}"),
        }
    }
}

impl std::error::Error for SiteKeyError {}

/// Canonical site key identifying a website for permissions and blocking exceptions.
///
/// Per Decision 0007, a `SiteKey` is:
/// - The **registrable domain** (eTLD+1) for domain names, in lowercase.
/// - The canonical host for IP addresses (IPv4 or IPv6) and single-label hosts (`localhost`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteKey(String);

impl SiteKey {
    /// Create a `SiteKey` directly from an already normalized string.
    ///
    /// Validates that the key is non-empty and contains no whitespace or control characters.
    pub fn new(key: impl Into<String>) -> Result<Self, SiteKeyError> {
        let s = key.into();
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(SiteKeyError::Empty);
        }
        if trimmed.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(SiteKeyError::InvalidHost(trimmed.to_owned()));
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    /// Parse a `SiteKey` from a full URL or address string.
    ///
    /// Strips scheme, credentials, port, path, query, and fragment, then resolves
    /// the host to its canonical registrable domain or host representation per Decision 0007.
    pub fn from_url(url: &str) -> Result<Self, SiteKeyError> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(SiteKeyError::Empty);
        }
        let host = extract_host_from_url(trimmed)?;
        Self::from_host(host)
    }

    /// Parse a `SiteKey` from a host or domain name string.
    ///
    /// Resolves domain names to their registrable domain (eTLD+1), lowercase,
    /// and preserves canonical IP literals and single-label hostnames.
    pub fn from_host(host: &str) -> Result<Self, SiteKeyError> {
        let trimmed = host.trim();
        if trimmed.is_empty() {
            return Err(SiteKeyError::Empty);
        }
        // Strip trailing dot if present (FQDN root label)
        let stripped = trimmed.strip_suffix('.').unwrap_or(trimmed);
        if stripped.is_empty() {
            return Err(SiteKeyError::Empty);
        }
        let lower = stripped.to_ascii_lowercase();

        // 1. Check for IPv6 literal: [::1]
        if is_ipv6_literal(&lower) {
            return Ok(Self(lower));
        }

        // 2. Check for IPv4 literal: 127.0.0.1
        if is_ipv4(&lower) {
            return Ok(Self(lower));
        }

        // 3. Validate hostname characters
        if lower.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(SiteKeyError::InvalidHost(lower));
        }
        for label in lower.split('.') {
            if label.is_empty() {
                return Err(SiteKeyError::InvalidHost(format!(
                    "empty label in host: {lower}"
                )));
            }
            if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err(SiteKeyError::InvalidHost(format!(
                    "invalid character in host label: {label}"
                )));
            }
        }

        // 4. Resolve registrable domain (eTLD+1) or single-label
        let reg_domain = resolve_registrable_domain(&lower);
        Ok(Self(reg_domain.to_string()))
    }

    /// Parse a `SiteKey` from either a URL or raw host string.
    pub fn parse(input: &str) -> Result<Self, SiteKeyError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(SiteKeyError::Empty);
        }
        if trimmed.contains("://") {
            Self::from_url(trimmed)
        } else {
            Self::from_host(trimmed)
        }
    }

    /// Returns the canonical string representation of the site key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the site key and return the underlying [`String`].
    pub fn into_string(self) -> String {
        self.0
    }

    /// Check if this `SiteKey` matches the given host.
    pub fn matches_host(&self, host: &str) -> bool {
        Self::from_host(host).map(|k| k == *self).unwrap_or(false)
    }

    /// Check if this `SiteKey` matches the given URL address.
    pub fn matches_url(&self, url: &str) -> bool {
        Self::from_url(url).map(|k| k == *self).unwrap_or(false)
    }
}

impl fmt::Display for SiteKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for SiteKey {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SiteKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SiteKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl FromStr for SiteKey {
    type Err = SiteKeyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

fn extract_host_from_url(url: &str) -> Result<&str, SiteKeyError> {
    let without_scheme = if let Some((_scheme, rest)) = url.split_once("://") {
        rest
    } else {
        url
    };

    // Take authority before path, query, or fragment
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme);

    if authority.is_empty() {
        return Err(SiteKeyError::InvalidAddress(format!(
            "missing authority in URL: {url}"
        )));
    }

    // Strip credentials (user:pass@)
    let host_and_port = if let Some((_, host_part)) = authority.rsplit_once('@') {
        host_part
    } else {
        authority
    };

    if host_and_port.is_empty() {
        return Err(SiteKeyError::InvalidAddress(format!(
            "empty host in URL: {url}"
        )));
    }

    // Handle IPv6 bracketed host: [::1]:8080
    if let Some(bracket_end) = host_and_port.find(']') {
        if host_and_port.starts_with('[') {
            return Ok(&host_and_port[..=bracket_end]);
        }
    }

    // Strip port (:8080)
    let host = if let Some((host_part, _port)) = host_and_port.split_once(':') {
        host_part
    } else {
        host_and_port
    };

    if host.is_empty() {
        return Err(SiteKeyError::InvalidAddress(format!(
            "empty host in URL: {url}"
        )));
    }

    Ok(host)
}

fn is_ipv6_literal(s: &str) -> bool {
    if s.starts_with('[') && s.ends_with(']') && s.len() > 2 {
        let inner = &s[1..s.len() - 1];
        inner
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
    } else {
        false
    }
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.is_empty() || part.len() > 3 {
            return false;
        }
        match part.parse::<u8>() {
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    true
}

fn resolve_registrable_domain(host: &str) -> &str {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 2 {
        // Single-label (localhost) or 2-label (bank.invalid, example.com)
        return host;
    }

    // Check if the last two labels form a known multi-part public suffix
    if is_two_part_public_suffix(&labels) {
        // e.g. ["login", "bank", "co", "uk"] -> take last 3: "bank.co.uk"
        if labels.len() >= 3 {
            let skip_count = labels.len() - 3;
            let byte_offset = labels[..skip_count]
                .iter()
                .map(|l| l.len() + 1)
                .sum::<usize>();
            return &host[byte_offset..];
        }
    }

    // Default single-label public suffix: take last 2 labels (e.g. "bank.invalid")
    let skip_count = labels.len() - 2;
    let byte_offset = labels[..skip_count]
        .iter()
        .map(|l| l.len() + 1)
        .sum::<usize>();
    &host[byte_offset..]
}

fn is_two_part_public_suffix(labels: &[&str]) -> bool {
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    let sld = labels[labels.len() - 2];

    // Must be a 2-letter country-code TLD
    if tld.len() != 2 || !tld.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }

    // Common second-level public suffix prefixes
    matches!(
        sld,
        "co" | "com"
            | "org"
            | "net"
            | "gov"
            | "govt"
            | "edu"
            | "ac"
            | "gob"
            | "asso"
            | "nom"
            | "ltd"
            | "plc"
            | "me"
            | "or"
            | "ne"
            | "go"
            | "mil"
            | "gen"
            | "firm"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_login_subdomain_resolves_to_registrable_domain() {
        let key1 = SiteKey::from_url("https://login.bank.invalid/auth/login").unwrap();
        let key2 = SiteKey::from_url("https://bank.invalid/").unwrap();
        let key3 = SiteKey::from_url("https://www.bank.invalid:8443/portal").unwrap();

        assert_eq!(key1.as_str(), "bank.invalid");
        assert_eq!(key1, key2);
        assert_eq!(key2, key3);
        assert!(key1.matches_host("auth.login.bank.invalid"));
        assert!(key1.matches_url("https://idp.bank.invalid/token"));
    }

    #[test]
    fn multi_part_public_suffix_resolves_correctly() {
        let key1 = SiteKey::from_url("https://login.mybank.co.uk/signin").unwrap();
        let key2 = SiteKey::from_url("https://mybank.co.uk").unwrap();
        let key3 = SiteKey::from_host("online.banking.mybank.co.uk").unwrap();

        assert_eq!(key1.as_str(), "mybank.co.uk");
        assert_eq!(key1, key2);
        assert_eq!(key2, key3);
    }

    #[test]
    fn ip_addresses_and_localhost() {
        let loopback = SiteKey::from_url("http://localhost:3000/").unwrap();
        assert_eq!(loopback.as_str(), "localhost");

        let ipv4 = SiteKey::from_url("http://127.0.0.1:8080/dashboard").unwrap();
        assert_eq!(ipv4.as_str(), "127.0.0.1");

        let ipv6 = SiteKey::from_url("https://[::1]:8443/").unwrap();
        assert_eq!(ipv6.as_str(), "[::1]");
    }

    #[test]
    fn normalization_strips_trailing_dots_and_lowercases() {
        let key = SiteKey::from_url("HTTPS://WWW.BANK.INVALID.:443/").unwrap();
        assert_eq!(key.as_str(), "bank.invalid");
    }

    #[test]
    fn invalid_hosts_error() {
        assert!(SiteKey::from_host("").is_err());
        assert!(SiteKey::from_host("   ").is_err());
        assert!(SiteKey::from_host("bank..invalid").is_err());
        assert!(SiteKey::from_host("bank space.invalid").is_err());
    }
}
