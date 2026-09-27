//! Chrome's and Edge's profile stores, which share Chromium's formats.
//!
//! - `Bookmarks`: one JSON document, rewritten whole through a temporary
//!   file and a rename whenever a bookmark changes. Newer Chrome keeps the
//!   bookmarks saved to the signed-in account in `AccountBookmarks` beside
//!   it, in the same format, and shows the two merged; both are read.
//! - `History`: an SQLite database in rollback-journal mode, held under an
//!   exclusive lock while the browser runs. Only its `urls` table is read:
//!   one row per address, carrying the most recent visit.
//!
//! Nothing else in the profile is opened. `Login Data`, `Login Data For
//! Account`, `Web Data` and `Cookies` hold credentials and are never read.

#![forbid(unsafe_code)]

use std::fs;
use std::path::Path;

use super::json::{self, Json};
use super::snapshot::{FileSource, SnapshotPolicy, StoreFiles, read_bounded};
use super::sqlite::{Database, Value};
use super::{
    ImportError, ImportScope, ImportedNode, ImportedRoot, ImportedVisit, MAX_PROFILE_LIST_BYTES,
    RootKind, SourceBrowser, SourceProfile, chromium_time, clean_address, copy_store, is_dir,
};

/// Every file an import of a Chromium profile reads.
pub(super) const STORE_FILES: &[&str] = &[
    "Bookmarks",
    "AccountBookmarks",
    "History",
    "History-wal",
    "History-journal",
];

/// Chromium's JSON keys for its permanent folders.
const ROOTS: [(&str, RootKind); 3] = [
    ("bookmark_bar", RootKind::Toolbar),
    ("other", RootKind::Other),
    ("synced", RootKind::Mobile),
];

/// Bounded as the JSON parser bounds nesting, so a tree of any depth the
/// parser accepts is walked without exhausting the worker's stack.
const MAX_FOLDER_DEPTH: usize = 256;

pub(super) fn read(
    profile: &Path,
    scope: ImportScope,
    policy: SnapshotPolicy,
    source: &mut impl FileSource,
) -> Result<(Vec<ImportedRoot>, Vec<ImportedVisit>), ImportError> {
    let mut roots: Vec<ImportedRoot> = Vec::new();
    if scope.bookmarks {
        for store in ["Bookmarks", "AccountBookmarks"] {
            let files = StoreFiles::single(profile.join(store));
            let Some(copy) = copy_store(source, files, store, policy)? else {
                continue;
            };
            let text = String::from_utf8(copy.main)
                .map_err(|_| ImportError::unreadable(store, "the file is not UTF-8"))?;
            let document =
                json::parse(&text).map_err(|error| ImportError::unreadable(store, error))?;
            for root in
                bookmarks(&document).map_err(|reason| ImportError::unreadable(store, reason))?
            {
                match roots.iter_mut().find(|held| held.kind == root.kind) {
                    Some(held) => held.children.extend(root.children),
                    None => roots.push(root),
                }
            }
        }
    }

    let mut history = Vec::new();
    if scope.history {
        let store = "History";
        let files = StoreFiles::sqlite(profile.join(store));
        if let Some(copy) = copy_store(source, files, store, policy)? {
            let unreadable = |error| ImportError::unreadable(store, error);
            let db = Database::from_bytes(&copy.main, copy.wal.as_deref()).map_err(unreadable)?;
            history = visits(&db).map_err(unreadable)?;
        }
    }
    Ok((roots, history))
}

/// The permanent folders of a `Bookmarks` document and what they hold.
fn bookmarks(document: &Json) -> Result<Vec<ImportedRoot>, &'static str> {
    let roots = document
        .get("roots")
        .ok_or("the document has no \"roots\" member")?;
    let mut out = Vec::new();
    for (key, kind) in ROOTS {
        let Some(root) = roots.get(key) else {
            continue;
        };
        out.push(ImportedRoot {
            kind,
            children: nodes(root, 0)?,
        });
    }
    Ok(out)
}

