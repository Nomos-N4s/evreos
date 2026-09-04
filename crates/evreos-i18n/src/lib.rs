//! Interface text, keyed by language alone.
//!
//! FR-035 requires interface text in German, Greek and English, keyed by the
//! BCP-47 primary language subtag alone — `de`, `el`, `en` — with no region
//! subtag in the key and place never fused into the language value. Two
//! consequences shape everything here.
//!
//! The catalogue key type is a closed enum. [`Language`] has exactly three
//! variants and no other type opens a catalogue — never a `LanguageIdentifier`
//! or any parsed-tag type, because such a type carries a region field and
//! would re-admit `de-DE` through the front door FR-035 closes. Adding a
//! language is a change to this enum, visible in review, and the region
//! cannot be spelled at all.
//!
//! Language and place are two values. [`Place`] exists so that everything
//! downstream — stored preferences, interface state, every request to an
//! Apivo service — carries the two side by side, as FR-035 requires
//! everywhere either appears. It is deliberately not a parameter of anything
//! in this crate: a catalogue is opened by [`Language`] alone, a message key
//! never names a place, and a `Place` has no route into either.
//!
//! The catalogues are compiled into the binary. FR-016a's neutral menu entry
//! is drawn from them and must render at first run — no account, no network,
//! no Apivo state — so resolution is a pure function over embedded text and
//! this crate has no dependencies and performs no I/O. Messages interpolate
//! **named arguments**: a brand name reaches a message as an argument at the
//! call site, never as text of a catalogue file, which is how FR-042's rule
//! that no brand name is hardcoded outside the brand configuration holds here.
//!
//! The catalogue file format is provisional. N10 — plain keyed table against
//! Fluent — is settled by the measurement recorded at
//! `specs/001-evreos-v1/measurements/n10-catalogue-format.md`, so nothing in
//! this crate's API names a format, and the embedded files carry a
//! format-neutral name until that measurement adopts one.

#![forbid(unsafe_code)]

use core::fmt;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The languages interface text exists in, closed.
///
/// FR-035 names the three; a fourth is an amendment to that requirement and
/// lands as a fourth variant here, with its catalogue, in the same change.
/// The variants carry no region on purpose: this enum is the only catalogue
/// key type, so a region subtag in a catalogue key is unrepresentable rather
/// than merely forbidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// German, `de`.
    De,
    /// Greek, `el`.
    El,
    /// English, `en`.
    En,
}

impl Language {
    /// Every language, in catalogue order. Tests iterate this rather than
    /// naming the variants, so a fourth language cannot be added without
    /// every closure property being re-proved over it.
    pub const ALL: [Language; 3] = [Language::De, Language::El, Language::En];

    /// The BCP-47 primary language subtag, alone. This is the only spelling
    /// of a language this crate emits, so anything that serialises a
    /// language — a stored preference, a request parameter — gets `de`,
    /// never `de-DE`.
    pub fn subtag(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::El => "el",
            Language::En => "en",
        }
    }
}

/// Where the member is, as a value of its own.
///
/// FR-035: language and place are two separate values wherever either
/// appears. This type is the second value. It holds an ISO 3166-1 alpha-2
/// style code — two ASCII uppercase letters — and nothing in this crate
/// accepts one: no catalogue key, no message key, no resolution parameter.
/// It lives here so that the crate that defines what a language is also
/// defines what a place is, and no later crate invents a fused type because
/// the unfused pair was not to hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place(String);

impl Place {
    /// Build a place from its two-letter uppercase code.
    ///
    /// The shape is validated because a `Place` that could hold `de` — or a
    /// whole fused tag — would be a language smuggled into the place slot,
    /// the inverse of the fusion FR-035 forbids.
    pub fn new(code: &str) -> Result<Place, PlaceError> {
        if code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            Ok(Place(code.to_owned()))
        } else {
            Err(PlaceError {
                code: code.to_owned(),
            })
        }
    }

    /// The code, exactly as validated.
    pub fn code(&self) -> &str {
        &self.0
    }
}

/// A string that is not a place code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceError {
    code: String,
}

impl fmt::Display for PlaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not a place: a place is two ASCII uppercase letters, \
             carried beside a language and never inside it",
            self.code
        )
    }
}

impl core::error::Error for PlaceError {}

