//! Hand-off browser selection and offers under FR-015a, FR-037, and FR-007a.
//!
//! Under FR-015a, where the platform engine does not provide site-credential autofill,
//! the browser MUST offer to open the site in the hand-off browser when a
//! site-credential field is detected. Local detection MUST inspect only whether
//! a password-type input is present, and MUST NOT transmit or retain page content.
//!
//! Under FR-037, where a capability proves unavailable (such as hardware media capture,
//! geolocation, notification, or protected-media playback), the browser MUST say so
//! and offer a hand-off rather than failing silently or presenting a misleading prompt.
//!
//! Under FR-007a, hand-off is the fourth enumerated transmission: it passes
//! ONLY the address of the current site, on the member's action for that occasion,
//! to a program on the same machine and NEVER to a server.
//!
//! Under Key Entities, Evreos MUST NEVER nominate itself as the hand-off browser.

use core::fmt;
use std::path::Path;

use crate::permissions::Capability;
use crate::profile::ProfileError;

/// Reasons for presenting a hand-off offer to the member.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HandOffReason {
    /// FR-015a: A password-type input was detected on a platform where the engine
    /// does not provide site-credential autofill.
    PasswordInputDetected,
    /// FR-037: A declared site capability proves unavailable on this platform.
    CapabilityUnavailable(Capability),
    /// FR-037: Protected media playback proves unavailable on this platform.
    ProtectedMediaUnavailable,
}

impl HandOffReason {
    /// Human-readable explanation of why the hand-off offer was raised.
    pub fn description(&self) -> &'static str {
        match self {
            Self::PasswordInputDetected => {
                "Site-credential autofill is not available in Evreos. Open this site in another browser to use your saved passwords."
            }
            Self::CapabilityUnavailable(Capability::Camera) => {
                "Camera hardware access is unavailable on this platform. Open this site in another browser to use your camera."
            }
            Self::CapabilityUnavailable(Capability::Microphone) => {
                "Microphone hardware access is unavailable on this platform. Open this site in another browser to use your microphone."
            }
            Self::CapabilityUnavailable(Capability::Location) => {
                "Geographic location access is unavailable on this platform. Open this site in another browser to share your location."
            }
            Self::CapabilityUnavailable(Capability::Notification) => {
                "System desktop notifications are unavailable on this platform. Open this site in another browser to receive notifications."
            }
            Self::ProtectedMediaUnavailable => {
                "Protected media playback is unavailable on this platform. Open this site in another browser to play protected content."
            }
        }
    }
}

impl fmt::Display for HandOffReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Errors arising from hand-off browser selection or dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandOffError {
    /// The application attempted to nominate itself as the hand-off browser.
    SelfNominationRefused(String),
    /// The nominated browser program string was empty or only whitespace.
    EmptyProgram,
    /// The site address passed to hand-off was invalid or empty.
    InvalidAddress(String),
    /// The local dispatch failed to execute.
    ExecutionFailed(String),
}

impl fmt::Display for HandOffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SelfNominationRefused(prog) => {
                write!(
                    f,
                    "self-nomination refused: cannot nominate Evreos ({prog}) as hand-off browser"
                )
            }
            Self::EmptyProgram => write!(f, "nominated browser program cannot be empty"),
            Self::InvalidAddress(addr) => write!(f, "invalid hand-off address: {addr}"),
            Self::ExecutionFailed(msg) => write!(f, "hand-off execution failed: {msg}"),
        }
    }
}

impl std::error::Error for HandOffError {}

impl From<HandOffError> for ProfileError {
    fn from(err: HandOffError) -> Self {
        ProfileError::InvalidHandOffBrowser(err.to_string())
    }
}

/// Hand-off browser selection configuring which local application receives hand-offs.
///
/// Under Key Entities and FR-015a, Evreos MUST NOT nominate itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum HandOffBrowser {
    /// The operating system's default browser application.
    #[default]
    SystemDefault,
    /// A specific external application nominated by the user.
    Nominated {
        /// The executable command or application path.
        program: String,
    },
}

impl HandOffBrowser {
    /// Nominate a specific external browser executable or command.
    ///
    /// Validates that the application does not nominate itself (Key Entities rule).
    pub fn nominated(program: impl Into<String>) -> Result<Self, ProfileError> {
        let prog = program.into();
        Self::validate_nominated(&prog).map_err(Into::into)
    }

