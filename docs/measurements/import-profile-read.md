# Import profile-store reads

- **Task**: T058 (`specs/001-evreos-v1/tasks.md`) / CAR-114
- **Date**: 2026-09-26
- **Status**: Measured on the Linux instrument named below. The tier-1 and
  tier-2 runs are owed, on each tier's pinned runner once T013 procures it.
- **Identifier**: none. N10 is the catalogue-format measurement
  (`specs/001-evreos-v1/measurements/n10-catalogue-format.md`) and is not
  reused here, although research.md's N10 entry asks this question beside it.

## The two questions

FR-012 requires importing bookmarks and history from Chrome, Firefox and Edge.
research.md asks two things of that import before it is built, and T058 commits
the answers here:

1. **Does each profile store read reliably while its browser is running**,
   and how: read directly, or copied and then read, with a write-ahead log
   present?
2. **What does the dependency the reading needs cost** in bytes, stated
   against `budgets.toml` under FR-043?

## The answers

1. **No store can be read directly through the SQLite library while its
   browser runs, and a single copy is not reliable either; a verified copy is.**
   All three browsers hold their SQLite store under an exclusive lock, so the
   library's own read fails outright with "database is locked" on every one.
   Copying the files and reading the copy works — but a copy taken in one pass
   can catch a transaction half-written. Against the live browsers that was
   rare (no torn copy in 16,972 naive copies), but the window is real: a
   rollback journal was seen hot during five copies. Against a writer that
   rewrites its whole store in every transaction, 412 of 14,465 naive copies
   were torn and 9 more unreadable. The import therefore copies each store
   into memory **twice over** — read, re-read and compare, with the rollback
   journal checked cold before and after — and accepts a copy only when
   nothing moved; the protocol is at
   `crates/evreos-shell/src/import/snapshot.rs`. It accepted no torn copy in
   any run: 0 of 7,038 trials across the three live browsers, 0 of 1,543
   against the worst-case writers. Firefox's write-ahead log **must be copied
   with its main file**: a read of `places.sqlite` alone missed committed
   rows in 826 of 1,101 trials.
2. **The reader is in-tree, safe Rust, and costs 0 bytes in the binary this
   change ships and 93,960 bytes (0.090 MB) once an interface reaches it**,
   against at least 1,112,160 bytes (1.061 MB) for the SQLite library with a
   JSON parser — the option not adopted. Both
   figures are Linux x86-64 deltas; no SC-001 entry can be measured yet, for
   the reason the gate states below.

## Instrument, and what this record claims

| | |
| --- | --- |
| Machine | Cloud container: Intel Xeon @ 2.80 GHz, 4 vCPU, 15 GiB memory, ext4 on a virtual disk |
| Operating system | Ubuntu 24.04.4 LTS, Linux 6.18.44 |
| Browsers | Google Chrome 154.0.8037.57, Microsoft Edge 154.0.4258.37, Mozilla Firefox 156.0.1 — each the vendor's Linux build, run headless with a fresh profile |
| SQLite library (comparison arms) | 3.45.1, through Python's `sqlite3` module |
| Evreos reader | this change, `cargo build --release`, rustc 1.94.1 |

This instrument is **neither tier**. decisions/0004 authorises an interim
instrument for T015 and T016 alone, and this record does not extend it: it
claims nothing about Windows or macOS. What it establishes is what the
browsers themselves decide, which the platform does not — each store's
format, its journal mode and its locking mode, all set by the browser's own
code and observed here in the files it wrote — and how the reader behaves
against those files under concurrent writes.

What it cannot establish, and the tier runs must: whether the operating
system lets another process open a file the browser holds open. On Linux a
lock is advisory and never stops a read. On Windows a lock is mandatory, but
SQLite takes it only on a byte range at the 1 GiB offset, and SQLite's own
file opens share read and write access, so a reader's open is expected to
succeed there too — expected, not measured. The expectation has a known way
to fail: Chromium has shipped a lock on Windows that opens its cookie store
with sharing denied, to keep other processes out of it, and a browser that
did the same with `History` would refuse even the copy. The import would then
fail with the store named as unreadable rather than import anything, and the
tier-1 run is what finds out. The probe below is the instrument for that run:
it is committed so each pinned runner can take it unchanged.

## Method

