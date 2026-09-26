//! FR-012 import: bookmarks and history from Chrome, Firefox and Edge.
//!
//! This module grows with the import. It holds, so far, a read-only reader of
//! the SQLite file format the browsers keep their stores in, a parser for the
//! JSON document Chromium keeps bookmarks in, and a verified copy of a store
//! its browser may be writing.

#![forbid(unsafe_code)]

pub mod json;
pub mod snapshot;
pub mod sqlite;