    /// Validate and construct a nominated hand-off browser target.
    ///
    /// Strictly rejects self-nomination: matching "self", "evreos", executable
    /// stems, or the branded product name.
    pub fn validate_nominated(program: &str) -> Result<Self, HandOffError> {
        let trimmed = program.trim();
        if trimmed.is_empty() {
            return Err(HandOffError::EmptyProgram);
        }

        if is_self_nomination(trimmed) {
            return Err(HandOffError::SelfNominationRefused(trimmed.to_string()));
        }

        Ok(Self::Nominated {
            program: trimmed.to_string(),
        })
    }

    /// Returns the nominated program command, if any.
    pub fn program(&self) -> Option<&str> {
        match self {
            Self::SystemDefault => None,
            Self::Nominated { program } => Some(program.as_str()),
        }
    }

    /// Whether this selection uses the system default browser.
    pub fn is_system_default(&self) -> bool {
        matches!(self, Self::SystemDefault)
    }
}

/// Check whether `program` refers to Evreos itself in any shape.
fn is_self_nomination(program: &str) -> bool {
    let lower = program.to_ascii_lowercase();

    // 1. Literal "self"
    if lower == "self" {
        return true;
    }

    // 2. Base identity "evreos"
    if lower == "evreos" || lower.starts_with("evreos ") {
        return true;
    }

    // 3. Dynamic brand product name
    let product = crate::brand::brand().product_name.to_ascii_lowercase();
    if !product.is_empty()
        && product != "unset"
        && (lower == product || lower.starts_with(&format!("{product} ")))
    {
        return true;
    }

    // 4. File stem check for paths (e.g. /usr/bin/evreos, C:\...\evreos.exe)
    // Extract filename by splitting on both '/' and '\\' so cross-platform paths parse correctly on any host OS.
    let filename = program
        .split(['/', '\\'])
        .rfind(|segment| !segment.is_empty())
        .unwrap_or(program);
    let path = Path::new(filename);
    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        let stem_lower = stem.to_ascii_lowercase();
        if stem_lower == "evreos" {
            return true;
        }
        if !product.is_empty() && product != "unset" && stem_lower == product {
            return true;
        }
    }

    false
}

