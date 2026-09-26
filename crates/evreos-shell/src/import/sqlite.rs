//! A read-only reader for the SQLite file format, over bytes already in memory.
//!
//! FR-012 needs three tables out of two browsers' SQLite stores — Chromium's
//! `urls`, Firefox's `moz_places` and `moz_bookmarks` — and nothing else a
//! database engine does. The import-profile-read measurement at
//! `docs/measurements/import-profile-read.md` records why this reader exists
//! rather than a binding to the SQLite library: every one of the three
//! browsers holds its store under an exclusive lock while it runs, so the
//! library cannot open it at all, and the store has to be copied before it is
//! read either way. Once the bytes are copied, reading them needs the file
//! format and not the engine, and the format alone costs a fraction of the
//! engine's bytes against SC-001.
//!
//! What this reader does, and deliberately nothing more:
//!
//! - the database header, the page size and the text encoding;
//! - a write-ahead log applied over the main file, honouring only frames up
//!   to its last valid commit, exactly as SQLite's own recovery does, so a
//!   store whose recent writes are still in its `-wal` file reads with them;
//! - table b-trees (interior and leaf pages), overflow chains and the record
//!   format, so any row of a rowid table can be read;
//! - the schema table, and enough of `CREATE TABLE` to find a column by name.
//!
//! It never writes, never takes a lock, never follows a pointer it has not
//! bounds-checked, and never trusts a page graph to be acyclic: a malformed
//! or adversarial store is an error value, never a panic or an endless walk.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fmt;

/// The 16-byte string every SQLite database file begins with.
const HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";
/// The size of the database header on page 1.
const HEADER_LEN: usize = 100;
/// The write-ahead log header, and each frame's header.
const WAL_HEADER_LEN: usize = 32;
const WAL_FRAME_HEADER_LEN: usize = 24;
/// Deeper than any real table b-tree: a four-level tree of 512-byte pages
/// already addresses more rows than a browser profile holds.
const MAX_TREE_DEPTH: usize = 40;

/// Why a store could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SqliteError {
    /// The bytes are not an SQLite database, or its header is inconsistent.
    NotADatabase(String),
    /// A page, cell, record or overflow chain points outside the file or
    /// back into itself.
    Corrupt(String),
    /// The schema names no table by that name.
    NoSuchTable(String),
    /// The table has no column by that name.
    NoSuchColumn(String),
    /// A feature this reader does not implement, named.
    Unsupported(String),
}

impl fmt::Display for SqliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADatabase(why) => write!(f, "not an SQLite database: {why}"),
            Self::Corrupt(why) => write!(f, "corrupt SQLite database: {why}"),
            Self::NoSuchTable(name) => write!(f, "no table named {name}"),
            Self::NoSuchColumn(name) => write!(f, "no column named {name}"),
            Self::Unsupported(what) => write!(f, "unsupported SQLite feature: {what}"),
        }
    }
}

impl std::error::Error for SqliteError {}

fn corrupt(why: impl Into<String>) -> SqliteError {
    SqliteError::Corrupt(why.into())
}

/// The encoding the database stores text in, from header offset 56.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

/// One value of one column of one row.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL, and the value of a column a row predates.
    Null,
    /// A signed integer of any stored width.
    Integer(i64),
    /// An IEEE 754 double.
    Real(f64),
    /// Text, decoded from the database's encoding.
    Text(String),
    /// Raw bytes.
    Blob(Vec<u8>),
}

impl Value {
    /// The value as an integer, if it is one.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// The value as text, if it is text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
}

/// A table found in the schema: where its b-tree starts and what its columns
/// are called.
#[derive(Debug, Clone)]
pub struct Table {
    name: String,
    root_page: u32,
    columns: Vec<String>,
    /// The column declared `INTEGER PRIMARY KEY`, which the record stores as
    /// NULL because its value is the rowid.
    rowid_alias: Option<usize>,
}

