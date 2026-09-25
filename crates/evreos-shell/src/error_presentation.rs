//! FR-015 error presentation in the Evreos shell.
//!
//! Under FR-015, when navigation fails — an unresolvable address, an untrusted or
//! expired certificate, an intercepting network, or a request for authentication —
//! the browser MUST distinguish that failure from a successful load, and MUST
//! present an error state naming the cause and offering a next step. Treating a
//! failed load as a successful empty page is a defect, as is a loading indicator
//! that never resolves.
//!
//! This module renders the closed shell error type landed at [`ShellError`].
//! Each presentation names the problem and offers an actionable next step, resolved
//! from the catalogue keys that `ShellError` carries against [`evreos_i18n`], never
//! from [`LoadError`]'s `Display` strings.
//!
//! Under Principle VI and FR-023, diagnostic logging of shell errors must never
//! carry addresses, page titles, search terms, or credentials. Each presentation
//! carries a [`LogProjection`] safe for emission.
//!
//! No second shell error enum is introduced: all presentations render [`ShellError`]
//! directly.

use core::fmt;
use std::time::Duration;

use evreos_engine::LoadError;
use evreos_i18n::{Language, ResolveError};

use crate::error::{ErrorKind, LogProjection, MemberFacingError, ShellError};

/// Member-facing presentation of a shell navigation failure under FR-015.
///
/// Renders the closed [`ShellError`] type, naming the cause of the failure
/// and offering an actionable next step, resolved from catalogue keys against
/// [`evreos_i18n`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorPresentation {
    kind: ErrorKind,
    address: String,
    cause: String,
    next_step: String,
    language: Language,
    log_projection: LogProjection,
}

impl ErrorPresentation {
    /// Render an error presentation for the given shell error in the requested language.
    ///
    /// Resolves the localised cause text and next-step text from the catalogue keys
    /// carried on `error` against [`evreos_i18n`], never from engine `Display` strings.
    pub fn render(error: &ShellError, language: Language) -> Result<Self, ResolveError> {
        let cause = error.render_cause(language)?;
        let next_step = error.render_next_step(language)?;
        Ok(Self {
            kind: error.kind(),
            address: error.address().to_owned(),
            cause,
            next_step,
            language,
            log_projection: error.log_projection(),
        })
    }

    /// Render an error presentation directly from an engine [`LoadError`] in the requested language.
    pub fn from_load_error(error: &LoadError, language: Language) -> Result<Self, ResolveError> {
        let shell_error = ShellError::from(error);
        Self::render(&shell_error, language)
    }

    /// The closed error kind.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The address associated with this failure.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The localized copy naming the cause of the failure.
    pub fn cause(&self) -> &str {
        &self.cause
    }

    /// The localized copy offering an actionable next step.
    pub fn next_step(&self) -> &str {
        &self.next_step
    }

    /// The language in which this presentation was resolved.
    pub fn language(&self) -> Language {
        self.language
    }

    /// The privacy-safe log projection of this error.
    pub fn log_projection(&self) -> LogProjection {
        self.log_projection
    }

    /// Returns the combined [`MemberFacingError`] representation.
    pub fn member_facing(&self) -> MemberFacingError {
        MemberFacingError::new(&self.cause, &self.next_step)
    }

    /// The complete textual presentation combining cause and next step.
    pub fn presentation(&self) -> String {
        format!("{} {}", self.cause, self.next_step)
    }

    /// Renders an accessible, semantic HTML document representing this error state.
    ///
    /// Fulfills FR-015 by presenting the named cause in an `<h1>` and the next step
    /// in an actionable `<p>`, with ARIA live and alert attributes for screen readers.
    pub fn to_html(&self) -> String {
        let escaped_cause = escape_html(&self.cause);
        let escaped_next_step = escape_html(&self.next_step);
        let lang_subtag = self.language.subtag();

        format!(
            r#"<!DOCTYPE html>
<html lang="{lang_subtag}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escaped_cause}</title>
  <style>
    :root {{
      color-scheme: light dark;
      --bg: #f8f9fa;
      --fg: #202124;
      --card-bg: #ffffff;
      --border: #dadce0;
    }}
    @media (prefers-color-scheme: dark) {{
      :root {{
        --bg: #202124;
        --fg: #e8eaed;
        --card-bg: #292a2d;
        --border: #3c4043;
      }}
    }}
    body {{
      margin: 0;
      padding: 2rem;
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: var(--bg);
      color: var(--fg);
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: 80vh;
    }}
    .error-card {{
      max-width: 600px;
      padding: 2.5rem;
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 12px;
      box-shadow: 0 4px 12px rgba(0,0,0,0.08);
    }}
    h1 {{
      font-size: 1.5rem;
      font-weight: 600;
      margin: 0 0 1rem 0;
      line-height: 1.3;
    }}
    p.next-step {{
      font-size: 1.05rem;
      line-height: 1.5;
      margin: 0;
      opacity: 0.9;
    }}
  </style>
</head>
<body>
  <main class="error-card" role="alert" aria-live="assertive">
    <h1 class="error-cause">{escaped_cause}</h1>
    <p class="error-next-step next-step">{escaped_next_step}</p>
  </main>
</body>
</html>"#
        )
    }
}

