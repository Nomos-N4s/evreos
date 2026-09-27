//! Firefox's profile store.
//!
//! Bookmarks and history both live in `places.sqlite`, an SQLite database in
//! write-ahead-log mode, held under an exclusive lock while Firefox runs —
//! which is why it keeps no `-shm` file and why its `-wal` file has to be
//! copied with it: the measurement found rows present only in the log. Two
//! tables are read, `moz_places` and `moz_bookmarks`.
//!
//! Nothing else in the profile is opened. `logins.json`, `logins.db`,
//! `key4.db`, `cookies.sqlite` and `formhistory.sqlite` hold credentials or
//! form data and are never read.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::snapshot::{FileSource, SnapshotPolicy, StoreFiles, read_bounded};
use super::sqlite::{Database, SqliteError, Value};
use super::{
    ImportError, ImportScope, ImportedNode, ImportedRoot, ImportedVisit, MAX_PROFILE_LIST_BYTES,
    RootKind, SourceBrowser, SourceProfile, clean_address, copy_store, firefox_time, is_dir,
};

/// Every file an import of a Firefox profile reads.
pub(super) const STORE_FILES: &[&str] = &[
    "places.sqlite",
    "places.sqlite-wal",
    "places.sqlite-journal",
];

/// The GUIDs Firefox gives its permanent folders. The tags root is not here:
/// tags are a second index over bookmarks already imported from these.
const ROOTS: [(&str, RootKind); 4] = [
    ("toolbar_____", RootKind::Toolbar),
    ("menu________", RootKind::Menu),
    ("unfiled_____", RootKind::Other),
    ("mobile______", RootKind::Mobile),
];

const TYPE_BOOKMARK: i64 = 1;
const TYPE_FOLDER: i64 = 2;
const MAX_FOLDER_DEPTH: usize = 256;

/// A `moz_places` row, as much of it as an import needs.
struct Place {
    url: String,
    title: Option<String>,
    hidden: bool,
    last_visit: Option<i64>,
}

/// A `moz_bookmarks` row.
struct Item {
    id: i64,
    kind: i64,
    place: Option<i64>,
    parent: i64,
    position: i64,
    title: Option<String>,
    added: Option<i64>,
    guid: Option<String>,
}

pub(super) fn read(
    profile: &Path,
    scope: ImportScope,
    policy: SnapshotPolicy,
    source: &mut impl FileSource,
) -> Result<(Vec<ImportedRoot>, Vec<ImportedVisit>), ImportError> {
    let store = "places.sqlite";
    let files = StoreFiles::sqlite(profile.join(store));
    let Some(copy) = copy_store(source, files, store, policy)? else {
        return Ok((Vec::new(), Vec::new()));
    };
    let unreadable = |error: SqliteError| ImportError::unreadable(store, error);
    let db = Database::open(&copy.main, copy.wal.as_deref()).map_err(unreadable)?;
    let places = places(&db).map_err(unreadable)?;

    let history = if scope.history {
        let mut visits: Vec<ImportedVisit> = places
            .values()
            .filter(|place| !place.hidden)
            .filter_map(|place| {
                Some(ImportedVisit {
                    address: clean_address(&place.url)?,
                    title: place.title.clone().unwrap_or_default(),
                    visited_at: place.last_visit.and_then(firefox_time)?,
                })
            })
            .collect();
        visits.sort_by(|a, b| {
            a.visited_at
                .cmp(&b.visited_at)
                .then(a.address.cmp(&b.address))
        });
        visits
    } else {
        Vec::new()
    };

    let roots = if scope.bookmarks {
        bookmarks(&db, &places).map_err(unreadable)?
    } else {
        Vec::new()
    };
    Ok((roots, history))
}

fn places(db: &Database) -> Result<HashMap<i64, Place>, SqliteError> {
    let table = db.table("moz_places")?;
    let id = table.column("id")?;
    let url = table.column("url")?;
    let title = table.column("title")?;
    let hidden = table.column("hidden")?;
    let last_visit = table.column("last_visit_date")?;
    let mut out = HashMap::new();
    db.scan(&table, |row| {
        let (Some(key), Some(address)) = (row.get(id).as_integer(), row.get(url).as_text()) else {
            return Ok(());
        };
        out.insert(
            key,
            Place {
                url: address.to_string(),
                title: text(row.get(title)),
                hidden: row.get(hidden).as_integer().unwrap_or(0) != 0,
                last_visit: row.get(last_visit).as_integer(),
            },
        );
        Ok(())
    })?;
    Ok(out)
}

