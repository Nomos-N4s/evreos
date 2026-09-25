//! The shell's closed error type for presented failures and log projections.
//!
//! Under FR-015, when navigation fails — an unresolvable address, an untrusted or
//! expired certificate, an intercepting network, or a request for authentication —
//! the browser MUST distinguish that failure from a successful load, and MUST
//! present an error state naming the cause and offering a next step.
//!
//! The variants form a closed enum: a catch-all would let an implementation report
//! every failure as one indistinguishable cause, which is the state FR-015 exists
//! to forbid.
//!
//! Each variant carries the catalogue key that names its cause and the key that
//! names its next step, both resolved against [`evreos_i18n`] rather than baked as
//! English strings.
//!
//! For logging and reporting, [`ShellError::log_projection`] provides a separate
//! projection that carries no address, page title, search term or credential,
//! complying by construction with the privacy rules of FR-023, FR-007a, and FR-039c.

use core::fmt;
use evreos_engine::LoadError;
use evreos_i18n::{Language, ResolveError, catalogue};

/// The failures the shell presents to members, closed.
///
/// These correspond to the four failure causes enumerated in FR-015 and engine
/// [`LoadError`]. Each variant carries the information required to render the
/// localized cause and next step through [`evreos_i18n`], while offering a
/// separate log projection free of sensitive data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellError {
    /// The address could not be resolved.
    Unresolvable { address: String },
    /// The server certificate was untrusted, expired, or invalid.
    Certificate { address: String, detail: String },
    /// An intercepting network or captive portal answered in place of the site.
    Intercepted { address: String },
    /// The site demanded credentials before serving content.
    AuthenticationRequired { address: String },
}

impl ShellError {
    /// The address the failure concerns.
    pub fn address(&self) -> &str {
        match self {
            Self::Unresolvable { address }
            | Self::Certificate { address, .. }
            | Self::Intercepted { address }
            | Self::AuthenticationRequired { address } => address,
        }
    }

    /// The host extracted from the address for catalogue message interpolation.
    pub fn host(&self) -> &str {
        extract_host(self.address())
    }

    /// Technical detail if this is a certificate error.
    pub fn detail(&self) -> Option<&str> {
        match self {
            Self::Certificate { detail, .. } => Some(detail.as_str()),
            _ => None,
        }
    }

    /// The closed failure classification kind.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Unresolvable { .. } => ErrorKind::Unresolvable,
            Self::Certificate { .. } => ErrorKind::Certificate,
            Self::Intercepted { .. } => ErrorKind::Intercepted,
            Self::AuthenticationRequired { .. } => ErrorKind::AuthenticationRequired,
        }
    }

    /// The catalogue key that names this failure's cause.
    pub fn cause_key(&self) -> &'static str {
        match self {
            Self::Unresolvable { .. } => "error.unresolvable.cause",
            Self::Certificate { .. } => "error.certificate.cause",
            Self::Intercepted { .. } => "error.intercepted.cause",
            Self::AuthenticationRequired { .. } => "error.authentication.cause",
        }
    }

    /// The catalogue key that names this failure's next step.
    pub fn next_step_key(&self) -> &'static str {
        match self {
            Self::Unresolvable { .. } => "error.unresolvable.next_step",
            Self::Certificate { .. } => "error.certificate.next_step",
            Self::Intercepted { .. } => "error.intercepted.next_step",
            Self::AuthenticationRequired { .. } => "error.authentication.next_step",
        }
    }

    /// The pair of catalogue keys naming this failure's cause and next step.
    pub fn catalogue_keys(&self) -> (&'static str, &'static str) {
        (self.cause_key(), self.next_step_key())
    }

    /// Resolves the localised cause text for this failure in `language`.
    pub fn render_cause(&self, language: Language) -> Result<String, ResolveError> {
        let cat = catalogue(language);
        let address = self.address();
        let host = self.host();
        let args = [("address", address), ("host", host)];
        cat.resolve(self.cause_key(), &args)
    }

    /// Resolves the localised next-step text for this failure in `language`.
    pub fn render_next_step(&self, language: Language) -> Result<String, ResolveError> {
        let cat = catalogue(language);
        let address = self.address();
        let host = self.host();
        let args = [("address", address), ("host", host)];
        cat.resolve(self.next_step_key(), &args)
    }

    /// Renders the member-facing error presentation naming the cause and offering a next step.
    pub fn render(&self, language: Language) -> Result<MemberFacingError, ResolveError> {
        let cause = self.render_cause(language)?;
        let next_step = self.render_next_step(language)?;
        Ok(MemberFacingError::new(cause, next_step))
    }

    /// Projects this error into a form safe for writing to a log or a report.
    ///
    /// The projection carries no address, page title, search term or credential,
    /// fulfilling FR-023, FR-007a, and FR-039c by construction.
    pub fn log_projection(&self) -> LogProjection {
        LogProjection {
            kind: self.kind(),
            cause_key: self.cause_key(),
            next_step_key: self.next_step_key(),
        }
    }
}