/// Inspect an HTML document or snippet strictly for the presence of password-type inputs.
///
/// Under FR-015a:
/// - Detection MUST be local to the device.
/// - Detection MUST inspect ONLY whether a password-type input is present.
/// - Detection MUST NOT transmit or retain page content.
///
/// Returns `true` if an `<input>` element with `type="password"` (case-insensitive)
/// is found, and `false` otherwise. This function retains zero page content.
pub fn detect_password_input(html_or_snippet: &str) -> bool {
    let mut cursor = html_or_snippet;

    while let Some(tag_start) = cursor.find('<') {
        let rest = &cursor[tag_start + 1..];
        let Some(tag_end) = rest.find('>') else {
            break;
        };
        let tag_content = &rest[..tag_end];
        cursor = &rest[tag_end + 1..];

        // Must start with "input" (case-insensitive) followed by whitespace, slash, or end
        let trimmed = tag_content.trim_start();
        if trimmed.len() >= 5 && trimmed[..5].eq_ignore_ascii_case("input") {
            let after_input = &trimmed[5..];
            let is_boundary = after_input.is_empty()
                || after_input
                    .starts_with(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>');
            if is_boundary && has_password_type_attribute(after_input) {
                return true;
            }
        }
    }

    false
}

/// Helper parsing tag attributes to check if `type` equals "password".
fn has_password_type_attribute(attributes: &str) -> bool {
    let lower = attributes.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() {
        // Look for 'type' attribute
        if let Some(type_pos) = lower[idx..].find("type") {
            let abs_pos = idx + type_pos;
            let before = if abs_pos == 0 {
                ' '
            } else {
                bytes[abs_pos - 1] as char
            };

            // Ensure 'type' is a standalone attribute name
            if before.is_ascii_whitespace() || before == '/' {
                let after = abs_pos + 4;
                let remainder = lower[after..].trim_start();
                if let Some(stripped) = remainder.strip_prefix('=') {
                    let val = stripped.trim_start();
                    let val_content = if let Some(unquoted) = val.strip_prefix('"') {
                        unquoted.split('"').next().unwrap_or("")
                    } else if let Some(unquoted) = val.strip_prefix('\'') {
                        unquoted.split('\'').next().unwrap_or("")
                    } else {
                        val.split(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
                            .next()
                            .unwrap_or("")
                    };

                    if val_content.trim() == "password" {
                        return true;
                    }
                }
            }
            idx = abs_pos + 4;
        } else {
            break;
        }
    }

    false
}

/// A member-facing hand-off offer under FR-015a or FR-037.
///
/// Carries ONLY the current site address and the reason for the offer.
/// Under FR-007a and FR-015a, it retains NO page content, DOM data, or credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandOffOffer {
    reason: HandOffReason,
    address: String,
    target: HandOffBrowser,
}

impl HandOffOffer {
    /// Create a new hand-off offer for an address and reason.
    pub fn new(
        reason: HandOffReason,
        address: impl Into<String>,
        target: HandOffBrowser,
    ) -> Result<Self, HandOffError> {
        let addr = address.into();
        let trimmed_addr = addr.trim();
        if trimmed_addr.is_empty() {
            return Err(HandOffError::InvalidAddress(
                "address cannot be empty".into(),
            ));
        }

        Ok(Self {
            reason,
            address: trimmed_addr.to_string(),
            target,
        })
    }

    /// Create an FR-015a hand-off offer for detected password inputs.
    pub fn for_site_credential(
        address: impl Into<String>,
        target: HandOffBrowser,
    ) -> Result<Self, HandOffError> {
        Self::new(HandOffReason::PasswordInputDetected, address, target)
    }

    /// Create an FR-037 hand-off offer for an unavailable capability.
    pub fn for_unavailable_capability(
        capability: Capability,
        address: impl Into<String>,
        target: HandOffBrowser,
    ) -> Result<Self, HandOffError> {
        Self::new(
            HandOffReason::CapabilityUnavailable(capability),
            address,
            target,
        )
    }

    /// Create an FR-037 hand-off offer for unavailable protected media playback.
    pub fn for_protected_media(
        address: impl Into<String>,
        target: HandOffBrowser,
    ) -> Result<Self, HandOffError> {
        Self::new(HandOffReason::ProtectedMediaUnavailable, address, target)
    }

    /// The reason this hand-off offer was raised.
    pub fn reason(&self) -> &HandOffReason {
        &self.reason
    }

    /// The address of the current site to hand off.
    ///
    /// Under FR-007a, this address is the ONLY data passed to the target browser.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The target browser to receive the hand-off.
    pub fn target(&self) -> &HandOffBrowser {
        &self.target
    }

    /// Prepare the execution dispatch payload.
    ///
    /// Returns the target program and arguments consisting strictly of `[self.address]`.
    pub fn prepare_dispatch(&self) -> (Option<&str>, Vec<String>) {
        (self.target.program(), vec![self.address.clone()])
    }

    /// Execute the hand-off on the member's action for that occasion.
    ///
    /// Passes ONLY the address of the current site to a program on the same machine,
    /// never to a remote server (FR-007a).
    pub fn dispatch(&self, executor: &mut impl HandOffExecutor) -> Result<(), HandOffError> {
        executor.execute_handoff(&self.target, &self.address)
    }
}

impl fmt::Display for HandOffOffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Hand-off offer for {}: {}",
            self.address,
            self.reason.description()
        )
    }
}

/// Trait implemented by hand-off execution backends.
///
/// Guarantees that only the address is dispatched to a local program on the same machine.
pub trait HandOffExecutor {
    /// Execute hand-off to the target browser on the local machine with `address`.
    fn execute_handoff(
        &mut self,
        target: &HandOffBrowser,
        address: &str,
    ) -> Result<(), HandOffError>;
}

/// In-memory mock hand-off executor for deterministic testing.
///
/// Records every dispatch call and verifies that only the site address was transmitted.
#[derive(Debug, Default, Clone)]
pub struct MockHandOffExecutor {
    dispatches: Vec<(HandOffBrowser, String)>,
}

