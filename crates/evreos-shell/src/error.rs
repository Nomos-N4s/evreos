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

    fn all_variants(address: &str) -> [ShellError; 4] {
        [
            ShellError::Unresolvable {
                address: address.to_owned(),
            },
            ShellError::Certificate {
                address: address.to_owned(),
                detail: "certificate has expired".to_owned(),
            },
            ShellError::Intercepted {
                address: address.to_owned(),
            },
            ShellError::AuthenticationRequired {
                address: address.to_owned(),
            },
        ]
    }

    #[test]
    fn every_variant_resolves_to_both_keys_in_all_languages() {
        let address = "https://example.invalid/";
        let variants = all_variants(address);

        for error in &variants {
            for language in Language::ALL {
                let cause = error.render_cause(language);
                assert!(
                    cause.is_ok(),
                    "cause for {:?} failed to resolve in {}: {:?}",
                    error.kind(),
                    language.subtag(),
                    cause
                );
                let cause_text = cause.unwrap();
                assert!(
                    !cause_text.trim().is_empty(),
                    "cause for {:?} resolved to empty text in {}",
                    error.kind(),
                    language.subtag()
                );

                let next_step = error.render_next_step(language);
                assert!(
                    next_step.is_ok(),
                    "next step for {:?} failed to resolve in {}: {:?}",
                    error.kind(),
                    language.subtag(),
                    next_step
                );
                let next_step_text = next_step.unwrap();
                assert!(
                    !next_step_text.trim().is_empty(),
                    "next step for {:?} resolved to empty text in {}",
                    error.kind(),
                    language.subtag()
                );
            }
        }
    }

    #[test]
    fn member_facing_rendering_names_cause_and_offers_next_step() {
        let host = "subdomain.example.invalid";
        let address = format!("https://{host}/path");
        let variants = all_variants(&address);

        for error in &variants {
            for language in Language::ALL {
                let rendered = error
                    .render(language)
                    .unwrap_or_else(|e| panic!("failed rendering {error:?} in {language:?}: {e}"));

                // Names the problem
                assert!(
                    !rendered.cause().is_empty(),
                    "empty cause in {}",
                    language.subtag()
                );
                // Offers a next step
                assert!(
                    !rendered.next_step().is_empty(),
                    "empty next step in {}",
                    language.subtag()
                );

                // For English, verify specific content requirements
                if language == Language::En {
                    match error {
                        ShellError::Unresolvable { .. } => {
                            assert!(
                                rendered.cause().contains(&address),
                                "Unresolvable cause must name address: {}",
                                rendered.cause()
                            );
                            assert!(
                                rendered.next_step().contains("spelling"),
                                "Unresolvable next step must advise spelling: {}",
                                rendered.next_step()
                            );
                        }
                        ShellError::Certificate { .. } => {
                            assert!(
                                rendered.cause().contains(host),
                                "Certificate cause must name host: {}",
                                rendered.cause()
                            );
                            assert!(
                                rendered.next_step().contains("Go back"),
                                "Certificate next step must advise going back: {}",
                                rendered.next_step()
                            );
                        }
                        ShellError::Intercepted { .. } => {
                            assert!(
                                rendered.cause().contains(host),
                                "Intercepted cause must name host: {}",
                                rendered.cause()
                            );
                            assert!(
                                rendered.next_step().contains("Sign in"),
                                "Intercepted next step must advise sign in or switch network: {}",
                                rendered.next_step()
                            );
                        }
                        ShellError::AuthenticationRequired { .. } => {
                            assert!(
                                rendered.cause().contains(host),
                                "Authentication cause must name host: {}",
                                rendered.cause()
                            );
                            assert!(
                                rendered.next_step().contains("Sign in"),
                                "Authentication next step must advise sign in: {}",
                                rendered.next_step()
                            );
                        }
                    }
                }

                // Check presentation format joins both
                let presentation = rendered.presentation();
                assert!(presentation.contains(rendered.cause()));
                assert!(presentation.contains(rendered.next_step()));
            }
        }
    }

    #[test]
    fn log_projection_is_strictly_free_of_address_page_title_search_term_and_credential() {
        let sensitive_address = "https://sensitive.finance.bank.invalid:8443/accounts/private";
        let sensitive_title = "My Secret Financial Portfolio";
        let sensitive_search_term = "confidential tax records 2026";
        let sensitive_credential = "super_secret_bearer_token_xyz123";

        let sensitive_url_with_credentials = format!(
            "https://user:{sensitive_credential}@sensitive.finance.bank.invalid:8443/search?q={sensitive_search_term}"
        );

        let variants = [
            ShellError::Unresolvable {
                address: sensitive_url_with_credentials.clone(),
            },
            ShellError::Certificate {
                address: sensitive_url_with_credentials.clone(),
                detail: format!("{sensitive_title} - cert failed for {sensitive_address}"),
            },
            ShellError::Intercepted {
                address: sensitive_url_with_credentials.clone(),
            },
            ShellError::AuthenticationRequired {
                address: sensitive_url_with_credentials,
            },
        ];

        let sensitive_tokens = [
            sensitive_address,
            "sensitive.finance.bank.invalid",
            "accounts/private",
            sensitive_title,
            sensitive_search_term,
            sensitive_credential,
            "super_secret",
            "confidential",
        ];

        for error in &variants {
            let projection = error.log_projection();
            let debug_repr = format!("{projection:?}");
            let display_repr = format!("{projection}");
            let code = projection.code();
            let cause_key = projection.cause_key();
            let next_step_key = projection.next_step_key();

            for token in sensitive_tokens {
                assert!(
                    !debug_repr.contains(token),
                    "Debug representation of log projection for {:?} contained sensitive token {:?}: {}",
                    error.kind(),
                    token,
                    debug_repr
                );
                assert!(
                    !display_repr.contains(token),
                    "Display representation of log projection for {:?} contained sensitive token {:?}: {}",
                    error.kind(),
                    token,
                    display_repr
                );
                assert!(
                    !code.contains(token),
                    "Code for {:?} contained sensitive token {:?}: {}",
                    error.kind(),
                    token,
                    code
                );
                assert!(
                    !cause_key.contains(token),
                    "Cause key for {:?} contained sensitive token {:?}: {}",
                    error.kind(),
                    token,
                    cause_key
                );
                assert!(
                    !next_step_key.contains(token),
                    "Next step key for {:?} contained sensitive token {:?}: {}",
                    error.kind(),
                    token,
                    next_step_key
                );
            }
        }
    }

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