impl From<LoadError> for ShellError {
    fn from(error: LoadError) -> Self {
        match error {
            LoadError::Unresolvable { address } => Self::Unresolvable { address },
            LoadError::Certificate { address, detail } => Self::Certificate { address, detail },
            LoadError::Intercepted { address } => Self::Intercepted { address },
            LoadError::AuthenticationRequired { address } => {
                Self::AuthenticationRequired { address }
            }
        }
    }
}

impl From<&LoadError> for ShellError {
    fn from(error: &LoadError) -> Self {
        match error {
            LoadError::Unresolvable { address } => Self::Unresolvable {
                address: address.clone(),
            },
            LoadError::Certificate { address, detail } => Self::Certificate {
                address: address.clone(),
                detail: detail.clone(),
            },
            LoadError::Intercepted { address } => Self::Intercepted {
                address: address.clone(),
            },
            LoadError::AuthenticationRequired { address } => Self::AuthenticationRequired {
                address: address.clone(),
            },
        }
    }
}

impl From<ShellError> for LoadError {
    fn from(error: ShellError) -> Self {
        match error {
            ShellError::Unresolvable { address } => Self::Unresolvable { address },
            ShellError::Certificate { address, detail } => Self::Certificate { address, detail },
            ShellError::Intercepted { address } => Self::Intercepted { address },
            ShellError::AuthenticationRequired { address } => {
                Self::AuthenticationRequired { address }
            }
        }
    }
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.render(Language::En) {
            Ok(rendered) => write!(f, "{rendered}"),
            Err(_) => write!(f, "{}: {}", self.cause_key(), self.address()),
        }
    }
}

impl core::error::Error for ShellError {}

/// The closed classification kinds of shell failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The address could not be found.
    Unresolvable,
    /// The certificate was untrusted or expired.
    Certificate,
    /// A captive portal or proxy intercepted the connection.
    Intercepted,
    /// The host requires authentication before loading.
    AuthenticationRequired,
}

impl ErrorKind {
    /// The stable machine-readable code for this failure kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unresolvable => "unresolvable",
            Self::Certificate => "certificate",
            Self::Intercepted => "intercepted",
            Self::AuthenticationRequired => "authentication_required",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The member-facing rendering of a shell error.
///
/// In accordance with FR-015, this presentation names the cause of the failure
/// and offers an actionable next step to the member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberFacingError {
    cause: String,
    next_step: String,
}

impl MemberFacingError {
    /// Create a new rendered presentation.
    pub fn new(cause: impl Into<String>, next_step: impl Into<String>) -> Self {
        Self {
            cause: cause.into(),
            next_step: next_step.into(),
        }
    }

    /// The localised copy naming the cause.
    pub fn cause(&self) -> &str {
        &self.cause
    }

    /// The localised copy offering the next step.
    pub fn next_step(&self) -> &str {
        &self.next_step
    }

    /// The complete presentation combining cause and next step.
    pub fn presentation(&self) -> String {
        format!("{} {}", self.cause, self.next_step)
    }
}