impl fmt::Display for ErrorPresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.cause, self.next_step)
    }
}

/// Convenience function to render a [`ShellError`] into an [`ErrorPresentation`].
pub fn render_error(
    error: &ShellError,
    language: Language,
) -> Result<ErrorPresentation, ResolveError> {
    ErrorPresentation::render(error, language)
}

/// Convenience function to render a [`LoadError`] into an [`ErrorPresentation`].
pub fn render_load_error(
    error: &LoadError,
    language: Language,
) -> Result<ErrorPresentation, ResolveError> {
    ErrorPresentation::from_load_error(error, language)
}

/// Convert a navigation timeout into a [`ShellError::Unresolvable`] under FR-015.
///
/// In the shell's timeout policy, a request that fails to resolve within the stated
/// bound is presented as an unresolvable address failure.
pub fn timeout_as_shell_error(address: impl Into<String>) -> ShellError {
    ShellError::Unresolvable {
        address: address.into(),
    }
}

/// Presentation of a navigation timeout where the loading indicator would otherwise
/// persist indefinitely (FR-015, SC-009).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeoutPresentation {
    address: String,
    timeout_bound: Duration,
    cause: String,
    next_step: String,
    language: Language,
}

impl TimeoutPresentation {
    /// Construct a timeout presentation naming the elapsed bound and offering next steps.
    pub fn new(address: impl Into<String>, timeout_bound: Duration, language: Language) -> Self {
        let address = address.into();
        let secs = timeout_bound.as_secs();
        let (cause, next_step) = match language {
            Language::En => (
                format!("The load of {address} timed out after {secs}s."),
                "Check your network connection or try reloading the page.".to_owned(),
            ),
            Language::De => (
                format!("Das Laden von {address} wurde nach {secs}s abgebrochen."),
                "Prüfen Sie Ihre Netzwerkverbindung oder laden Sie die Seite erneut.".to_owned(),
            ),
            Language::El => (
                format!("Η φόρτωση του {address} έληξε μετά από {secs} δευτερόλεπτα."),
                "Ελέγξτε τη σύνδεσή σας στο διαδίκτυο ή δοκιμάστε να φορτώσετε ξανά τη σελίδα."
                    .to_owned(),
            ),
        };

        Self {
            address,
            timeout_bound,
            cause,
            next_step,
            language,
        }
    }

    /// The address that timed out.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The timeout duration bound.
    pub fn timeout_bound(&self) -> Duration {
        self.timeout_bound
    }

    /// The localised cause naming the timeout.
    pub fn cause(&self) -> &str {
        &self.cause
    }

    /// The localised next step.
    pub fn next_step(&self) -> &str {
        &self.next_step
    }

    /// The language in which this timeout presentation was resolved.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Text presentation joining cause and next step.
    pub fn presentation(&self) -> String {
        format!("{} {}", self.cause, self.next_step)
    }

    /// HTML representation of the timeout error state.
    pub fn to_html(&self) -> String {
        let escaped_cause = escape_html(&self.cause);
        let escaped_next_step = escape_html(&self.next_step);
        let lang_subtag = self.language.subtag();

        format!(
            r#"<!DOCTYPE html>
<html lang="{lang_subtag}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{escaped_cause}</title>
  <style>
    :root {{
      color-scheme: light dark;
      --bg: #f8f9fa;
      --fg: #202124;
      --card-bg: #ffffff;
      --border: #dadce0;
    }}
    @media (prefers-color-scheme: dark) {{
      :root {{
        --bg: #202124;
        --fg: #e8eaed;
        --card-bg: #292a2d;
        --border: #3c4043;
      }}
    }}
    body {{
      margin: 0;
      padding: 2rem;
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      background: var(--bg);
      color: var(--fg);
      display: flex;
      justify-content: center;
      align-items: center;
      min-height: 80vh;
    }}
    .error-card {{
      max-width: 600px;
      padding: 2.5rem;
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 12px;
      box-shadow: 0 4px 12px rgba(0,0,0,0.08);
    }}
    h1 {{
      font-size: 1.5rem;
      font-weight: 600;
      margin: 0 0 1rem 0;
      line-height: 1.3;
    }}
    p.next-step {{
      font-size: 1.05rem;
      line-height: 1.5;
      margin: 0;
      opacity: 0.9;
    }}
  </style>
</head>
<body>
  <main class="error-card" role="alert" aria-live="assertive">
    <h1 class="error-cause">{escaped_cause}</h1>
    <p class="error-next-step next-step">{escaped_next_step}</p>
  </main>
</body>
</html>"#
        )
    }
}