/// Why a catalogue's text is not a catalogue.
///
/// Every variant names the line that caused it, so a defective catalogue is
/// locatable from the message alone. These are defects of the tree, not
/// runtime conditions: the catalogues are compiled in, and
/// `tests/catalogues.rs` parses all three before a build ships.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueError {
    /// A line that is neither blank, a comment, nor `key = text`.
    MissingSeparator { line: usize },
    /// A key with a character outside lowercase ASCII, `.` and `_`.
    ///
    /// The charset is the FR-035 rule made unrepresentable: a region subtag
    /// needs an uppercase pair, digits or a hyphen, and none of the four can
    /// be spelled. It also keeps a `Place` code out of a key by construction,
    /// since a place is uppercase and a key cannot be.
    InvalidKey { line: usize, key: String },
    /// A key defined twice. The second definition would silently shadow the
    /// first in review, so it is refused instead.
    DuplicateKey { line: usize, key: String },
    /// A message with no text. A blank message is not a translation; the
    /// missing-key state must stay visible rather than resolve to nothing.
    EmptyMessage { line: usize, key: String },
    /// A `{` without a matching `}`, an empty `{}`, an argument name with a
    /// character outside lowercase ASCII and `_`, or a stray `}`. Braces
    /// delimit named arguments and do nothing else; there is no escape,
    /// because no message needs a literal brace and an escape rule is one
    /// more thing a translator can get silently wrong.
    InvalidPlaceholder { line: usize, key: String },
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogueError::MissingSeparator { line } => {
                write!(f, "line {line}: not blank, not a comment, not `key = text`")
            }
            CatalogueError::InvalidKey { line, key } => {
                write!(
                    f,
                    "line {line}: key {key:?} strays outside lowercase ASCII, `.` \
                     and `_`; FR-035 admits no region subtag and no place in a key"
                )
            }
            CatalogueError::DuplicateKey { line, key } => {
                write!(f, "line {line}: key {key:?} is defined twice")
            }
            CatalogueError::EmptyMessage { line, key } => {
                write!(f, "line {line}: key {key:?} has no text")
            }
            CatalogueError::InvalidPlaceholder { line, key } => {
                write!(
                    f,
                    "line {line}: key {key:?} has a malformed argument \
                     placeholder; braces delimit `{{name}}` and nothing else"
                )
            }
        }
    }
}

impl core::error::Error for CatalogueError {}

/// Why a message did not resolve.
///
/// Failure is a value here for the reason it is on the engine seam: a shell
/// that asked for a key no catalogue holds, or withheld an argument a message
/// names, has a defect to surface, and a resolver that silently returned the
/// key or a half-filled template would hide it as rendered text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// No message under this key. The completeness test in
    /// `tests/catalogues.rs` makes this equally true or false in all three
    /// catalogues, so it cannot depend on the member's language.
    UnknownKey { key: String },
    /// The message names an argument the caller did not supply. Named, so
    /// the failure says which — a brand name a call site forgot is found
    /// from the message rather than by diffing templates.
    MissingArgument { key: String, name: String },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::UnknownKey { key } => {
                write!(f, "no message under {key:?} in any catalogue")
            }
            ResolveError::MissingArgument { key, name } => {
                write!(
                    f,
                    "message {key:?} names an argument {name:?} the caller did not supply"
                )
            }
        }
    }
}

impl core::error::Error for ResolveError {}

/// One piece of a parsed message: literal text, or a named argument.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Text(String),
    Argument(String),
}

/// One language's messages, parsed.
///
/// Sorted storage, so [`Catalogue::keys`] iterates in one order everywhere
/// and a test's report of a missing key is stable.
#[derive(Debug)]
pub struct Catalogue {
    language: Language,
    messages: BTreeMap<String, Vec<Piece>>,
}

