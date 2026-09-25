//! The shell's structured, typed logging facility.
//!
//! Under FR-023, FR-007a, and FR-039c, no log may hold an account credential, a
//! token derived from one, or any value from which either can be reconstructed, nor
//! any visited address, page title, or search query.
//!
//! To enforce this by construction rather than by care:
//! 1. Records are built solely from typed fields with no free-form message argument.
//! 2. Sensitive values — addresses, page titles, search terms, credentials, and tokens —
//!    must be placed in [`Sensitive<T>`] wrappers (or their domain newtypes) which have
//!    no path to a written log value.
//! 3. Neither [`Sensitive<T>`], [`Address`], [`Credential`], [`PageTitle`],
//!    [`SearchTerm`], [`Token`], nor arbitrary heap [`String`] implements [`LogValue`].
//! 4. Attempting to log a sensitive value fails at compile time.
//!
//! # On-Disk Location and Retention Policy
//!
//! - **Directory**: [`LOG_DIR_NAME`] (`"logs"`) relative to the member profile directory.
//! - **Active file**: [`LOG_FILE_NAME`] (`"evreos.log"`), at [`LOG_RELATIVE_PATH`] (`"logs/evreos.log"`).
//! - **Retention duration**: [`LOG_RETENTION_DAYS`] (7 days), bounding the diagnostic footprint
//!   on disk in accordance with FR-023 and FR-007a.

use core::fmt;
use std::path::{Path, PathBuf};

use crate::error::{ErrorKind, LogProjection};

// -----------------------------------------------------------------------------
// On-Disk Location and Retention Policy Constants
// -----------------------------------------------------------------------------

/// The directory name under the profile directory where logs are written.
pub const LOG_DIR_NAME: &str = "logs";

/// The active log file name.
pub const LOG_FILE_NAME: &str = "evreos.log";

/// The relative path from the profile root to the active log file.
pub const LOG_RELATIVE_PATH: &str = "logs/evreos.log";

/// The retention period in days for log records and log files.
///
/// Local logs older than this duration are subject to pruning, fulfilling
/// the privacy constraints of FR-023 and FR-007a.
pub const LOG_RETENTION_DAYS: u64 = 7;

/// The retention period as a [`core::time::Duration`].
pub const LOG_RETENTION: core::time::Duration =
    core::time::Duration::from_secs(LOG_RETENTION_DAYS * 24 * 60 * 60);

/// Resolves the absolute path to the active log file within a given profile root.
pub fn log_file_path(profile_root: &Path) -> PathBuf {
    profile_root.join(LOG_DIR_NAME).join(LOG_FILE_NAME)
}

/// Resolves the log directory within a given profile root.
pub fn log_directory(profile_root: &Path) -> PathBuf {
    profile_root.join(LOG_DIR_NAME)
}

// -----------------------------------------------------------------------------
// Sensitive Wrapper and Domain Types
// -----------------------------------------------------------------------------

/// A wrapper for sensitive values that must never be emitted to logs or diagnostics.
///
/// In compliance with FR-023, FR-007a, and FR-039c, values that carry an address,
/// a page title, a search term, a credential, or a token must be wrapped in `Sensitive<T>`.
///
/// `Sensitive<T>` has no path to a written value:
/// - It intentionally does NOT implement [`fmt::Display`].
/// - It intentionally does NOT implement [`LogValue`].
/// - It intentionally does NOT implement [`core::ops::Deref`].
/// - Its [`fmt::Debug`] implementation renders `[REDACTED]`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    /// Wraps a value in a `Sensitive` container.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Accesses the inner value for authorized non-logging operations.
    pub fn as_inner(&self) -> &T {
        &self.0
    }

    /// Consumes the wrapper, returning the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

/// A web address / URL, protected under FR-007a, FR-023, and FR-039c.
///
/// Holds an address within [`Sensitive<String>`] with no path to a log stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub Sensitive<String>);

impl Address {
    /// Creates a new sensitive `Address`.
    pub fn new(address: impl Into<String>) -> Self {
        Self(Sensitive::new(address.into()))
    }

    /// Returns a reference to the underlying [`Sensitive`] wrapper.
    pub fn as_sensitive(&self) -> &Sensitive<String> {
        &self.0
    }

    /// Consumes this address, returning the underlying [`Sensitive`] wrapper.
    pub fn into_sensitive(self) -> Sensitive<String> {
        self.0
    }