A local server answered every request with a page that navigated onward
after 150 ms, so a browser left on it recorded a new visit several times a
second for as long as it ran. Each browser was started headless on a fresh
profile pointed at it. For the bookmark store, each Chromium browser's own
bookmarks page (`chrome://bookmarks`, `edge://favorites`) was driven over the
DevTools protocol to create a folder and five bookmarks every 100 ms, so the
browser kept rewriting `Bookmarks`.

Three arms read each store while the browser wrote it:

- **The SQLite library, directly**: opening the live store read-only
  (`mode=ro`) and counting rows.
- **The SQLite library, on a copy**: copying the files and opening the copy,
  with and without the write-ahead log; and 150 s of repeated single-pass
  copies, each checked with the library's `PRAGMA quick_check`.
- **The reader this change ships**:
  `crates/evreos-shell/examples/import_probe.rs`, which runs the import's own
  read — `read_profile_with`, the verified copy included — in a loop for 60 s
  and reports attempts, the cause of each retry, and any accepted copy that
  read torn: fewer rows or bookmarks than an earlier trial, which a browser
  that only adds them cannot produce. A copy the parser refuses fails its
  trial and is counted as a failure, not as a torn copy. Beside it the probe runs
  two unverified arms through the same parser: one single read of the files
  with the log applied, and one of the main file alone.

The live browsers write little per transaction, so a torn copy is rare
against them. To show what the verification buys when a torn copy is not
rare, two **synthetic worst-case writers** used the browsers' own modes —
exclusive locking with a rollback journal as Chromium runs `History`, and
with a write-ahead log as Firefox runs `places.sqlite` — on a 3,000-row
table, rewriting every row in every transaction and stamping one generation
number into all of them. A consistent copy holds exactly one generation; a
torn one holds two.

Every figure below is a count of trials. No address or title from any
profile is recorded here or anywhere else; the probe prints counts only.

## Results

### What each browser keeps, and how

| Store | Format | Journal | Locking | Page size |
| --- | --- | --- | --- | --- |
| Chrome `History` | SQLite | rollback journal: `History-journal` kept after each transaction with its header zeroed | exclusive | 4,096 |
| Edge `History` | SQLite | as Chrome | exclusive | 4,096 |
| Chrome, Edge `Bookmarks` | JSON | none: rewritten whole, through a temporary file renamed into place, by Chromium's design | none | — |
| Firefox `places.sqlite` | SQLite | write-ahead log: `places.sqlite-wal`, no `-shm` file | exclusive | 32,768 |

The locking mode is read from the files: in exclusive mode SQLite zeroes a
rollback journal's header rather than deleting the journal, and never creates
the shared-memory file a write-ahead log otherwise has — both as observed. The
SQLite header's write and read versions were 1 and 1 for both `History`
stores and 2 and 2 for `places.sqlite`. Firefox also grows `places.sqlite` in
preallocated chunks: the file was 5 MiB while its header counted 52 pages,
which the reader honours as the format specifies.

### Direct read, and a copy read once, through the SQLite library

| Store | Direct read | Copy of main file | Copy with sidecars |
| --- | --- | --- | --- |
| Chrome `History` | fails: database is locked | 171 rows | 171 rows |
| Edge `History` | fails: database is locked | 171 rows | 171 rows |
| Firefox `places.sqlite` | fails: database is locked | 203 visited places | 206 visited places |

| 150 s of single-pass copies | Copies | Torn (`quick_check`) | Unreadable | Journal seen hot mid-copy |
| --- | --- | --- | --- | --- |
| Chrome `History`, main file | 11,131 | 0 | 0 | 5 |
| Firefox `places.sqlite`, with log | 5,841 | 0 | 0 | — |

### The reader this change ships, against the live browsers

| 60 s each | Trials | Failed | Accepted copy torn | Attempts (attempts: trials) | Retry causes | Slowest |
| --- | --- | --- | --- | --- | --- | --- |
| Chrome, history writes | 1,622 | 0 | 0 | 1: 1,621, 2: 1 | journal hot 1 | 96 ms |
| Edge, history writes | 1,691 | 0 | 0 | 1: 1,689, 2: 2 | journal hot 2 | 80 ms |
| Firefox, history writes | 1,101 | 0 | 0 | 1: 1,078, 2: 22, 3: 1 | moved 24 | 109 ms |
| Chrome, history and bookmark writes | 993 | 0 | 0 | 1: 990, 2: 2, 3: 1 | moved 1, journal hot 3 | 550 ms |
| Edge, history and bookmark writes | 1,631 | 0 | 0 | 1: 1,629, 2: 2 | moved 1, journal hot 1 | 371 ms |