impl Catalogue {
    /// Parse catalogue text for `language`.
    ///
    /// Public so the tests can prove what this parser refuses — a region
    /// subtag in a key above all — not so that catalogues can arrive at
    /// runtime: the three this crate serves are compiled in, and text from
    /// anywhere else is not interface text under FR-035.
    pub fn parse(language: Language, text: &str) -> Result<Catalogue, CatalogueError> {
        let mut messages = BTreeMap::new();
        for (line, raw) in text.lines().enumerate() {
            let line = line + 1;
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, message)) = trimmed.split_once('=') else {
                return Err(CatalogueError::MissingSeparator { line });
            };
            let key = key.trim();
            let message = message.trim();
            if key.is_empty()
                || !key
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'.' || byte == b'_')
            {
                return Err(CatalogueError::InvalidKey {
                    line,
                    key: key.to_owned(),
                });
            }
            if message.is_empty() {
                return Err(CatalogueError::EmptyMessage {
                    line,
                    key: key.to_owned(),
                });
            }
            let pieces = parse_message(message).ok_or(CatalogueError::InvalidPlaceholder {
                line,
                key: key.to_owned(),
            })?;
            if messages.insert(key.to_owned(), pieces).is_some() {
                return Err(CatalogueError::DuplicateKey {
                    line,
                    key: key.to_owned(),
                });
            }
        }
        Ok(Catalogue { language, messages })
    }

    /// The language this catalogue serves.
    pub fn language(&self) -> Language {
        self.language
    }

    /// Every message key, in one stable order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(String::as_str)
    }

    /// The named arguments the message under `key` interpolates, in order of
    /// first appearance.
    ///
    /// This is how a caller — and the completeness test — knows what a
    /// message requires without guessing: [`Catalogue::resolve`] fails on a
    /// missing argument, and this is the list it fails against.
    pub fn arguments(&self, key: &str) -> Result<Vec<&str>, ResolveError> {
        let pieces = self
            .messages
            .get(key)
            .ok_or_else(|| ResolveError::UnknownKey {
                key: key.to_owned(),
            })?;
        let mut names = Vec::new();
        for piece in pieces {
            if let Piece::Argument(name) = piece {
                if !names.contains(&name.as_str()) {
                    names.push(name.as_str());
                }
            }
        }
        Ok(names)
    }

    /// Resolve the message under `key`, interpolating `arguments` by name.
    ///
    /// A supplied argument no message names is ignored — the caller may hold
    /// one set of values for several messages — but an argument a message
    /// names and the caller omits is an error, because rendering a template
    /// with a hole where a brand name belongs is FR-042's rule half-kept.
    pub fn resolve(&self, key: &str, arguments: &[(&str, &str)]) -> Result<String, ResolveError> {
        let pieces = self
            .messages
            .get(key)
            .ok_or_else(|| ResolveError::UnknownKey {
                key: key.to_owned(),
            })?;
        let mut resolved = String::new();
        for piece in pieces {
            match piece {
                Piece::Text(text) => resolved.push_str(text),
                Piece::Argument(name) => {
                    let value = arguments
                        .iter()
                        .find(|(supplied, _)| supplied == name)
                        .map(|(_, value)| *value)
                        .ok_or_else(|| ResolveError::MissingArgument {
                            key: key.to_owned(),
                            name: name.clone(),
                        })?;
                    resolved.push_str(value);
                }
            }
        }
        Ok(resolved)
    }
}

/// Split a message into literal text and named arguments, or `None` when its
/// braces do not spell `{name}` and nothing else.
fn parse_message(message: &str) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut text = String::new();
    let mut characters = message.chars();
    while let Some(character) = characters.next() {
        match character {
            '{' => {
                let mut name = String::new();
                loop {
                    match characters.next()? {
                        '}' => break,
                        inner if inner.is_ascii_lowercase() || inner == '_' => name.push(inner),
                        _ => return None,
                    }
                }
                if name.is_empty() {
                    return None;
                }
                if !text.is_empty() {
                    pieces.push(Piece::Text(std::mem::take(&mut text)));
                }
                pieces.push(Piece::Argument(name));
            }
            '}' => return None,
            _ => text.push(character),
        }
    }
    if !text.is_empty() {
        pieces.push(Piece::Text(text));
    }
    Some(pieces)
}

// The three catalogues, embedded at build time. `include_str!` is what makes
// FR-016a's first-run guarantee structural: the text is in the binary, so
// there is nothing to fetch, nothing to install and nothing a missing profile
// can withhold. The filenames carry the format-neutral `.catalogue` name
// until the N10 measurement adopts a format; renaming them is that
// measurement's task and touches these three lines and nothing else.
const DE_TEXT: &str = include_str!("../catalogues/de.catalogue");
const EL_TEXT: &str = include_str!("../catalogues/el.catalogue");
const EN_TEXT: &str = include_str!("../catalogues/en.catalogue");

static DE: OnceLock<Catalogue> = OnceLock::new();
static EL: OnceLock<Catalogue> = OnceLock::new();
static EN: OnceLock<Catalogue> = OnceLock::new();

/// The catalogue for `language`, parsed once per process.
///
/// The `expect` is deliberate and is not a runtime failure mode: the text it
/// parses is compiled into this binary, so a parse failure is a defective
/// tree, and `tests/catalogues.rs` parses all three catalogues before any
/// build carrying them is cut. A `Result` here would hand every call site a
/// failure that cannot occur at runtime and that none of them could render —
/// this crate is where the rendering text comes from.
pub fn catalogue(language: Language) -> &'static Catalogue {
    let (cell, text) = match language {
        Language::De => (&DE, DE_TEXT),
        Language::El => (&EL, EL_TEXT),
        Language::En => (&EN, EN_TEXT),
    };
    cell.get_or_init(|| {
        Catalogue::parse(language, text).unwrap_or_else(|error| {
            panic!(
                "the embedded {} catalogue does not parse ({error}); this is a \
                 defect of the tree, caught by tests/catalogues.rs before release",
                language.subtag()
            )
        })
    })
}