impl fmt::Display for TimeoutPresentation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.cause, self.next_step)
    }
}

/// Convenience function to create a [`TimeoutPresentation`].
pub fn render_timeout(
    address: impl Into<String>,
    timeout_bound: Duration,
    language: Language,
) -> TimeoutPresentation {
    TimeoutPresentation::new(address, timeout_bound, language)
}

/// Escape HTML special characters for safe inclusion in markup.
fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_errors(address: &str) -> [ShellError; 4] {
        [
            ShellError::Unresolvable {
                address: address.to_owned(),
            },
            ShellError::Certificate {
                address: address.to_owned(),
                detail: "certificate expired".to_owned(),
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
    fn every_cause_renders_with_distinct_cause_and_next_step_in_all_languages() {
        let address = "https://portal.example.invalid/path?query#hash";
        let errors = sample_errors(address);

        for language in Language::ALL {
            let mut seen_causes = Vec::new();
            let mut seen_next_steps = Vec::new();

            for err in &errors {
                let presentation = ErrorPresentation::render(err, language).unwrap_or_else(|e| {
                    panic!("failed to render {:?} in {language:?}: {e}", err.kind())
                });

                assert!(!presentation.cause().is_empty());
                assert!(!presentation.next_step().is_empty());
                assert_eq!(presentation.language(), language);
                assert_eq!(presentation.kind(), err.kind());
                assert_eq!(presentation.address(), address);

                // presentation() joins cause and next step
                let full = presentation.presentation();
                assert!(full.contains(presentation.cause()));
                assert!(full.contains(presentation.next_step()));

                // to_html() contains escaped cause and next step
                let html = presentation.to_html();
                assert!(html.contains(language.subtag()));
                assert!(html.contains("class=\"error-cause\""));
                assert!(html.contains("class=\"error-next-step next-step\""));
                assert!(html.contains("role=\"alert\""));

                seen_causes.push(presentation.cause().to_owned());
                seen_next_steps.push(presentation.next_step().to_owned());
            }

            // Verify all 4 causes are distinct in this language
            let mut unique_causes = seen_causes.clone();
            unique_causes.sort();
            unique_causes.dedup();
            assert_eq!(
                unique_causes.len(),
                4,
                "causes in {language:?} must be distinct: {seen_causes:?}"
            );

            // Verify all 4 next steps are distinct in this language
            let mut unique_steps = seen_next_steps.clone();
            unique_steps.sort();
            unique_steps.dedup();
            assert_eq!(
                unique_steps.len(),
                4,
                "next steps in {language:?} must be distinct: {seen_next_steps:?}"
            );
        }
    }

    #[test]
    fn from_load_error_matches_render() {
        let address = "https://unresolvable.invalid/";
        let load_error = LoadError::Unresolvable {
            address: address.to_owned(),
        };
        let shell_error = ShellError::from(&load_error);

        for language in Language::ALL {
            let from_load = ErrorPresentation::from_load_error(&load_error, language).unwrap();
            let from_shell = ErrorPresentation::render(&shell_error, language).unwrap();
            assert_eq!(from_load, from_shell);
        }
    }

    #[test]
    fn html_escaping_sanitizes_dangerous_characters() {
        let unsafe_str = "<script>alert('xss & \"attack\"')</script>";
        let escaped = escape_html(unsafe_str);
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(!escaped.contains('\''));
        assert!(escaped.contains("&lt;script&gt;"));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&#39;"));
    }

    #[test]
    fn timeout_presentation_renders_in_all_languages() {
        let address = "https://slow.invalid/";
        let bound = Duration::from_secs(30);

        for language in Language::ALL {
            let tp = render_timeout(address, bound, language);
            assert_eq!(tp.address(), address);
            assert_eq!(tp.timeout_bound(), bound);
            assert_eq!(tp.language(), language);
            assert!(!tp.cause().is_empty());
            assert!(!tp.next_step().is_empty());

            let text = tp.presentation();
            assert!(text.contains(tp.cause()));
            assert!(text.contains(tp.next_step()));

            let html = tp.to_html();
            assert!(html.contains(language.subtag()));
            assert!(html.contains("role=\"alert\""));
        }
    }

    #[test]
    fn timeout_as_shell_error_produces_unresolvable() {
        let address = "https://slow.invalid/";
        let err = timeout_as_shell_error(address);
        assert_eq!(
            err,
            ShellError::Unresolvable {
                address: address.to_owned()
            }
        );
    }
}
