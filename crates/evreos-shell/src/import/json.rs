//! A strict JSON parser for Chromium's `Bookmarks` file, and nothing wider.
//!
//! Chromium keeps bookmarks as one JSON document; this parses it into a value
//! tree the Chromium reader walks. It exists for the reason the SQLite reader
//! does — the measurement at `docs/measurements/import-profile-read.md` states
//! the byte cost of each option — and holds to the same rules: every input is
//! untrusted, so nesting is bounded rather than recursed into without limit,
//! and malformed text is an error value, never a panic.

#![forbid(unsafe_code)]

use std::fmt;

/// Deeper than any bookmark tree a person builds by hand, and shallow enough
/// that the recursive descent below cannot exhaust a worker thread's stack.
const MAX_DEPTH: usize = 256;

/// A parsed JSON value. Numbers keep their text, since the one consumer reads
/// Chromium's timestamps, which it stores as strings anyway.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    /// `null`
    Null,
    /// `true` or `false`
    Bool(bool),
    /// A number, as written.
    Number(String),
    /// A string, unescaped.
    String(String),
    /// An array.
    Array(Vec<Json>),
    /// An object, members in document order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// The member named `key`, if this is an object holding one.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Object(members) => members
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    /// The string, if this is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Why a document is not JSON, with the byte offset it failed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    /// Byte offset into the document.
    pub offset: usize,
    /// What was expected there.
    pub expected: &'static str,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected {} at byte {}", self.expected, self.offset)
    }
}

impl std::error::Error for JsonError {}

/// Parse one JSON document. A UTF-8 byte-order mark is skipped; trailing
/// content after the document is an error.
pub fn parse(text: &str) -> Result<Json, JsonError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut parser = Parser {
        bytes: text.as_bytes(),
        text,
        at: 0,
    };
    let value = parser.value(0)?;
    parser.whitespace();
    if parser.at != parser.bytes.len() {
        return Err(parser.error("the end of the document"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn error(&self, expected: &'static str) -> JsonError {
        JsonError {
            offset: self.at,
            expected,
        }
    }

    fn whitespace(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.at) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8, expected: &'static str) -> Result<(), JsonError> {
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(expected))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("a value"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.error("nesting no deeper than 256"));
        }
        self.whitespace();
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Json::String(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("a value")),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1;
        let mut members = Vec::new();
        self.whitespace();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.whitespace();
            if self.bytes.get(self.at) != Some(&b'"') {
                return Err(self.error("a member name"));
            }
            let name = self.string()?;
            self.whitespace();
            self.eat(b':', "':'")?;
            let value = self.value(depth + 1)?;
            members.push((name, value));
            self.whitespace();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(self.error("',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, JsonError> {
        self.at += 1;
        let mut items = Vec::new();
        self.whitespace();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.whitespace();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.error("',' or ']'")),
            }
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        let digits = |parser: &mut Self| {
            let from = parser.at;
            while parser.bytes.get(parser.at).is_some_and(u8::is_ascii_digit) {
                parser.at += 1;
            }
            parser.at > from
        };
        if !digits(self) {
            return Err(self.error("a digit"));
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            if !digits(self) {
                return Err(self.error("a digit after '.'"));
            }
        }
        if let Some(b'e' | b'E') = self.bytes.get(self.at) {
            self.at += 1;
            if let Some(b'+' | b'-') = self.bytes.get(self.at) {
                self.at += 1;
            }
            if !digits(self) {
                return Err(self.error("an exponent"));
            }
        }
        Ok(Json::Number(self.text[start..self.at].to_string()))
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let run = self.at;
            while let Some(&byte) = self.bytes.get(self.at) {
                if byte == b'"' || byte == b'\\' || byte < 0x20 {
                    break;
                }
                self.at += 1;
            }
            // `run..at` stops only at ASCII bytes, so it is a char boundary.
            out.push_str(&self.text[run..self.at]);
            match self.bytes.get(self.at) {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    let escape = *self
                        .bytes
                        .get(self.at)
                        .ok_or_else(|| self.error("an escape"))?;
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()?),
                        _ => return Err(self.error("a valid escape")),
                    }
                }
                _ => return Err(self.error("a closing '\"'")),
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, JsonError> {
        let digits = self
            .text
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("four hex digits"))?;
        let value = u16::from_str_radix(digits, 16).map_err(|_| self.error("four hex digits"))?;
        self.at += 4;
        Ok(value)
    }

    /// A `\uXXXX` escape, joining a surrogate pair; a lone surrogate becomes
    /// U+FFFD, as Chromium itself writes one it could not encode.
    fn unicode_escape(&mut self) -> Result<char, JsonError> {
        let first = self.hex4()?;
        if (0xd800..0xdc00).contains(&first) && self.bytes[self.at..].starts_with(b"\\u") {
            let save = self.at;
            self.at += 2;
            let second = self.hex4()?;
            if (0xdc00..0xe000).contains(&second) {
                let code =
                    0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
                return Ok(char::from_u32(code).unwrap_or('\u{fffd}'));
            }
            self.at = save;
        }
        Ok(char::from_u32(u32::from(first)).unwrap_or('\u{fffd}'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_parse_into_values() {
        let doc = parse(r#"{"a": [1, -2.5e3, true, false, null], "b": "x\"y\u00e9\ud83d\ude00"}"#)
            .unwrap();
        assert_eq!(
            doc.get("a").and_then(Json::as_array).map(<[Json]>::len),
            Some(5)
        );
        assert_eq!(doc.get("b").and_then(Json::as_str), Some("x\"yé😀"));
    }

    #[test]
    fn malformed_documents_are_errors_not_panics() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\" 1}",
            "\"unterminated",
            "tru",
            "01x",
            "{} {}",
            "\"\\u12\"",
            "\"\\q\"",
            "-",
            "1.",
            "1e",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn nesting_is_bounded() {
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep).is_err());
        let fine = "[".repeat(10) + &"]".repeat(10);
        assert!(parse(&fine).is_ok());
    }

    #[test]
    fn a_byte_order_mark_is_skipped_and_a_lone_surrogate_is_replaced() {
        assert_eq!(
            parse("\u{feff}\"\\ud800x\"").unwrap(),
            Json::String("\u{fffd}x".into())
        );
    }
}