fn text(value: &Value) -> Option<String> {
    value
        .as_text()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn bookmarks(
    db: &Database,
    places: &HashMap<i64, Place>,
) -> Result<Vec<ImportedRoot>, SqliteError> {
    let table = db.table("moz_bookmarks")?;
    let columns = [
        "id",
        "type",
        "fk",
        "parent",
        "position",
        "title",
        "dateAdded",
        "guid",
    ]
    .map(|name| table.column(name));
    let [id, kind, fk, parent, position, title, added, guid] = columns;
    let (id, kind, fk, parent, position, title, added, guid) =
        (id?, kind?, fk?, parent?, position?, title?, added?, guid?);

    let mut items: HashMap<i64, Item> = HashMap::new();
    let mut children: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    db.scan(&table, |row| {
        let Some(item_id) = row.get(id).as_integer() else {
            return Ok(());
        };
        let item = Item {
            id: item_id,
            kind: row.get(kind).as_integer().unwrap_or(0),
            place: row.get(fk).as_integer(),
            parent: row.get(parent).as_integer().unwrap_or(0),
            position: row.get(position).as_integer().unwrap_or(0),
            title: text(row.get(title)),
            added: row.get(added).as_integer(),
            guid: text(row.get(guid)),
        };
        children
            .entry(item.parent)
            .or_default()
            .push((item.position, item.id));
        items.insert(item_id, item);
        Ok(())
    })?;
    for list in children.values_mut() {
        list.sort_unstable();
    }

    let mut out = Vec::new();
    for (root_guid, root_kind) in ROOTS {
        let Some(root) = items
            .values()
            .find(|item| item.guid.as_deref() == Some(root_guid))
        else {
            continue;
        };
        let mut visited = HashSet::new();
        visited.insert(root.id);
        out.push(ImportedRoot {
            kind: root_kind,
            children: walk(root.id, &items, &children, places, &mut visited, 0)?,
        });
    }
    Ok(out)
}

fn walk(
    folder: i64,
    items: &HashMap<i64, Item>,
    children: &HashMap<i64, Vec<(i64, i64)>>,
    places: &HashMap<i64, Place>,
    visited: &mut HashSet<i64>,
    depth: usize,
) -> Result<Vec<ImportedNode>, SqliteError> {
    if depth > MAX_FOLDER_DEPTH {
        return Err(SqliteError::Corrupt(
            "bookmark folders nest too deeply".into(),
        ));
    }
    let mut out = Vec::new();
    for (_, child_id) in children.get(&folder).map(Vec::as_slice).unwrap_or(&[]) {
        if !visited.insert(*child_id) {
            return Err(SqliteError::Corrupt(
                "the bookmark tree contains a cycle".into(),
            ));
        }
        let Some(item) = items.get(child_id) else {
            continue;
        };
        match item.kind {
            TYPE_FOLDER => out.push(ImportedNode::Folder {
                title: item.title.clone().unwrap_or_default(),
                children: walk(item.id, items, children, places, visited, depth + 1)?,
            }),
            TYPE_BOOKMARK => {
                let Some(place) = item.place.and_then(|key| places.get(&key)) else {
                    continue;
                };
                let Some(address) = clean_address(&place.url) else {
                    continue;
                };
                let title = item
                    .title
                    .clone()
                    .or_else(|| place.title.clone())
                    .unwrap_or_else(|| address.clone());
                out.push(ImportedNode::Bookmark {
                    title,
                    address,
                    added_at: item.added.and_then(firefox_time),
                });
            }
            // Separators, and any type a later Firefox adds, are skipped.
            _ => {}
        }
    }
    Ok(out)
}

/// Whether a `Path=` from `profiles.ini` has a shape discovery may follow.
/// Discovery runs before the member has chosen anything, and looking at a
/// path on another machine opens a connection to it, so only the plain local
/// shapes are followed, and anything else is skipped, whatever it names:
/// - relative to `app_dir`, with no leading separator and no `:`, so that
///   joining it cannot replace `app_dir` with a drive, a share or a device;
/// - absolute on Unix, `/` followed by anything but a second separator;
/// - absolute on Windows, a drive letter, `:` and one separator.
///
/// Refused so are `\\host\share`, `//host/share`, and the device forms
/// `\\?\`, `\\.\` and `\??\`; and, whatever its shape, a path with a
/// component Windows reads as a device, such as `LPT1` or `nul.txt`, which it
/// opens as `\\.\LPT1` and which can itself be redirected to a share. A
/// drive letter the system maps to a share cannot be told from a local drive
/// by its name, and is followed like one.
fn a_local_path(raw: &str, relative: bool) -> bool {
    if raw.split(['/', '\\']).any(names_a_device) {
        return false;
    }
    let separator = |ch: Option<char>| matches!(ch, Some('/' | '\\'));
    let mut chars = raw.chars();
    let (first, second, third) = (chars.next(), chars.next(), chars.next());
    if relative {
        return !raw.is_empty() && !separator(first) && !raw.contains(':');
    }
    let unix = first == Some('/') && !separator(second);
    let drive = first.is_some_and(|ch| ch.is_ascii_alphabetic())
        && second == Some(':')
        && separator(third)
        && !separator(raw.chars().nth(3));
    unix || drive
}

/// Whether a path component is one of the names Windows reserves for a
/// device, in any case, with or without an extension or trailing spaces and
/// dots: `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, and `COM` or `LPT`
/// followed by a digit or a superscript one, two or three.
fn names_a_device(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    match stem.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" => true,
        _ => {
            let port = stem
                .strip_prefix("COM")
                .or_else(|| stem.strip_prefix("LPT"));
            port.is_some_and(|rest| {
                let mut chars = rest.chars();
                matches!(
                    (chars.next(), chars.next()),
                    (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
                )
            })
        }
    }
}

/// The profiles `profiles.ini` lists under `app_dir`, by the names it gives.
pub(super) fn discover(app_dir: &Path) -> Vec<SourceProfile> {
    let Some(ini) = read_bounded(&app_dir.join("profiles.ini"), MAX_PROFILE_LIST_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
    else {
        return Vec::new();
    };
    let mut profiles = Vec::new();
    let mut section = String::new();
    let mut name: Option<String> = None;
    let mut path: Option<String> = None;
    let mut relative = true;
    let mut flush =
        |section: &str, name: &mut Option<String>, path: &mut Option<String>, relative: bool| {
            if section.starts_with("Profile") {
                if let Some(raw) = path.take().filter(|raw| a_local_path(raw, relative)) {
                    let dir: PathBuf = if relative {
                        app_dir.join(&raw)
                    } else {
                        PathBuf::from(&raw)
                    };
                    if is_dir(&dir) {
                        let label = name.take().unwrap_or(raw);
                        profiles.push(SourceProfile::new(SourceBrowser::Firefox, label, dir));
                    }
                }
            }
            *name = None;
            *path = None;
        };
    for line in ini.lines() {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            flush(&section, &mut name, &mut path, relative);
            section = header.to_string();
            relative = true;
        } else if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "Name" => name = Some(value.trim().to_string()),
                "Path" => path = Some(value.trim().to_string()),
                "IsRelative" => relative = value.trim() != "0",
                _ => {}
            }
        }
    }
    flush(&section, &mut name, &mut path, relative);
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_local_profile_paths_are_followed() {
        for (raw, relative) in [
            ("Profiles/abc.default", true),
            ("abc.default", true),
            ("/home/member/.mozilla/firefox/abc", false),
            ("C:\\Users\\member\\abc", false),
            ("d:/profiles/abc", false),
            ("Profiles/COM10", true),
            ("Profiles/console", true),
            ("Profiles/lpt", true),
        ] {
            assert!(a_local_path(raw, relative), "{raw:?} is local");
        }
        for (raw, relative) in [
            ("\\\\host\\share\\p", false),
            ("//host/share/p", false),
            ("\\\\?\\UNC\\host\\share\\p", false),
            ("//?/UNC/host/share/p", false),
            ("\\\\.\\pipe\\p", false),
            ("\\??\\UNC\\host\\share\\p", false),
            ("\\??\\GLOBALROOT\\Device\\Mup\\host\\share", false),
            ("C:\\\\host\\share", false),
            ("Profiles/abc", false),
            ("", false),
            ("\\\\host\\share\\p", true),
            ("/abs/path", true),
            ("C:\\Users\\abc", true),
            ("\\??\\UNC\\host\\share", true),
            ("LPT1", true),
            ("Profiles/COM1", true),
            ("Profiles/nul.txt", true),
            ("Profiles/con .default", true),
            ("C:\\x\\LPT1", false),
            ("/home/member/aux", false),
            ("Profiles/COM\u{b9}", true),
            ("", true),
        ] {
            assert!(!a_local_path(raw, relative), "{raw:?}, relative {relative}");
        }
    }
}
