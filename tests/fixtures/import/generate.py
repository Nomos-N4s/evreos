"""Generate the FR-012 import fixture profiles in this directory.

Run from the repository root:

    python3 tests/fixtures/import/generate.py

The output is committed; this script is its provenance. It uses only the
standard library's sqlite3 module. Each store's schema is the one the named
browser version wrote on the measurement instrument, copied from the live
profiles `docs/measurements/import-profile-read.md` describes: Chrome and Edge
154 (`urls`, `meta`), Firefox 156 (`moz_places`, `moz_bookmarks`). Only the
tables the importer reads, and `meta`, are reproduced.

Every value is invented. What each profile exercises is listed in README.md;
the tests in crates/evreos-shell/tests/import.rs assert it. A write-ahead log's
salts are random, so regenerating changes the Firefox log's bytes but not
what it holds.

The decoy credential stores beside each profile carry SENTINEL, which no file
Evreos writes during an import may ever contain.
"""

import json
import os
import shutil
import sqlite3

HERE = os.path.dirname(os.path.abspath(__file__))
SENTINEL = "fixture-credential-sentinel-4b1d"

CHROME_URLS = (
    "CREATE TABLE urls(id INTEGER PRIMARY KEY AUTOINCREMENT,url LONGVARCHAR,"
    "title LONGVARCHAR,visit_count INTEGER DEFAULT 0 NOT NULL,"
    "typed_count INTEGER DEFAULT 0 NOT NULL,last_visit_time INTEGER NOT NULL,"
    "hidden INTEGER DEFAULT 0 NOT NULL)"
)
CHROME_META = "CREATE TABLE meta(key LONGVARCHAR NOT NULL UNIQUE PRIMARY KEY, value LONGVARCHAR)"
MOZ_PLACES = (
    "CREATE TABLE moz_places (   id INTEGER PRIMARY KEY, url LONGVARCHAR, title LONGVARCHAR, "
    "rev_host LONGVARCHAR, visit_count INTEGER DEFAULT 0, hidden INTEGER DEFAULT 0 NOT NULL, "
    "typed INTEGER DEFAULT 0 NOT NULL, frecency INTEGER DEFAULT -1 NOT NULL, "
    "last_visit_date INTEGER , guid TEXT, foreign_count INTEGER DEFAULT 0 NOT NULL, "
    "url_hash INTEGER DEFAULT 0 NOT NULL , description TEXT, preview_image_url TEXT, "
    "site_name TEXT, origin_id INTEGER REFERENCES moz_origins(id), "
    "recalc_frecency INTEGER NOT NULL DEFAULT 0, alt_frecency INTEGER, "
    "recalc_alt_frecency INTEGER NOT NULL DEFAULT 0)"
)
MOZ_BOOKMARKS = (
    "CREATE TABLE moz_bookmarks (  id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER DEFAULT NULL, "
    "parent INTEGER, position INTEGER, title LONGVARCHAR, keyword_id INTEGER, folder_type TEXT, "
    "dateAdded INTEGER, lastModified INTEGER, guid TEXT, syncStatus INTEGER NOT NULL DEFAULT 0, "
    "syncChangeCounter INTEGER NOT NULL DEFAULT 1)"
)

# 2023-11-14T22:13:20Z, in each browser's epoch.
UNIX_BASE_US = 1_700_000_000_000_000
CHROME_BASE_US = UNIX_BASE_US + 11_644_473_600_000_000


def fresh(path):
    if os.path.isdir(path):
        shutil.rmtree(path)
    os.makedirs(path)


def write_json(path, value):
    with open(path, "w", encoding="utf-8") as f:
        json.dump(value, f, ensure_ascii=False, indent=3)
        f.write("\n")


def decoy_sqlite(path, table, value):
    db = sqlite3.connect(path)
    db.execute(f"CREATE TABLE {table}(origin_url TEXT, username_value TEXT, password_value BLOB)")
    db.execute(f"INSERT INTO {table} VALUES (?, ?, ?)",
               ("https://bank.example/", "alice", value.encode()))
    db.commit()
    db.close()