    /// Exposes the address string for navigation engine dispatch.
    pub fn as_str(&self) -> &str {
        self.0.as_inner().as_str()
    }

    /// Consumes the address, returning the underlying string.
    pub fn into_inner(self) -> String {
        self.0.into_inner()
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address([REDACTED])")
    }
}

/// A page title, protected under FR-007a, FR-023, and FR-039c.
///
/// Holds a page title within [`Sensitive<String>`] with no path to a log stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageTitle(pub Sensitive<String>);

impl PageTitle {
    /// Creates a new sensitive `PageTitle`.
    pub fn new(title: impl Into<String>) -> Self {
        Self(Sensitive::new(title.into()))
    }

    /// Returns a reference to the underlying [`Sensitive`] wrapper.
    pub fn as_sensitive(&self) -> &Sensitive<String> {
        &self.0
    }

    /// Consumes this title, returning the underlying [`Sensitive`] wrapper.
    pub fn into_sensitive(self) -> Sensitive<String> {
        self.0
    }

    /// Exposes the title string for tab title presentation.
    pub fn as_str(&self) -> &str {
        self.0.as_inner().as_str()
    }

    /// Consumes the title, returning the underlying string.
    pub fn into_inner(self) -> String {
        self.0.into_inner()
    }
}

impl fmt::Debug for PageTitle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PageTitle([REDACTED])")
    }
}

/// A search query entered by the member, protected under FR-007a and FR-023.
///
/// Holds a search query within [`Sensitive<String>`] with no path to a log stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SearchTerm(pub Sensitive<String>);

impl SearchTerm {
    /// Creates a new sensitive `SearchTerm`.
    pub fn new(term: impl Into<String>) -> Self {
        Self(Sensitive::new(term.into()))
    }

    /// Returns a reference to the underlying [`Sensitive`] wrapper.
    pub fn as_sensitive(&self) -> &Sensitive<String> {
        &self.0
    }

    /// Consumes this search term, returning the underlying [`Sensitive`] wrapper.
    pub fn into_sensitive(self) -> Sensitive<String> {
        self.0
    }

    /// Exposes the query string for search submission.
    pub fn as_str(&self) -> &str {
        self.0.as_inner().as_str()
    }

    /// Consumes the search term, returning the underlying string.
    pub fn into_inner(self) -> String {
        self.0.into_inner()
    }
}

impl fmt::Debug for SearchTerm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SearchTerm([REDACTED])")
    }
}

/// An account or site credential, protected under FR-023.
///
/// Holds a credential within [`Sensitive<String>`] with no path to a log stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Credential(pub Sensitive<String>);

impl Credential {
    /// Creates a new sensitive `Credential`.
    pub fn new(credential: impl Into<String>) -> Self {
        Self(Sensitive::new(credential.into()))
    }

    /// Returns a reference to the underlying [`Sensitive`] wrapper.
    pub fn as_sensitive(&self) -> &Sensitive<String> {
        &self.0
    }

    /// Consumes this credential, returning the underlying [`Sensitive`] wrapper.
    pub fn into_sensitive(self) -> Sensitive<String> {
        self.0
    }

    /// Exposes the secret value strictly for the OS secure credential store.
    pub fn expose_secret(&self) -> &str {
        self.0.as_inner().as_str()
    }

    /// Consumes the credential, returning the underlying secret string.
    pub fn into_inner(self) -> String {
        self.0.into_inner()
    }
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Credential([REDACTED])")
    }
}

/// A security or session token derived from a credential, protected under FR-023.
///
/// Holds a token within [`Sensitive<String>`] with no path to a log stream.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Token(pub Sensitive<String>);

impl Token {
    /// Creates a new sensitive `Token`.
    pub fn new(token: impl Into<String>) -> Self {
        Self(Sensitive::new(token.into()))
    }

    /// Returns a reference to the underlying [`Sensitive`] wrapper.
    pub fn as_sensitive(&self) -> &Sensitive<String> {
        &self.0
    }

    /// Consumes this token, returning the underlying [`Sensitive`] wrapper.
    pub fn into_sensitive(self) -> Sensitive<String> {
        self.0
    }

    /// Exposes the token value for secure transmission.
    pub fn expose_secret(&self) -> &str {
        self.0.as_inner().as_str()
    }

    /// Consumes the token, returning the underlying string.
    pub fn into_inner(self) -> String {
        self.0.into_inner()
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token([REDACTED])")
    }
}

