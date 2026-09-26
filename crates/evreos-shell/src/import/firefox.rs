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
use std::path::Path;

use super::snapshot::{FileSource, SnapshotPolicy, StoreFiles};
use super::sqlite::{Database, SqliteError, Value};
use super::{
    ImportError, ImportScope, ImportedNode, ImportedRoot, ImportedVisit, RootKind, clean_address,
    copy_store, firefox_time,
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