def chromium_history(path, rows, page_size=4096):
    db = sqlite3.connect(path)
    db.execute(f"PRAGMA page_size={page_size}")
    db.execute(CHROME_META)
    db.execute(CHROME_URLS)
    db.executemany("INSERT INTO meta VALUES (?, ?)", [("version", "70"), ("last_compatible_version", "16")])
    db.executemany(
        "INSERT INTO urls(url, title, visit_count, last_visit_time, hidden) VALUES (?, ?, ?, ?, ?)",
        rows,
    )
    db.commit()
    db.close()
    # Chromium runs its stores in exclusive locking mode, which keeps the
    # rollback journal after each transaction with its header zeroed rather
    # than deleting it: this is what a profile looks like at rest.
    with open(path + "-journal", "wb") as f:
        f.write(bytes(512))


def chromium_bookmark(name, url, n):
    return {"date_added": str(CHROME_BASE_US + n * 1_000_000), "guid": f"00000000-0000-4000-8000-{n:012d}",
            "id": str(100 + n), "name": name, "type": "url", "url": url}


def chromium_folder(name, children, n):
    return {"children": children, "date_added": str(CHROME_BASE_US + n * 1_000_000),
            "guid": f"00000000-0000-4000-9000-{n:012d}", "id": str(500 + n), "name": name,
            "type": "folder"}


def chromium_bookmarks(bar, other, synced, bar_name, other_name, synced_name):
    return {
        "checksum": "00000000000000000000000000000000",
        "roots": {
            "bookmark_bar": chromium_folder(bar_name, bar, 1),
            "other": chromium_folder(other_name, other, 2),
            "synced": chromium_folder(synced_name, synced, 3),
        },
        "version": 1,
    }


def chrome():
    root = os.path.join(HERE, "chrome")
    fresh(root)
    default = os.path.join(root, "Default")
    os.makedirs(default)

    rows = []
    for n in range(1500):
        # Enough rows that the table is a two-level b-tree.
        rows.append((f"https://site{n % 97}.example/page/{n}", f"Page {n}", 1 + n % 5,
                     CHROME_BASE_US + n * 60_000_000, 0))
    rows += [
        ("https://el.example/", "Καλημέρα κόσμε", 3, CHROME_BASE_US + 1_600 * 60_000_000, 0),
        # Longer than a page, so it is stored across an overflow chain.
        ("https://long.example/?q=" + "x" * 6000, "Long address", 1, CHROME_BASE_US + 1_601 * 60_000_000, 0),
        # A credential in the address: imported without it.
        (f"https://alice:{SENTINEL}@bank.example/account", "Bank", 2, CHROME_BASE_US + 1_602 * 60_000_000, 0),
        # Not imported: hidden, browser-internal, inline data, never visited.
        ("https://hidden.example/frame", "Hidden", 1, CHROME_BASE_US + 1_603 * 60_000_000, 1),
        ("chrome://settings/", "Settings", 1, CHROME_BASE_US + 1_604 * 60_000_000, 0),
        ("data:text/html,hi", "Inline", 1, CHROME_BASE_US + 1_605 * 60_000_000, 0),
        ("https://never.example/", "Never visited", 0, 0, 0),
    ]
    chromium_history(os.path.join(default, "History"), rows)

    write_json(os.path.join(default, "Bookmarks"), chromium_bookmarks(
        bar=[
            chromium_bookmark("Σελίδα έναρξης", "https://start.example/", 1),
            chromium_folder("Work", [
                chromium_bookmark("Tracker", "https://tracker.example/board", 2),
                chromium_folder("Deep", [
                    chromium_bookmark("Deep link", "https://deep.example/a/b", 3),
                ], 4),
                chromium_folder("Empty", [], 5),
            ], 6),
            chromium_bookmark("Bookmarklet", "javascript:alert(1)", 7),
            chromium_bookmark("Bank login", f"https://bob:{SENTINEL}@bank.example/login", 8),
            chromium_bookmark("", "https://untitled.example/", 9),
        ],
        other=[chromium_bookmark("Recipes", "https://recipes.example/", 10)],
        synced=[],
        bar_name="Bookmarks bar", other_name="Other bookmarks", synced_name="Mobile bookmarks",
    ))
    # Bookmarks saved to the signed-in account, which newer Chrome keeps in a
    # file of their own and shows merged with the local ones.
    account = chromium_bookmarks(
        bar=[chromium_bookmark("Account bookmark", "https://account.example/", 11)],
        other=[], synced=[chromium_bookmark("From phone", "https://phone.example/", 12)],
        bar_name="Bookmarks bar", other_name="Other bookmarks", synced_name="Mobile bookmarks",
    )
    write_json(os.path.join(default, "AccountBookmarks"), account)
    write_json(os.path.join(default, "Preferences"), {"profile": {"name": "Person 1"}})

    # Decoy credential stores, which an import never opens.
    decoy_sqlite(os.path.join(default, "Login Data"), "logins", SENTINEL)
    decoy_sqlite(os.path.join(default, "Login Data For Account"), "logins", SENTINEL)
    decoy_sqlite(os.path.join(default, "Cookies"), "cookies", SENTINEL)
    decoy_sqlite(os.path.join(default, "Web Data"), "autofill", SENTINEL)

    # A second profile holding no store but its preferences, and two
    # directories Chrome keeps beside profiles that are not profiles.
    for extra in ("Profile 1", "System Profile", "Guest Profile"):
        os.makedirs(os.path.join(root, extra))
        write_json(os.path.join(root, extra, "Preferences"), {})
    write_json(os.path.join(root, "Local State"), {
        "profile": {"info_cache": {"Default": {"name": "Person 1"}, "Profile 1": {"name": "Work"}}},
    })


