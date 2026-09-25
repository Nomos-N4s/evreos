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

    /// Returns the canonical string representation of the site key.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the site key and return the underlying [`String`].
    pub fn into_string(self) -> String {
        self.0
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
        Self::new(s)
    }
}
