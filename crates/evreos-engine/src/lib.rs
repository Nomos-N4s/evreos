//! The rendering seam.
//!
//! Principle III requires rendering to go through an interface **the shell
//! defines as the consumer**, with the system web runtime as the default
//! implementation and a headless implementation kept working from day one, so
//! that the seam is proved by a second implementation rather than asserted.
//!
//! Two consequences shape everything here.
//!
//! The trait is written from what the shell needs, never from what a webview
//! offers. Nothing in this crate names a platform, a runtime, or a vendor, and
//! nothing returns a handle to one. A seam that leaks its default
//! implementation's vocabulary is a seam only until the second implementation
//! arrives, which is the failure this interface exists to prevent.
//!
//! Failure is a value, not a panic. FR-015 requires the browser to distinguish
//! a failed load from a successful one and to name the cause, and records that
//! "treating a failed load as a successful empty page is a defect, as is a
//! loading indicator that never resolves". A trait that can only succeed or
//! panic cannot carry that requirement, so [`Engine::load`] returns a result
//! and [`LoadError`] enumerates the causes FR-015 names.

#![forbid(unsafe_code)]

use core::fmt;

/// What the shell asks an engine to render.
///
/// An address is carried as a string rather than a parsed URL type: parsing and
/// policy belong to the shell, which owns FR-003's combined entry field and
/// FR-007a's rule on what may leave the machine. An engine that parsed
/// addresses would be making those decisions where they cannot be reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    address: String,
}

impl Request {
    /// Build a request for `address`.
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
        }
    }

    /// The address the shell asked for, unmodified.
    pub fn address(&self) -> &str {
        &self.address
    }
}

/// Why a load did not produce a page.
///
/// The four variants are the four failures FR-015 enumerates, and SC-009
/// requires each to be exercised on every supported platform. They are a closed
/// set on purpose: a catch-all would let an implementation report every failure
/// as one indistinguishable cause, which is the state FR-015 exists to forbid.
/// Adding a cause is a change to this enum, visible in review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The address did not resolve.
    Unresolvable { address: String },
    /// The certificate was untrusted, expired, or did not match.
    Certificate { address: String, detail: String },
    /// Something between the shell and the site answered in its place.
    Intercepted { address: String },
    /// The site demanded credentials before serving anything.
    AuthenticationRequired { address: String },
}

impl LoadError {
    /// The address the failure concerns.
    pub fn address(&self) -> &str {
        match self {
            Self::Unresolvable { address }
            | Self::Certificate { address, .. }
            | Self::Intercepted { address }
            | Self::AuthenticationRequired { address } => address,
        }
    }
}

impl fmt::Display for LoadError {
    /// Plain language, because FR-015 requires the error state to name the
    /// cause to the member. These strings are the fallback the shell uses when
    /// no catalogue under FR-035 has a translation; they are deliberately not
    /// the member-facing copy, which is localised.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unresolvable { address } => {
                write!(f, "{address} could not be found")
            }
            Self::Certificate { address, detail } => {
                write!(
                    f,
                    "the identity of {address} could not be verified: {detail}"
                )
            }
            Self::Intercepted { address } => {
                write!(f, "something answered in place of {address}")
            }
            Self::AuthenticationRequired { address } => {
                write!(f, "{address} requires a sign-in before it will load")
            }
        }
    }
}

impl core::error::Error for LoadError {}

/// A page an engine has loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    address: String,
    title: String,
}

impl Page {
    pub fn new(address: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
        }
    }

    /// The address that actually loaded, which may differ from the requested
    /// one after a redirect. The shell shows this rather than what was typed,
    /// because showing the request while displaying the response is how an
    /// address bar lies.
    pub fn address(&self) -> &str {
        &self.address
    }

    pub fn title(&self) -> &str {
        &self.title
    }
}

/// What the shell requires of anything that renders web content.
///
/// Implemented by the system-webview backend on each supported platform and by
/// [`evreos-engine-headless`] for tests. FR-044 requires both to exist and the
/// second to be kept working from milestone M0.
pub trait Engine {
    /// A short, stable name for this implementation, used in diagnostics and in
    /// the benchmark records SC-013 requires to be reproducible.
    fn name(&self) -> &'static str;

    /// Load `request`, or report why it could not be loaded.
    ///
    /// An implementation MUST NOT report success for a load that did not
    /// produce a page. FR-015 names that specific defect, and it is the reason
    /// this returns a `Result` rather than a `Page` with an empty body.
    fn load(&mut self, request: &Request) -> Result<Page, LoadError>;

    /// The page currently loaded, if any.
    fn current(&self) -> Option<&Page>;
}