impl MockHandOffExecutor {
    /// Create a new mock executor.
    pub fn new() -> Self {
        Self::default()
    }

    /// All recorded dispatches.
    pub fn dispatches(&self) -> &[(HandOffBrowser, String)] {
        &self.dispatches
    }

    /// Total number of dispatches recorded.
    pub fn dispatch_count(&self) -> usize {
        self.dispatches.len()
    }

    /// Reset recorded dispatches.
    pub fn clear(&mut self) {
        self.dispatches.clear();
    }
}

impl HandOffExecutor for MockHandOffExecutor {
    fn execute_handoff(
        &mut self,
        target: &HandOffBrowser,
        address: &str,
    ) -> Result<(), HandOffError> {
        if address.trim().is_empty() {
            return Err(HandOffError::InvalidAddress(
                "address cannot be empty".into(),
            ));
        }
        self.dispatches.push((target.clone(), address.to_string()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_detection_finds_all_password_input_variants() {
        let positive_cases = [
            "<input type=\"password\">",
            "<input type='password'>",
            "<input type=password>",
            "<input type=\"PASSWORD\">",
            "<INPUT TYPE=\"password\">",
            "<input name=\"pass\" type=\"password\" id=\"p\" />",
            "<input class=\"form-control\" type = 'password' >",
            "<div><form><input type=\"password\"></form></div>",
            "<input\n  type=\"password\"\n  autocomplete=\"current-password\">",
        ];

        for snippet in positive_cases {
            assert!(
                detect_password_input(snippet),
                "failed to detect password input in: {snippet}"
            );
        }
    }

    #[test]
    fn password_detection_ignores_non_password_inputs_and_text() {
        let negative_cases = [
            "",
            "<html><body>No inputs here</body></html>",
            "<input type=\"text\">",
            "<input type=\"email\">",
            "<input type=\"search\">",
            "<input type=\"hidden\" value=\"password\">",
            "<p>Please enter your password:</p><input type=\"text\">",
            "<button type=\"submit\">Change password</button>",
            "<label for=\"password\">Password</label>",
            "<textarea name=\"password\"></textarea>",
            "<div class=\"password-wrapper\"></div>",
        ];

        for snippet in negative_cases {
            assert!(
                !detect_password_input(snippet),
                "false positive password detection in: {snippet}"
            );
        }
    }

    #[test]
    fn handoff_browser_refuses_self_nomination() {
        assert!(HandOffBrowser::nominated("self").is_err());
        assert!(HandOffBrowser::nominated("SELF").is_err());
        assert!(HandOffBrowser::nominated("evreos").is_err());
        assert!(HandOffBrowser::nominated("EVREOS").is_err());
        assert!(HandOffBrowser::nominated("evreos.exe").is_err());
        assert!(HandOffBrowser::nominated("/usr/local/bin/evreos").is_err());
        assert!(HandOffBrowser::nominated("C:\\Program Files\\Evreos\\evreos.exe").is_err());
        assert!(HandOffBrowser::nominated("   ").is_err());
    }

    #[test]
    fn handoff_browser_accepts_external_browsers() {
        assert!(HandOffBrowser::nominated("firefox").is_ok());
        assert!(HandOffBrowser::nominated("chrome").is_ok());
        assert!(HandOffBrowser::nominated("safari").is_ok());
        assert!(HandOffBrowser::nominated("edge").is_ok());
        assert!(HandOffBrowser::nominated("/usr/bin/firefox").is_ok());
    }

    #[test]
    fn handoff_offer_creation_and_dispatch() {
        let mut executor = MockHandOffExecutor::new();
        let target = HandOffBrowser::nominated("firefox").expect("valid browser");
        let offer =
            HandOffOffer::for_site_credential("https://login.example.invalid/", target.clone())
                .expect("valid offer");

        assert_eq!(offer.reason(), &HandOffReason::PasswordInputDetected);
        assert_eq!(offer.address(), "https://login.example.invalid/");
        assert_eq!(offer.target(), &target);

        offer.dispatch(&mut executor).expect("dispatch success");
        assert_eq!(executor.dispatch_count(), 1);
        assert_eq!(
            executor.dispatches()[0],
            (target, "https://login.example.invalid/".to_string())
        );
    }
}