def edge():
    root = os.path.join(HERE, "edge")
    fresh(root)
    default = os.path.join(root, "Default")
    os.makedirs(default)
    chromium_history(os.path.join(default, "History"), [
        ("https://news.example/", "News", 4, CHROME_BASE_US + 10 * 60_000_000, 0),
        ("https://mail.example/inbox", "Mail", 9, CHROME_BASE_US + 20 * 60_000_000, 0),
        ("edge://settings/", "Settings", 1, CHROME_BASE_US + 30 * 60_000_000, 0),
    ])
    write_json(os.path.join(default, "Bookmarks"), chromium_bookmarks(
        bar=[chromium_bookmark("News", "https://news.example/", 1)],
        other=[chromium_folder("Travel", [chromium_bookmark("Maps", "https://maps.example/", 2)], 3)],
        synced=[],
        bar_name="Favorites bar", other_name="Other favorites", synced_name="Mobile favorites",
    ))
    write_json(os.path.join(root, "Local State"), {"profile": {"info_cache": {"Default": {"name": "Profile 1"}}}})
    decoy_sqlite(os.path.join(default, "Login Data"), "logins", SENTINEL)


def firefox():
    root = os.path.join(HERE, "firefox")
    fresh(root)
    profile = os.path.join(root, "Profiles", "fx1a2b3c.default-release")
    os.makedirs(profile)
    with open(os.path.join(root, "profiles.ini"), "w", encoding="utf-8") as f:
        f.write(
            "[Install4F96D1932A9F858E]\nDefault=Profiles/fx1a2b3c.default-release\nLocked=1\n\n"
            "[Profile1]\nName=gone\nIsRelative=1\nPath=Profiles/no-such-profile\n\n"
            "[Profile0]\nName=default-release\nIsRelative=1\nPath=Profiles/fx1a2b3c.default-release\n"
            "Default=1\n\n[General]\nStartWithLastProfile=1\nVersion=2\n"
        )

    work = os.path.join(root, "work")
    os.makedirs(work)
    path = os.path.join(work, "places.sqlite")
    db = sqlite3.connect(path, isolation_level=None)
    db.execute("PRAGMA page_size=32768")
    # Firefox's mode: WAL, under an exclusive lock, so no -shm file exists.
    db.execute("PRAGMA locking_mode=EXCLUSIVE")
    db.execute("PRAGMA journal_mode=WAL")
    db.execute("PRAGMA wal_autocheckpoint=0")
    db.execute(MOZ_PLACES)
    db.execute(MOZ_BOOKMARKS)

    def place(pid, url, title, last_visit, hidden=0, visits=1):
        db.execute("INSERT INTO moz_places(id, url, title, visit_count, hidden, last_visit_date, guid) "
                   "VALUES (?, ?, ?, ?, ?, ?, ?)",
                   (pid, url, title, visits, hidden, last_visit, f"place{pid:07d}"))

    def item(iid, kind, parent, position, title, fk=None, guid=None):
        db.execute("INSERT INTO moz_bookmarks(id, type, fk, parent, position, title, dateAdded, "
                   "lastModified, guid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                   (iid, kind, fk, parent, position, title, UNIX_BASE_US + iid * 1_000_000,
                    UNIX_BASE_US + iid * 1_000_000, guid or f"item{iid:08d}"))

    db.execute("BEGIN")
    for n in range(800):
        place(n + 1, f"https://fx{n % 41}.example/p/{n}", f"Firefox page {n}", UNIX_BASE_US + n * 60_000_000)
    place(801, "https://el.example/firefox", "Γειά σου", UNIX_BASE_US + 801 * 60_000_000)
    place(802, "https://hidden.example/", "Hidden", UNIX_BASE_US + 802 * 60_000_000, hidden=1)
    place(803, "https://bookmarked-only.example/", "Bookmarked, never visited", None, visits=0)
    place(804, "about:preferences", "Preferences", UNIX_BASE_US + 804 * 60_000_000)
    place(805, "place:sort=8&maxResults=10", "Most visited", None, visits=0)
    place(806, f"https://carol:{SENTINEL}@bank.example/", "Bank", UNIX_BASE_US + 806 * 60_000_000)
    place(807, "https://untitled.example/", "Place title", UNIX_BASE_US + 807 * 60_000_000)
    place(808, "javascript:void(0)", "Script", None, visits=0)
    place(809, "https://tagged.example/", "Tagged", UNIX_BASE_US + 809 * 60_000_000)

    folder, bookmark, separator = 2, 1, 3
    item(1, folder, 0, 0, "", guid="root________")
    item(2, folder, 1, 0, "menu", guid="menu________")
    item(3, folder, 1, 1, "toolbar", guid="toolbar_____")
    item(4, folder, 1, 2, "tags", guid="tags________")
    item(5, folder, 1, 3, "unfiled", guid="unfiled_____")
    item(6, folder, 1, 4, "mobile", guid="mobile______")
    # Menu: Firefox's own default folder.
    item(10, folder, 2, 0, "Mozilla Firefox")
    item(11, bookmark, 10, 0, "Get Help", fk=1)
    item(12, bookmark, 10, 1, "Get Involved", fk=2)
    # Toolbar, positions deliberately inserted out of order.
    item(22, bookmark, 3, 2, "Script", fk=808)
    item(20, bookmark, 3, 0, "Γειά", fk=801)
    item(21, separator, 3, 1, None)
    item(23, folder, 3, 3, "Reading")
    item(24, folder, 23, 0, "Later")
    item(25, bookmark, 24, 0, "Bookmarked only", fk=803)
    item(26, bookmark, 3, 4, None, fk=807)
    item(27, bookmark, 3, 5, "Most visited", fk=805)
    item(28, bookmark, 3, 6, "Bank", fk=806)
    # Unfiled.
    item(30, bookmark, 5, 0, "Unfiled one", fk=3)
    # Tags: a tag folder holding an entry for an already-bookmarked place.
    item(40, folder, 4, 0, "a-tag")
    item(41, bookmark, 40, 0, None, fk=809)
    db.execute("COMMIT")
    db.execute("PRAGMA wal_checkpoint(TRUNCATE)")

    # Written after the checkpoint, so these exist only in the log.
    db.execute("BEGIN")
    place(900, "https://only-in-log.example/", "Only in the log", UNIX_BASE_US + 900 * 60_000_000)
    item(50, bookmark, 5, 1, "Only in the log", fk=900)
    db.execute("COMMIT")
    db.execute("UPDATE moz_places SET title = 'Renamed in the log' WHERE id = 2")

    # An uncommitted transaction large enough to spill frames into the log.
    db.execute("PRAGMA cache_size=1")
    db.execute("BEGIN")
    for n in range(300):
        place(10_000 + n, f"https://uncommitted.example/{n}", "Uncommitted " + "u" * 200,
              UNIX_BASE_US + (10_000 + n) * 60_000_000)
    # Copy while the connection is open: closing would checkpoint and delete
    # the log, which is exactly the state a running Firefox never leaves.
    shutil.copyfile(path, os.path.join(profile, "places.sqlite"))
    shutil.copyfile(path + "-wal", os.path.join(profile, "places.sqlite-wal"))
    db.execute("ROLLBACK")
    db.close()
    shutil.rmtree(work)

    with open(os.path.join(profile, "logins.json"), "w", encoding="utf-8") as f:
        json.dump({"logins": [{"hostname": "https://bank.example", "encryptedUsername": SENTINEL,
                               "encryptedPassword": SENTINEL}]}, f)
    with open(os.path.join(profile, "key4.db"), "wb") as f:
        f.write(SENTINEL.encode())
    decoy_sqlite(os.path.join(profile, "logins.db"), "logins", SENTINEL)
    decoy_sqlite(os.path.join(profile, "cookies.sqlite"), "moz_cookies", SENTINEL)


if __name__ == "__main__":
    chrome()
    edge()
    firefox()