// -----------------------------------------------------------------------------
// Typed Log Levels and Event Kinds
// -----------------------------------------------------------------------------

/// The severity level of a log record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    /// Fine-grained internal trace.
    Trace,
    /// Diagnostic debugging information.
    Debug,
    /// General operational milestones.
    Info,
    /// Potential issues or non-fatal anomalies.
    Warn,
    /// High-severity operational failures.
    Error,
}

impl Level {
    /// The uppercase identifier for this level.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The closed classification kinds of shell log events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// Navigation lifecycle events.
    Navigation,
    /// UI and rendering surface events.
    Surface,
    /// Member-facing and internal error events.
    Error,
    /// Application and window lifecycle events.
    Lifecycle,
    /// Download lifecycle events.
    Download,
    /// Local state and profile storage events.
    Storage,
    /// Privacy-safe diagnostic reports.
    Diagnostic,
}

impl EventKind {
    /// Machine-readable code for this event kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Navigation => "navigation",
            Self::Surface => "surface",
            Self::Error => "error",
            Self::Lifecycle => "lifecycle",
            Self::Download => "download",
            Self::Storage => "storage",
            Self::Diagnostic => "diagnostic",
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// -----------------------------------------------------------------------------
// Typed Field Values and LogValue Trait
// -----------------------------------------------------------------------------

/// The closed union of typed values permitted in structured log fields.
///
/// Notably, dynamic strings (`String`), addresses, credentials, search terms,
/// and tokens are excluded from this union by design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    /// Boolean flag.
    Bool(bool),
    /// Unsigned 64-bit integer (identifiers, counts, sizes, status codes).
    U64(u64),
    /// Signed 64-bit integer.
    I64(i64),
    /// Elapsed duration in milliseconds.
    DurationMs(u64),
    /// Compile-time static identifier string (never dynamic heap strings).
    StaticStr(&'static str),
    /// Privacy-safe error projection conforming to FR-023 and FR-007a.
    ErrorProjection(LogProjection),
    /// Closed error kind code.
    ErrorKind(ErrorKind),
}

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::U64(val) => write!(f, "{val}"),
            Self::I64(val) => write!(f, "{val}"),
            Self::DurationMs(ms) => write!(f, "{ms}ms"),
            Self::StaticStr(s) => write!(f, "{s}"),
            Self::ErrorProjection(proj) => {
                write!(
                    f,
                    "kind={} cause={} next={}",
                    proj.code(),
                    proj.cause_key(),
                    proj.next_step_key()
                )
            }
            Self::ErrorKind(kind) => write!(f, "{}", kind.as_str()),
        }
    }
}

/// A trait implemented strictly by safe, typed values permitted in log records.
///
/// Types containing addresses, credentials, page titles, search terms, tokens,
/// or free-form strings do NOT implement this trait.
pub trait LogValue: fmt::Debug {
    /// Converts this value into a closed [`FieldValue`].
    fn to_field_value(&self) -> FieldValue;
}

impl LogValue for bool {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::Bool(*self)
    }
}

impl LogValue for u8 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::U64(*self as u64)
    }
}

impl LogValue for u16 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::U64(*self as u64)
    }
}

impl LogValue for u32 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::U64(*self as u64)
    }
}

impl LogValue for u64 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::U64(*self)
    }
}

impl LogValue for usize {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::U64(*self as u64)
    }
}

impl LogValue for i8 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::I64(*self as i64)
    }
}

impl LogValue for i16 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::I64(*self as i64)
    }
}

impl LogValue for i32 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::I64(*self as i64)
    }
}

impl LogValue for i64 {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::I64(*self)
    }
}

impl LogValue for isize {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::I64(*self as i64)
    }
}

impl LogValue for core::time::Duration {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::DurationMs(self.as_millis() as u64)
    }
}

impl LogValue for LogProjection {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::ErrorProjection(*self)
    }
}

impl LogValue for ErrorKind {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::ErrorKind(*self)
    }
}

impl LogValue for &'static str {
    fn to_field_value(&self) -> FieldValue {
        FieldValue::StaticStr(self)
    }
}

// -----------------------------------------------------------------------------
// Typed Field and Record
// -----------------------------------------------------------------------------

/// A single named field in a structured log record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    name: &'static str,
    value: FieldValue,
}

impl Field {
    /// Creates a new field with a static key and a typed [`LogValue`].
    pub fn new<V: LogValue>(name: &'static str, value: V) -> Self {
        Self {
            name,
            value: value.to_field_value(),
        }
    }