| Unverified arms, same runs | Parse errors | Read torn | Main file alone short of rows |
| --- | --- | --- | --- |
| Chrome, Edge (all four runs) | 0 | 0 | 0 of 5,937 |
| Firefox | 0 | 0 | **826 of 1,101** |
| `Bookmarks` read once, Chrome and Edge | 0 of 2,624 unparseable | — | — |

A single unverified read of `Bookmarks` never failed, which is what replacing
the file whole should give: a reader sees the old document or the new one.
The import verifies it anyway, since the same protocol costs one comparison.

### The synthetic worst case

| | Rollback journal (Chromium's mode) | Write-ahead log (Firefox's mode) |
| --- | --- | --- |
| Writer transactions, each rewriting all 3,000 rows | 2,372 in 75 s beside the reader, 1,987 in 70 s beside the copies | 2,198 in 75 s, 1,782 in 70 s |
| SQLite library, single-pass copies | **412 of 14,465 torn**, 9 unreadable | **4 of 2,410 torn** |
| This reader, one unverified read | **28 of 588 torn**, 1 parse error | **1 of 955 torn** |
| This reader, verified (shipped) | **0 of 588 torn**; attempts 1: 344, 2: 117, 3: 76, 4: 26, 5: 12, 6: 7, 7: 5, 8: 1; slowest 1,657 ms | **0 of 955 torn**; attempts 1: 832, 2: 101, 3: 19, 4: 2, 5: 1; slowest 407 ms |

One trial against the rollback writer needed all eight attempts the default
policy allows. A browser writing that hard would, some of the time, exhaust
the bound, and the import then fails with `SourceBusy` — naming the store and
asking the member to close that browser — rather than importing a torn copy.

### Cross-check

With every browser stopped, the reader's history count equals the SQLite
library's for the same filter — visible, visited, `http`, `https` or `file` —
on every profile the runs produced: 6,613, 5,992, 539, 484 and 8,829 rows. Its
bookmark counts equal what the DevTools driver created (3,245 and 495) and
Firefox's four default bookmarks.

## What was adopted

- **Verified copy-then-read, in memory** (`snapshot.rs`). Each store's files
  are read whole, read again and compared chunk by chunk, with the rollback
  journal checked for its magic number before and after; up to eight attempts
  with a doubling pause capped at 400 ms, about 1.6 s in all. The copy never
  touches disk, so a crash mid-import leaves no copy of another browser's
  history behind. The cost is memory: the store's size, for as long as the
  read takes. A `History` or `places.sqlite` runs to tens of megabytes on a
  profile used for years. SC-004's condition — ten tabs, sampled through a
  soak — does not include an import, so no entry measures this; it is
  recorded so the harness that measures SC-004 does not meet it unannounced.
- **The write-ahead log is always read with its main file**, and applied only
  up to its last valid commit — frames with the wrong salt, a broken checksum
  chain, or no commit after them are ignored, as SQLite's own recovery ignores
  them. The fixture profile holds a log whose last transaction never committed,
  and `tests/import.rs` asserts none of it is imported.
- **An in-tree reader rather than the SQLite library.** The library could not
  have read the live stores anyway — the lock refuses it — so an importer built
  on it copies first too, and once the bytes are copied the file format is all
  that is needed. `crates/evreos-shell/src/import/sqlite.rs` reads table
  b-trees, overflow chains, records and the log, and nothing else; the JSON
  parser beside it reads `Bookmarks`. Both are safe Rust with every offset
  bounds-checked, depth bounded and page cycles refused, so a malformed store
  is an error value and never a panic.
- **One history row per address, dated at its most recent visit**, from
  Chromium's `urls` and Firefox's `moz_places` — the shape both browsers'
  own importers carry across, rather than every visit.
- **The read runs on the worker pool; the write runs where the stores live.**
  Reading took 8–22 ms for the live profiles here and 64 ms for a synthetic
  profile of 20,000 bookmarks in 400 folders. Writing, which happens on the
  thread that owns the stores, took 10–13 ms for the live profiles and
  **264 ms for the 20,000 bookmarks**: the bookmark store computes each new
  row's position by scanning every bookmark it holds, so a bulk insert grows
  with the square of its size. That is a single pause on the member's own
  action, not a latency on any path SC-006 measures, and it is recorded here
  so that it is not discovered later.