impl fmt::Display for MemberFacingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.cause, self.next_step)
    }
}

/// A projection of a shell error safe for emission to logs and diagnostic reports.
///
/// Under FR-023, FR-007a, and FR-039c, no log or report may contain an address,
/// page title, search term, or credential. This projection is strictly free of all
/// four by construction: it carries only the closed enum kind and static catalogue keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LogProjection {
    kind: ErrorKind,
    cause_key: &'static str,
    next_step_key: &'static str,
}

/// Type alias for [`LogProjection`].
pub type ErrorLogProjection = LogProjection;

impl LogProjection {
    /// The classified error kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Machine-readable code for the error kind.
    pub fn code(&self) -> &'static str {
        self.kind.as_str()
    }

    /// The catalogue key naming the cause.
    pub fn cause_key(&self) -> &'static str {
        self.cause_key
    }

    /// The catalogue key naming the next step.
    pub fn next_step_key(&self) -> &'static str {
        self.next_step_key
    }
}

impl From<&ShellError> for LogProjection {
    fn from(error: &ShellError) -> Self {
        error.log_projection()
    }
}

impl From<ShellError> for LogProjection {
    fn from(error: ShellError) -> Self {
        error.log_projection()
    }
}

impl fmt::Display for LogProjection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.cause_key)
    }
}

impl core::error::Error for LogProjection {}

/// Extract the hostname from an address or URL string.
fn extract_host(address: &str) -> &str {
    let without_scheme = if let Some((_, rest)) = address.split_once("://") {
        rest
    } else {
        address
    };
    let host_and_port = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme);
    let host_and_port = if let Some((_, host)) = host_and_port.split_once('@') {
        host
    } else {
        host_and_port
    };
    if let Some(bracket_end) = host_and_port.find(']') {
        &host_and_port[..=bracket_end]
    } else if let Some((host, _port)) = host_and_port.split_once(':') {
        host
    } else if !host_and_port.is_empty() {
        host_and_port
    } else {
        address
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_error_conversions() {
        let cases = [
            (
                LoadError::Unresolvable {
                    address: "https://unresolvable.invalid/".into(),
                },
                ShellError::Unresolvable {
                    address: "https://unresolvable.invalid/".into(),
                },
            ),
            (
                LoadError::Certificate {
                    address: "https://expired.invalid/".into(),
                    detail: "expired".into(),
                },
                ShellError::Certificate {
                    address: "https://expired.invalid/".into(),
                    detail: "expired".into(),
                },
            ),
            (
                LoadError::Intercepted {
                    address: "https://captive.invalid/".into(),
                },
                ShellError::Intercepted {
                    address: "https://captive.invalid/".into(),
                },
            ),
            (
                LoadError::AuthenticationRequired {
                    address: "https://auth.invalid/".into(),
                },
                ShellError::AuthenticationRequired {
                    address: "https://auth.invalid/".into(),
                },
            ),
        ];

        for (engine_err, shell_err) in cases {
            // Into ShellError
            let converted: ShellError = engine_err.clone().into();
            assert_eq!(converted, shell_err);

            // Ref into ShellError
            let converted_ref: ShellError = (&engine_err).into();
            assert_eq!(converted_ref, shell_err);

            // Back into LoadError
            let round_trip: LoadError = shell_err.clone().into();
            assert_eq!(round_trip, engine_err);

            // Accessors
            assert_eq!(shell_err.address(), engine_err.address());
        }
    }

    #[test]
    fn extract_host_handles_various_url_shapes() {
        assert_eq!(extract_host("https://example.com/path"), "example.com");
        assert_eq!(
            extract_host("http://sub.domain.org:8080/path?query#hash"),
            "sub.domain.org"
        );
        assert_eq!(
            extract_host("https://user:pass@secret.org:8443/login"),
            "secret.org"
        );
        assert_eq!(extract_host("plain-host"), "plain-host");
        assert_eq!(extract_host("https://[::1]:8080/"), "[::1]");
    }
}