    /// The field name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The field's typed value.
    pub fn value(&self) -> &FieldValue {
        &self.value
    }
}

/// A structured log record built entirely from typed fields.
///
/// Unlike conventional logging frameworks, a `Record` has **no free-form message
/// argument**. Every data point is typed and adheres to privacy invariants by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    level: Level,
    event: EventKind,
    timestamp: u64,
    fields: Vec<Field>,
}

impl Record {
    /// Creates a builder for a record with the specified level and event kind.
    pub fn builder(level: Level, event: EventKind) -> RecordBuilder {
        RecordBuilder::new(level, event)
    }

    /// The log level.
    pub fn level(&self) -> Level {
        self.level
    }

    /// The event classification kind.
    pub fn event(&self) -> EventKind {
        self.event
    }

    /// The record timestamp in milliseconds.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// The slice of typed fields carried by this record.
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// Formats the record as a structured, privacy-safe single-line entry.
    pub fn format_line(&self) -> String {
        let mut line = format!(
            "time={} level={} event={}",
            self.timestamp,
            self.level.as_str(),
            self.event.as_str()
        );
        for field in &self.fields {
            line.push(' ');
            line.push_str(field.name());
            line.push('=');
            line.push_str(&field.value().to_string());
        }
        line
    }
}

/// Builder for constructing structured [`Record`] instances.
#[derive(Debug, Clone)]
pub struct RecordBuilder {
    level: Level,
    event: EventKind,
    timestamp: u64,
    fields: Vec<Field>,
}

impl RecordBuilder {
    /// Creates a new builder for the given level and event kind.
    pub fn new(level: Level, event: EventKind) -> Self {
        Self {
            level,
            event,
            timestamp: 0,
            fields: Vec::new(),
        }
    }

    /// Sets the record timestamp (e.g. milliseconds since unix epoch).
    pub fn timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    /// Appends a typed field to the record.
    pub fn field<V: LogValue>(mut self, name: &'static str, value: V) -> Self {
        self.fields.push(Field::new(name, value));
        self
    }

    /// Attaches a surface identifier.
    pub fn surface_id(self, id: u64) -> Self {
        self.field("surface_id", id)
    }

    /// Attaches a navigation identifier.
    pub fn navigation_id(self, id: u64) -> Self {
        self.field("navigation_id", id)
    }

    /// Attaches an HTTP or engine status code.
    pub fn status_code(self, code: u16) -> Self {
        self.field("status_code", code)
    }

    /// Attaches an elapsed duration in milliseconds.
    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.fields.push(Field {
            name: "duration_ms",
            value: FieldValue::DurationMs(ms),
        });
        self
    }

    /// Attaches an elapsed [`core::time::Duration`].
    pub fn duration(self, duration: core::time::Duration) -> Self {
        self.field("duration", duration)
    }

    /// Attaches a byte count.
    pub fn byte_count(self, bytes: u64) -> Self {
        self.field("byte_count", bytes)
    }

    /// Attaches a privacy-safe error log projection.
    pub fn error_projection(self, proj: LogProjection) -> Self {
        self.field("error_projection", proj)
    }

    /// Attaches an error classification kind.
    pub fn error_kind(self, kind: ErrorKind) -> Self {
        self.field("error_kind", kind)
    }

    /// Builds the structured [`Record`].
    pub fn build(self) -> Record {
        Record {
            level: self.level,
            event: self.event,
            timestamp: self.timestamp,
            fields: self.fields,
        }
    }
}

// -----------------------------------------------------------------------------
// Log Sink and Dispatch
// -----------------------------------------------------------------------------

/// A sink that receives structured log records.
pub trait LogSink {
    /// Writes a single structured record.
    fn write_record(&mut self, record: &Record);
}

/// An in-memory sink for testing and buffered capture.
#[derive(Debug, Default, Clone)]
pub struct MemoryLogSink {
    records: Vec<Record>,
}

impl MemoryLogSink {
    /// Creates a new empty memory sink.
    pub fn new() -> Self {
        Self::default()
    }

    /// The records received so far.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Clears all stored records.
    pub fn clear(&mut self) {
        self.records.clear();
    }
}

impl LogSink for MemoryLogSink {
    fn write_record(&mut self, record: &Record) {
        self.records.push(record.clone());
    }
}

/// Emits a structured record to a sink.
///
/// Notice: this function takes `&Record`. Passing an [`Address`], [`Credential`],
/// or [`Sensitive`] directly fails at compile time because types do not match.
pub fn emit(sink: &mut dyn LogSink, record: &Record) {
    sink.write_record(record);
}