## Byte cost under FR-043

Five release builds of `evreos-shell` under the workspace's release profile
(`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`,
`strip = "symbols"`), each measured as the binary's `st_size`:

| Build | What it contains | Bytes |
| --- | --- | --- |
| A | the merge base, `main` at `aaffd75` | 507,576 |
| B | this change, as it ships | 507,576 |
| C | B, with `main` calling `discover` and `ImportJob::run` | 673,928 |
| E | B, with `main` making the same store and catalogue calls the import makes, and no import | 579,968 |
| D | B, with `main` reading a table through `rusqlite` 0.40.2 (`bundled`) and a document through `serde_json` 1.0.151 | 1,619,736 |

- **B − A = 0 bytes.** Nothing in the binary reaches the import yet: no
  interface surface calls it, so the linker keeps none of it. This change
  moves no SC-001 figure today.
- **C − E = 93,960 bytes (0.090 MB)**: the importer's own cost — both
  readers, the verified copy, discovery and the job — once a surface reaches
  it. C − B, 166,352 bytes, is larger because it also counts the
  stores and the catalogue, which the binary does not reach yet either and
  which every surface that shows history or bookmarks will pull in whether or
  not import exists.
- **D − B = 1,112,160 bytes (1.061 MB)**: a floor for the option not
  adopted. It is the libraries alone behind one query and one parse; the
  importer that would sit on them — both browsers' readers, the copy, the
  writes — is not in it.

The gate measures SC-001 only from "the installer artefact CI publishes", and
neither platform's installer exists, so on this host it reports every entry
unmeasured, as it must:

```
$ python3 scripts/check-budgets.py --allow-unpinned-runners --allow-unmeasured
  ...
    - SC-001 download size (windows)  (a windows figure is measured on a windows host; this host builds no tier's artefact; deferred by --allow-unmeasured)
    - SC-001 installed footprint (windows)  (a windows figure is measured on a windows host; this host builds no tier's artefact; deferred by --allow-unmeasured)
    - SC-001 download size (macos)  (a macos figure is measured on a macos host; this host builds no tier's artefact; deferred by --allow-unmeasured)
    - SC-001 installed footprint (macos)  (a macos figure is measured on a macos host; this host builds no tier's artefact; deferred by --allow-unmeasured)
  ...
  measured: nothing on this linux host; this host builds no tier's installer artefact
```

**All four SC-001 entries therefore stay unmeasured-with-reason.** Against
their figures — 20 MB download and 60 MB installed on each tier — the
importer's reachable cost is 0.45% of the download figure on this
host, and the library option's floor is 5.30%, 11.8 times as much. These
are Linux x86-64 deltas, not the tier-1 or tier-2 figure: N10 measured its deltas on
the tier-1 host, and the same builds on each tier's host are owed with the
tier runs above. No baseline is written, because no SC-001 entry was
measured.

## Reproducing

The harness is committed at `tests/spikes/import-profile-read/`, whose README
gives each script and a whole run; the probe is the example above, which
builds with the crate it measures. Per browser:

```
python3 tests/spikes/import-profile-read/pages.py 8765 150 &
google-chrome --headless=new --no-first-run --remote-debugging-port=9333 \
    --user-data-dir=/tmp/p http://127.0.0.1:8765/p/1 &
node tests/spikes/import-profile-read/cdp_bookmarks.mjs 9333 chrome://bookmarks 75 &
cargo run --release -p evreos-shell --example import_probe -- chrome /tmp/p/Default 60
```

The byte-cost builds are the five described above. C, D and E differ from B
only in a block at the top of `main` that makes the named calls reachable,
and D in two lines of `crates/evreos-shell/Cargo.toml`; none of the three is
committed, since each exists only to be measured.

## Owed

- **The tier runs**: the probe against Chrome, Edge and Firefox on each
  tier's pinned runner, which settles whether each operating system lets the
  reader open a store its browser holds open, and the five builds on each
  tier's host for the byte deltas there.
- **A signed-in Chrome profile**: newer Chrome keeps the account's bookmarks
  in `AccountBookmarks`, which the reader imports and the fixture profile
  exercises, but no run here signed in, so no live `AccountBookmarks` was
  read.