impl Table {
    /// The index of the named column, compared case-insensitively as SQL does.
    pub fn column(&self, name: &str) -> Result<usize, SqliteError> {
        self.columns
            .iter()
            .position(|column| column.eq_ignore_ascii_case(name))
            .ok_or_else(|| SqliteError::NoSuchColumn(format!("{}.{name}", self.name)))
    }
}

/// One row, with its rowid and its values in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    rowid: i64,
    values: Vec<Value>,
}

impl Row {
    /// The row's rowid.
    pub fn rowid(&self) -> i64 {
        self.rowid
    }

    /// The value at column `index`. A column added after the row was written
    /// reads as NULL, as SQLite reads it when the column has no default.
    pub fn get(&self, index: usize) -> &Value {
        self.values.get(index).unwrap_or(&Value::Null)
    }
}

/// A database opened over its main file's bytes and, optionally, its
/// write-ahead log's.
#[derive(Debug)]
pub struct Database<'a> {
    main: &'a [u8],
    wal: Option<&'a [u8]>,
    /// For each page the log holds, the offset of its newest committed image.
    wal_pages: HashMap<u32, usize>,
    page_size: usize,
    usable_size: usize,
    page_count: u32,
    encoding: TextEncoding,
}

impl<'a> Database<'a> {
    /// Open a database over `main` and, when the store has one, `wal`.
    ///
    /// The log is applied only up to its last valid commit frame: frames
    /// whose salts do not match the log header, whose checksum chain breaks,
    /// or which follow the last commit are ignored, which is what makes a log
    /// copied mid-append read as the last committed state rather than as a
    /// torn one.
    pub fn open(main: &'a [u8], wal: Option<&'a [u8]>) -> Result<Self, SqliteError> {
        let wal = wal.filter(|bytes| !bytes.is_empty());
        let log = match wal {
            Some(bytes) => parse_wal(bytes)?,
            None => None,
        };

        if main.is_empty() {
            // A database whose every page is still in its log, never
            // checkpointed: the log is the whole store.
            let Some(log) = log else {
                return Err(SqliteError::NotADatabase("the file is empty".into()));
            };
            let mut db = Self {
                main,
                wal,
                wal_pages: log.pages,
                page_size: log.page_size,
                usable_size: log.page_size,
                page_count: log.page_count,
                encoding: TextEncoding::Utf8,
            };
            db.read_header_fields()?;
            return Ok(db);
        }

        if main.len() < HEADER_LEN || &main[..16] != HEADER_MAGIC {
            return Err(SqliteError::NotADatabase(
                "the header string is missing".into(),
            ));
        }
        let page_size = decode_page_size(u16::from_be_bytes([main[16], main[17]]))?;
        let file_pages = u32::try_from(main.len() / page_size)
            .map_err(|_| corrupt("the file holds more pages than SQLite can address"))?;
        let header_pages = be_u32(main, 28)?;
        let change_counter = be_u32(main, 24)?;
        let valid_for = be_u32(main, 92)?;
        // The in-header size is authoritative only when it was written by the
        // same version that last changed the file; otherwise the file length
        // is, as the format specification states.
        let mut page_count = if header_pages != 0 && change_counter == valid_for {
            header_pages
        } else {
            file_pages
        };

        let mut wal_pages = HashMap::new();
        if let Some(log) = log {
            if log.page_size != page_size {
                return Err(corrupt(format!(
                    "the write-ahead log's page size {} differs from the database's {page_size}",
                    log.page_size
                )));
            }
            page_count = log.page_count;
            wal_pages = log.pages;
        }

        let mut db = Self {
            main,
            wal,
            wal_pages,
            page_size,
            usable_size: page_size,
            page_count,
            encoding: TextEncoding::Utf8,
        };
        db.read_header_fields()?;
        Ok(db)
    }