fn nodes(folder: &Json, depth: usize) -> Result<Vec<ImportedNode>, &'static str> {
    if depth > MAX_FOLDER_DEPTH {
        return Err("folders nest too deeply");
    }
    let Some(children) = folder.get("children").and_then(Json::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for child in children {
        let name = child.get("name").and_then(Json::as_str).unwrap_or("");
        match child.get("type").and_then(Json::as_str) {
            Some("folder") => out.push(ImportedNode::Folder {
                title: name.to_string(),
                children: nodes(child, depth + 1)?,
            }),
            Some("url") => {
                let Some(address) = child
                    .get("url")
                    .and_then(Json::as_str)
                    .and_then(clean_address)
                else {
                    continue;
                };
                let added_at = child
                    .get("date_added")
                    .and_then(|value| match value {
                        Json::String(text) | Json::Number(text) => text.parse().ok(),
                        _ => None,
                    })
                    .and_then(chromium_time);
                out.push(ImportedNode::Bookmark {
                    title: if name.is_empty() {
                        address.clone()
                    } else {
                        name.to_string()
                    },
                    address,
                    added_at,
                });
            }
            // A node type this reader does not know is skipped rather than
            // failing the import of every node it does know.
            _ => {}
        }
    }
    Ok(out)
}

/// One row per address from `urls`, skipping addresses Chromium marks hidden
/// (subframes and redirect sources it never shows in its own history) and
/// rows with no visit time.
fn visits(db: &Database) -> Result<Vec<ImportedVisit>, super::sqlite::SqliteError> {
    let table = db.table("urls")?;
    let url = table.column("url")?;
    let title = table.column("title")?;
    let last_visit = table.column("last_visit_time")?;
    let hidden = table.column("hidden")?;
    let mut out = Vec::new();
    db.scan(&table, |row| {
        if row.get(hidden).as_integer().unwrap_or(0) != 0 {
            return Ok(());
        }
        let Some(address) = row.get(url).as_text().and_then(clean_address) else {
            return Ok(());
        };
        let Some(visited_at) = row.get(last_visit).as_integer().and_then(chromium_time) else {
            return Ok(());
        };
        let title = match row.get(title) {
            Value::Text(text) => text.clone(),
            _ => String::new(),
        };
        out.push(ImportedVisit {
            address,
            title,
            visited_at,
        });
        Ok(())
    })?;
    Ok(out)
}

/// The profiles in a Chromium `User Data` directory: `Default` and each
/// `Profile N` holding a store, named as the browser's `Local State` names
/// them where it does.
pub(super) fn discover(browser: SourceBrowser, user_data: &Path) -> Vec<SourceProfile> {
    let Ok(entries) = fs::read_dir(user_data) else {
        return Vec::new();
    };
    let local_state = read_bounded(&user_data.join("Local State"), MAX_PROFILE_LIST_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .and_then(|text| json::parse(&text).ok());
    let mut profiles: Vec<SourceProfile> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let dir = entry.file_name().into_string().ok()?;
            let path = entry.path();
            let is_profile = dir == "Default" || dir.starts_with("Profile ");
            let has_store = ["Bookmarks", "History", "Preferences"]
                .iter()
                .any(|store| path.join(store).is_file());
            if !is_profile || !is_dir(&path) || !has_store {
                return None;
            }
            let name = local_state
                .as_ref()
                .and_then(|state| {
                    state
                        .get("profile")?
                        .get("info_cache")?
                        .get(&dir)?
                        .get("name")?
                        .as_str()
                })
                .filter(|name| !name.is_empty())
                .map_or_else(|| dir.clone(), str::to_string);
            Some(SourceProfile::new(browser, name, path))
        })
        .collect();
    profiles.sort_by(|a, b| a.path.cmp(&b.path));
    profiles
}
