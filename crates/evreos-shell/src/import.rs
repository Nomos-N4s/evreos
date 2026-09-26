//! FR-012 import: bookmarks and history from Chrome, Firefox and Edge.
//!
//! This module grows with the import. It holds, so far, a read-only reader of
//! the SQLite file format the browsers keep their stores in, and a parser for
//! the JSON document Chromium keeps bookmarks in.

#![forbid(unsafe_code)]

pub mod json;
pub mod sqlite;