// -----------------------------------------------------------------------------
// Unit Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ShellError;

    #[test]
    fn record_builder_produces_typed_records_with_no_message_argument() {
        let record = Record::builder(Level::Info, EventKind::Navigation)
            .timestamp(12345678)
            .surface_id(42)
            .navigation_id(101)
            .status_code(200)
            .duration_ms(15)
            .byte_count(4096)
            .field("action", "commit")
            .build();

        assert_eq!(record.level(), Level::Info);
        assert_eq!(record.event(), EventKind::Navigation);
        assert_eq!(record.timestamp(), 12345678);
        assert_eq!(record.fields().len(), 6);

        let formatted = record.format_line();
        assert!(formatted.contains("level=INFO"));
        assert!(formatted.contains("event=navigation"));
        assert!(formatted.contains("surface_id=42"));
        assert!(formatted.contains("navigation_id=101"));
        assert!(formatted.contains("status_code=200"));
        assert!(formatted.contains("duration_ms=15ms"));
        assert!(formatted.contains("byte_count=4096"));
        assert!(formatted.contains("action=commit"));
    }

    #[test]
    fn sensitive_wrapper_redacts_debug_and_has_no_display() {
        let secret = Sensitive::new("super-secret-token");
        let debug_str = format!("{secret:?}");
        assert_eq!(debug_str, "[REDACTED]");
        assert!(!debug_str.contains("super-secret-token"));
        assert_eq!(secret.as_inner(), &"super-secret-token");
        assert_eq!(secret.into_inner(), "super-secret-token");
    }

    #[test]
    fn sensitive_domain_types_redact_debug_and_protect_inner_secrets() {
        let addr = Address::new("https://secret.example.com/login?token=abc");
        assert_eq!(format!("{addr:?}"), "Address([REDACTED])");
        assert_eq!(addr.as_str(), "https://secret.example.com/login?token=abc");

        let title = PageTitle::new("Confidential Financial Dashboard");
        assert_eq!(format!("{title:?}"), "PageTitle([REDACTED])");
        assert_eq!(title.as_str(), "Confidential Financial Dashboard");

        let term = SearchTerm::new("how to access account");
        assert_eq!(format!("{term:?}"), "SearchTerm([REDACTED])");
        assert_eq!(term.as_str(), "how to access account");

        let cred = Credential::new("correct-horse-battery-staple");
        assert_eq!(format!("{cred:?}"), "Credential([REDACTED])");
        assert_eq!(cred.expose_secret(), "correct-horse-battery-staple");

        let token = Token::new("bearer-tok-12345");
        assert_eq!(format!("{token:?}"), "Token([REDACTED])");
        assert_eq!(token.expose_secret(), "bearer-tok-12345");
    }

    #[test]
    fn on_disk_location_and_retention_policy_constants() {
        assert_eq!(LOG_DIR_NAME, "logs");
        assert_eq!(LOG_FILE_NAME, "evreos.log");
        assert_eq!(LOG_RELATIVE_PATH, "logs/evreos.log");
        assert_eq!(LOG_RETENTION_DAYS, 7);
        assert_eq!(
            LOG_RETENTION,
            core::time::Duration::from_secs(7 * 24 * 60 * 60)
        );

        let profile = Path::new("/var/app/profile");
        assert_eq!(log_directory(profile), profile.join("logs"));
        assert_eq!(
            log_file_path(profile),
            profile.join("logs").join("evreos.log")
        );
    }

    #[test]
    fn error_log_projection_is_safely_loggable_without_address() {
        let error = ShellError::Unresolvable {
            address: "https://nonexistent.invalid".to_string(),
        };
        let projection = error.log_projection();

        let record = Record::builder(Level::Error, EventKind::Error)
            .timestamp(100)
            .error_projection(projection)
            .error_kind(projection.kind())
            .build();

        let formatted = record.format_line();
        assert!(!formatted.contains("nonexistent.invalid"));
        assert!(formatted.contains("kind=unresolvable"));
        assert!(formatted.contains("cause=error.unresolvable.cause"));
        assert!(formatted.contains("next=error.unresolvable.next_step"));

        let mut sink = MemoryLogSink::new();
        emit(&mut sink, &record);
        assert_eq!(sink.records().len(), 1);
        assert_eq!(sink.records()[0], record);
    }
}