    /// Read the reserved-space and encoding fields from page 1 as the log
    /// leaves it, since a log can rewrite the header too.
    fn read_header_fields(&mut self) -> Result<(), SqliteError> {
        let page1 = self.page(1)?;
        if page1.len() < HEADER_LEN || &page1[..16] != HEADER_MAGIC {
            return Err(SqliteError::NotADatabase(
                "page 1 carries no database header".into(),
            ));
        }
        let reserved = usize::from(page1[20]);
        let usable = self
            .page_size
            .checked_sub(reserved)
            .filter(|usable| *usable >= 480)
            .ok_or_else(|| corrupt("the reserved space leaves under 480 usable bytes"))?;
        let encoding = match be_u32(page1, 56)? {
            0 | 1 => TextEncoding::Utf8,
            2 => TextEncoding::Utf16Le,
            3 => TextEncoding::Utf16Be,
            other => return Err(corrupt(format!("unknown text encoding {other}"))),
        };
        self.usable_size = usable;
        self.encoding = encoding;
        Ok(())
    }

    /// The bytes of page `number`, counting from 1, as of the last commit.
    fn page(&self, number: u32) -> Result<&'a [u8], SqliteError> {
        if number == 0 || number > self.page_count {
            return Err(corrupt(format!(
                "page {number} is outside the database's {} pages",
                self.page_count
            )));
        }
        if let (Some(wal), Some(&offset)) = (self.wal, self.wal_pages.get(&number)) {
            return wal
                .get(offset..offset + self.page_size)
                .ok_or_else(|| corrupt(format!("page {number}'s log frame is truncated")));
        }
        let start = (number as usize - 1) * self.page_size;
        self.main
            .get(start..start + self.page_size)
            .ok_or_else(|| corrupt(format!("page {number} lies past the end of the file")))
    }

    /// Find a table by name in the schema table.
    pub fn table(&self, name: &str) -> Result<Table, SqliteError> {
        let schema = Table {
            name: "sqlite_schema".into(),
            root_page: 1,
            columns: ["type", "name", "tbl_name", "rootpage", "sql"]
                .map(String::from)
                .to_vec(),
            rowid_alias: None,
        };
        let mut found = None;
        self.scan(&schema, |row| {
            let is_table = row.get(0).as_text() == Some("table");
            let matches = row
                .get(1)
                .as_text()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name));
            if is_table && matches && found.is_none() {
                found = Some(row);
            }
            Ok(())
        })?;
        let row = found.ok_or_else(|| SqliteError::NoSuchTable(name.into()))?;

        let root_page = row
            .get(3)
            .as_integer()
            .and_then(|page| u32::try_from(page).ok())
            .filter(|page| *page != 0)
            .ok_or_else(|| corrupt(format!("table {name} has no root page")))?;
        let sql = row
            .get(4)
            .as_text()
            .ok_or_else(|| corrupt(format!("table {name} has no definition")))?;
        let (columns, rowid_alias) = parse_create_table(sql)?;
        Ok(Table {
            name: name.into(),
            root_page,
            columns,
            rowid_alias,
        })
    }

    /// Visit every row of `table` in rowid order.
    pub fn scan(
        &self,
        table: &Table,
        mut visit: impl FnMut(Row) -> Result<(), SqliteError>,
    ) -> Result<(), SqliteError> {
        let mut seen = HashSet::new();
        // An explicit stack rather than recursion, so a deep or cyclic page
        // graph is an error value rather than a stack overflow.
        let mut stack = vec![(table.root_page, 0usize)];
        while let Some((number, depth)) = stack.pop() {
            if depth > MAX_TREE_DEPTH {
                return Err(corrupt(format!("table {} is too deep", table.name)));
            }
            if !seen.insert(number) {
                return Err(corrupt(format!(
                    "page {number} is reached twice in table {}",
                    table.name
                )));
            }
            let page = self.page(number)?;
            let base = if number == 1 { HEADER_LEN } else { 0 };
            let kind = *page
                .get(base)
                .ok_or_else(|| corrupt(format!("page {number} has no header")))?;
            let cells = usize::from(be_u16(page, base + 3)?);
            match kind {
                0x05 => {
                    // Interior table page: children in key order, then the
                    // right-most child. Pushed in reverse so they pop in order.
                    let right = be_u32(page, base + 8)?;
                    stack.push((right, depth + 1));
                    for index in (0..cells).rev() {
                        let cell = cell_offset(page, base + 12, index, number)?;
                        stack.push((be_u32(page, cell)?, depth + 1));
                    }
                }
                0x0d => {
                    for index in 0..cells {
                        let cell = cell_offset(page, base + 8, index, number)?;
                        let row = self.leaf_row(page, cell, number, table)?;
                        visit(row)?;
                    }
                }
                0x02 | 0x0a => {
                    return Err(SqliteError::Unsupported(format!(
                        "table {} is stored as an index b-tree (WITHOUT ROWID)",
                        table.name
                    )));
                }
                other => {
                    return Err(corrupt(format!(
                        "page {number} has unknown b-tree page type {other:#04x}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Decode the row stored in the leaf cell at `cell` of page `number`.
    fn leaf_row(
        &self,
        page: &[u8],
        cell: usize,
        number: u32,
        table: &Table,
    ) -> Result<Row, SqliteError> {
        let (payload_len, used) = varint(page, cell)?;
        let (rowid, used_rowid) = varint(page, cell + used)?;
        let payload_len = usize::try_from(payload_len)
            .ok()
            .filter(|len| *len <= self.page_count as usize * self.page_size)
            .ok_or_else(|| corrupt(format!("a cell on page {number} claims an absurd size")))?;
        let start = cell + used + used_rowid;
        let payload = self.payload(page, start, payload_len, number)?;
        let mut values = decode_record(&payload, self.encoding)?;
        if let Some(alias) = table.rowid_alias {
            if let Some(slot) = values.get_mut(alias) {
                if *slot == Value::Null {
                    *slot = Value::Integer(rowid as i64);
                }
            }
        }
        Ok(Row {
            rowid: rowid as i64,
            values,
        })
    }

    /// Assemble a cell's payload: its local part, then its overflow chain.
    fn payload(
        &self,
        page: &[u8],
        start: usize,
        len: usize,
        number: u32,
    ) -> Result<Vec<u8>, SqliteError> {
        let usable = self.usable_size;
        let max_local = usable - 35;
        let local = if len <= max_local {
            len
        } else {
            let min_local = (usable - 12) * 32 / 255 - 23;
            let spill = min_local + (len - min_local) % (usable - 4);
            if spill <= max_local { spill } else { min_local }
        };
        let mut out = Vec::with_capacity(len);
        out.extend_from_slice(
            page.get(start..start + local)
                .ok_or_else(|| corrupt(format!("a cell on page {number} overruns its page")))?,
        );
        if local == len {
            return Ok(out);
        }

        let mut next = be_u32(page, start + local)?;
        let mut hops = 0u32;
        while out.len() < len {
            hops += 1;
            if next == 0 || hops > self.page_count {
                return Err(corrupt(format!(
                    "an overflow chain from page {number} ends early or loops"
                )));
            }
            let overflow = self.page(next)?;
            let take = (len - out.len()).min(usable - 4);
            out.extend_from_slice(
                overflow
                    .get(4..4 + take)
                    .ok_or_else(|| corrupt("an overflow page is truncated"))?,
            );
            next = be_u32(overflow, 0)?;
        }
        Ok(out)
    }
}

/// The parsed content of a write-ahead log.
struct Log {
    page_size: usize,
    page_count: u32,
    pages: HashMap<u32, usize>,
}

/// Parse a write-ahead log, keeping only frames up to its last valid commit.
///
/// Returns `None` for a log that holds no committed frame — one reset by a
/// checkpoint, or one whose header was never completed — because such a log
/// adds nothing to the main file, which is how SQLite treats it too.
fn parse_wal(bytes: &[u8]) -> Result<Option<Log>, SqliteError> {
    if bytes.len() < WAL_HEADER_LEN {
        return Ok(None);
    }
    let magic = be_u32(bytes, 0)?;
    let big_endian = match magic {
        0x377f_0682 => false,
        0x377f_0683 => true,
        _ => return Ok(None),
    };
    let version = be_u32(bytes, 4)?;
    if version != 3_007_000 {
        return Err(SqliteError::Unsupported(format!(
            "write-ahead log format {version}"
        )));
    }
    let page_size = decode_page_size_wal(be_u32(bytes, 8)?)?;
    let salt = (be_u32(bytes, 16)?, be_u32(bytes, 20)?);
    let mut sum = wal_checksum(&bytes[..24], big_endian, (0, 0));
    if sum != (be_u32(bytes, 24)?, be_u32(bytes, 28)?) {
        // A header whose checksum fails is a log SQLite would discard whole.
        return Ok(None);
    }

    let frame_len = WAL_FRAME_HEADER_LEN + page_size;
    let mut pending = Vec::new();
    let mut committed = HashMap::new();
    let mut page_count = None;
    let mut offset = WAL_HEADER_LEN;
    while offset + frame_len <= bytes.len() {
        let frame = &bytes[offset..offset + frame_len];
        let page = be_u32(frame, 0)?;
        let commit_size = be_u32(frame, 4)?;
        if (be_u32(frame, 8)?, be_u32(frame, 12)?) != salt || page == 0 {
            break;
        }
        sum = wal_checksum(&frame[..8], big_endian, sum);
        sum = wal_checksum(&frame[WAL_FRAME_HEADER_LEN..], big_endian, sum);
        if sum != (be_u32(frame, 16)?, be_u32(frame, 20)?) {
            break;
        }
        pending.push((page, offset + WAL_FRAME_HEADER_LEN));
        if commit_size != 0 {
            committed.extend(pending.drain(..));
            page_count = Some(commit_size);
        }
        offset += frame_len;
    }

    Ok(page_count.map(|page_count| Log {
        page_size,
        page_count,
        pages: committed,
    }))
}

/// SQLite's write-ahead-log checksum: two running 32-bit sums over pairs of
/// words in the byte order the log's magic number names.
fn wal_checksum(data: &[u8], big_endian: bool, seed: (u32, u32)) -> (u32, u32) {
    let (mut s0, mut s1) = seed;
    for pair in data.chunks_exact(8) {
        let word = |bytes: &[u8]| {
            let array = [bytes[0], bytes[1], bytes[2], bytes[3]];
            if big_endian {
                u32::from_be_bytes(array)
            } else {
                u32::from_le_bytes(array)
            }
        };
        s0 = s0.wrapping_add(word(&pair[..4])).wrapping_add(s1);
        s1 = s1.wrapping_add(word(&pair[4..])).wrapping_add(s0);
    }
    (s0, s1)
}

fn decode_page_size(raw: u16) -> Result<usize, SqliteError> {
    let size = if raw == 1 { 65_536 } else { usize::from(raw) };
    if (512..=65_536).contains(&size) && size.is_power_of_two() {
        Ok(size)
    } else {
        Err(SqliteError::NotADatabase(format!("page size {raw}")))
    }
}

fn decode_page_size_wal(raw: u32) -> Result<usize, SqliteError> {
    let size = raw as usize;
    if (512..=65_536).contains(&size) && size.is_power_of_two() {
        Ok(size)
    } else {
        Err(corrupt(format!("write-ahead log page size {raw}")))
    }
}

/// The offset of cell `index`, read from the cell-pointer array at `array`.
fn cell_offset(page: &[u8], array: usize, index: usize, number: u32) -> Result<usize, SqliteError> {
    let offset = usize::from(be_u16(page, array + index * 2)?);
    if offset == 0 || offset >= page.len() {
        return Err(corrupt(format!(
            "cell {index} on page {number} points outside the page"
        )));
    }
    Ok(offset)
}

/// Decode a record: a header of serial types, then the values they describe.
fn decode_record(payload: &[u8], encoding: TextEncoding) -> Result<Vec<Value>, SqliteError> {
    let (header_len, mut cursor) = varint(payload, 0)?;
    let header_len = usize::try_from(header_len)
        .ok()
        .filter(|len| *len <= payload.len() && *len >= cursor)
        .ok_or_else(|| corrupt("a record header overruns its record"))?;
    let mut body = header_len;
    let mut values = Vec::new();
    while cursor < header_len {
        let (serial, used) = varint(payload, cursor)?;
        cursor += used;
        let (value, width) = decode_value(serial, &payload[body.min(payload.len())..], encoding)?;
        body += width;
        values.push(value);
    }
    if cursor != header_len || body > payload.len() {
        return Err(corrupt("a record's values overrun its payload"));
    }
    Ok(values)
}

fn decode_value(
    serial: u64,
    body: &[u8],
    encoding: TextEncoding,
) -> Result<(Value, usize), SqliteError> {
    let int = |width: usize| -> Result<(Value, usize), SqliteError> {
        let bytes = body
            .get(..width)
            .ok_or_else(|| corrupt("an integer overruns its record"))?;
        let mut value = if bytes[0] & 0x80 != 0 { -1i64 } else { 0 };
        for byte in bytes {
            value = (value << 8) | i64::from(*byte);
        }
        Ok((Value::Integer(value), width))
    };
    match serial {
        0 => Ok((Value::Null, 0)),
        1 => int(1),
        2 => int(2),
        3 => int(3),
        4 => int(4),
        5 => int(6),
        6 => int(8),
        7 => {
            let bytes = body
                .get(..8)
                .ok_or_else(|| corrupt("a real overruns its record"))?;
            let mut array = [0u8; 8];
            array.copy_from_slice(bytes);
            Ok((Value::Real(f64::from_be_bytes(array)), 8))
        }
        8 => Ok((Value::Integer(0), 0)),
        9 => Ok((Value::Integer(1), 0)),
        10 | 11 => Err(corrupt(format!("reserved serial type {serial}"))),
        _ => {
            let len = usize::try_from((serial - 12) / 2)
                .map_err(|_| corrupt("a value's length overflows"))?;
            let bytes = body
                .get(..len)
                .ok_or_else(|| corrupt("a value overruns its record"))?;
            if serial % 2 == 0 {
                Ok((Value::Blob(bytes.to_vec()), len))
            } else {
                Ok((Value::Text(decode_text(bytes, encoding)), len))
            }
        }
    }
}

/// Decode stored text. Invalid sequences become U+FFFD rather than failing
/// the row: a title a browser stored badly is still worth importing.
fn decode_text(bytes: &[u8], encoding: TextEncoding) -> String {
    match encoding {
        TextEncoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|pair| {
                    if encoding == TextEncoding::Utf16Le {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect();
            String::from_utf16_lossy(&units)
        }
    }
}

/// An SQLite varint: one to nine bytes, big-endian, seven bits per byte
/// except the ninth, which contributes all eight.
fn varint(bytes: &[u8], at: usize) -> Result<(u64, usize), SqliteError> {
    let mut value = 0u64;
    for index in 0..9 {
        let byte = *bytes
            .get(at + index)
            .ok_or_else(|| corrupt("a varint runs off the end of its page"))?;
        if index == 8 {
            return Ok(((value << 8) | u64::from(byte), 9));
        }
        value = (value << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    unreachable!("the ninth byte always returns")
}

fn be_u16(bytes: &[u8], at: usize) -> Result<u16, SqliteError> {
    bytes
        .get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or_else(|| corrupt(format!("a 16-bit field at {at} lies outside its page")))
}

fn be_u32(bytes: &[u8], at: usize) -> Result<u32, SqliteError> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| corrupt(format!("a 32-bit field at {at} lies outside its page")))
}

/// The column names of a `CREATE TABLE` statement, in declaration order, and
/// the index of the `INTEGER PRIMARY KEY` column if there is one.
fn parse_create_table(sql: &str) -> Result<(Vec<String>, Option<usize>), SqliteError> {
    let open = sql
        .find('(')
        .ok_or_else(|| SqliteError::Unsupported("a table defined without a column list".into()))?;
    let close = sql
        .rfind(')')
        .filter(|close| *close > open)
        .ok_or_else(|| corrupt("a table definition's column list is unterminated"))?;
    let tail = sql[close + 1..].to_ascii_uppercase().replace(',', " ");
    let options: Vec<&str> = tail.split_whitespace().collect();
    if options.windows(2).any(|pair| pair == ["WITHOUT", "ROWID"]) {
        return Err(SqliteError::Unsupported("a WITHOUT ROWID table".into()));
    }

    let mut columns = Vec::new();
    let mut declared_types = Vec::new();
    let mut alias = None;
    let mut table_key: Option<String> = None;
    for definition in split_top_level(&sql[open + 1..close]) {
        let definition = definition.trim();
        let upper = definition.to_ascii_uppercase();
        let first = upper.split_whitespace().next().unwrap_or("");
        if matches!(first, "CONSTRAINT" | "UNIQUE" | "CHECK" | "FOREIGN") {
            continue;
        }
        if first == "PRIMARY" || upper.starts_with("PRIMARY(") {
            // A table-level key over one column makes that column the rowid
            // alias when the column is declared INTEGER.
            if let (Some(open), Some(close)) = (definition.find('('), definition.rfind(')')) {
                let inner: Vec<&str> = definition[open + 1..close].split(',').collect();
                if inner.len() == 1 {
                    let name = inner[0].split_whitespace().next().unwrap_or("");
                    table_key = Some(unquote(name).0);
                }
            }
            continue;
        }
        let (name, rest) = unquote(definition);
        if name.is_empty() {
            return Err(corrupt("a column with no name"));
        }
        let rest_upper = rest.to_ascii_uppercase();
        let declared = rest_upper
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        let is_key = rest_upper.contains("PRIMARY KEY") && !rest_upper.contains("PRIMARY KEY DESC");
        if declared == "INTEGER" && is_key {
            alias = Some(columns.len());
        }
        declared_types.push(declared);
        columns.push(name);
    }
    if alias.is_none() {
        if let Some(key) = table_key {
            alias = columns
                .iter()
                .position(|column| column.eq_ignore_ascii_case(&key))
                .filter(|index| declared_types[*index] == "INTEGER");
        }
    }
    Ok((columns, alias))
}

/// Split a column list on commas that are not inside parentheses or quotes.
fn split_top_level(list: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (index, ch) in list.char_indices() {
        match quote {
            Some(open) => {
                let close = if open == '[' { ']' } else { open };
                if ch == close {
                    quote = None;
                }
            }
            None => match ch {
                '"' | '\'' | '`' | '[' => quote = Some(ch),
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(&list[start..index]);
                    start = index + 1;
                }
                _ => {}
            },
        }
    }
    parts.push(&list[start..]);
    parts
}

/// Read one identifier, quoted or bare, from the start of `text`; returns it
/// and whatever follows.
fn unquote(text: &str) -> (String, &str) {
    let text = text.trim_start();
    let mut chars = text.char_indices();
    match chars.next() {
        Some((_, open @ ('"' | '`' | '['))) => {
            let close = if open == '[' { ']' } else { open };
            let mut name = String::new();
            let mut rest = "";
            let mut iter = text[1..].char_indices().peekable();
            while let Some((index, ch)) = iter.next() {
                if ch == close {
                    // A doubled closing quote is an escaped quote.
                    if close != ']' && iter.peek().map(|(_, next)| *next) == Some(close) {
                        name.push(close);
                        iter.next();
                        continue;
                    }
                    rest = &text[1 + index + ch.len_utf8()..];
                    break;
                }
                name.push(ch);
            }
            (name, rest)
        }
        Some(_) => {
            let end = text
                .find(|ch: char| ch.is_whitespace() || ch == '(' || ch == ',')
                .unwrap_or(text.len());
            (text[..end].to_string(), &text[end..])
        }
        None => (String::new(), ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_decode_across_their_widths() {
        assert_eq!(varint(&[0x05], 0).unwrap(), (5, 1));
        assert_eq!(varint(&[0x81, 0x00], 0).unwrap(), (128, 2));
        let nine = [0xff; 9];
        assert_eq!(varint(&nine, 0).unwrap(), (u64::MAX, 9));
        assert!(
            varint(&[0x80], 0).is_err(),
            "a truncated varint is an error"
        );
    }

    #[test]
    fn records_decode_every_serial_type() {
        // header: len 7, types NULL, i8, i16, int-0, int-1, text(3)
        let payload = [7, 0, 1, 2, 8, 9, 19, 0xfe, 0x01, 0x00, b'a', b'b', b'c'];
        let values = decode_record(&payload, TextEncoding::Utf8).unwrap();
        assert_eq!(
            values,
            vec![
                Value::Null,
                Value::Integer(-2),
                Value::Integer(256),
                Value::Integer(0),
                Value::Integer(1),
                Value::Text("abc".into()),
            ]
        );
    }

    #[test]
    fn a_record_that_overruns_its_payload_is_an_error() {
        let payload = [2, 19, b'a'];
        assert!(decode_record(&payload, TextEncoding::Utf8).is_err());
    }

    #[test]
    fn create_table_parsing_finds_columns_and_the_rowid_alias() {
        let (columns, alias) = parse_create_table(
            "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url LONGVARCHAR,\
             title LONGVARCHAR,visit_count INTEGER DEFAULT 0 NOT NULL,\
             last_visit_time INTEGER NOT NULL,hidden INTEGER DEFAULT 0 NOT NULL)",
        )
        .unwrap();
        assert_eq!(
            columns,
            [
                "id",
                "url",
                "title",
                "visit_count",
                "last_visit_time",
                "hidden"
            ]
        );
        assert_eq!(alias, Some(0));

        let (columns, alias) = parse_create_table(
            "CREATE TABLE \"a b\" ([x y] TEXT, `z` INTEGER, n INTEGER, \
             CHECK (n > 0), PRIMARY KEY (n))",
        )
        .unwrap();
        assert_eq!(columns, ["x y", "z", "n"]);
        assert_eq!(alias, Some(2));

        assert!(matches!(
            parse_create_table("CREATE TABLE t(a TEXT PRIMARY KEY, b) WITHOUT ROWID"),
            Err(SqliteError::Unsupported(_))
        ));
    }

    #[test]
    fn non_databases_are_refused_without_panicking() {
        assert!(Database::open(b"", None).is_err());
        assert!(Database::open(b"not a database at all", None).is_err());
        let mut header = vec![0u8; 512];
        header[..16].copy_from_slice(HEADER_MAGIC);
        header[16..18].copy_from_slice(&3u16.to_be_bytes());
        assert!(matches!(
            Database::open(&header, None),
            Err(SqliteError::NotADatabase(_))
        ));
    }

    #[test]
    fn the_wal_checksum_matches_sqlites_reference_arithmetic() {
        // Two words per step: s0 += w0 + s1; s1 += w1 + s0.
        let data = [0, 0, 0, 1, 0, 0, 0, 2];
        assert_eq!(wal_checksum(&data, true, (0, 0)), (1, 3));
        assert_eq!(
            wal_checksum(&data, false, (0, 0)),
            (1 << 24, (2 << 24) + (1 << 24))
        );
    }
}
