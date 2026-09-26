# Import profile-store read: the harness

The scripts behind `docs/measurements/import-profile-read.md`, committed so
each tier's run on its pinned runner can take them unchanged. Nothing in the
build or CI runs them. They use only Python's standard library and Node 22's
built-in `fetch` and `WebSocket`. The instrument that measures the shipped
reader is not here: it is `crates/evreos-shell/examples/import_probe.rs`,
which builds with the crate so it cannot drift from the code it measures.

| Script | What it does |
| --- | --- |
| `pages.py PORT INTERVAL_MS` | Serves pages that navigate onward after `INTERVAL_MS`, so a browser left on one keeps recording visits. |
| `cdp_bookmarks.mjs PORT PAGE SECONDS` | Opens the browser's own bookmarks page (`chrome://bookmarks`, `edge://favorites`) over the DevTools protocol and creates a folder and five bookmarks every 100 ms, so the browser keeps rewriting `Bookmarks`. The browser must be started with `--remote-debugging-port=PORT`. |
| `probe_lib.py STORE QUERY` | Reads a live store through the SQLite library: its header, its journal state, a direct read, and a read of a copy with and without its sidecars. |
| `stress_copy.py STORE QUERY SECONDS main\|sidecars` | Copies a live store in one pass, repeatedly, and checks each copy with `PRAGMA quick_check`. |
| `synth_writer.py rollback\|wal DIR SECONDS` | The worst-case writer: the browsers' locking and journal modes, every row rewritten and stamped with one generation per transaction. |
| `synth_naive.py rollback\|wal DIR SECONDS` | Single-pass copies of the synthetic store, torn if `quick_check` fails or the copy holds two generations. |

A run, per browser:

```
python3 pages.py 8765 150 &
google-chrome --headless=new --no-first-run --remote-debugging-port=9333 \
    --user-data-dir=/tmp/p http://127.0.0.1:8765/p/1 &
node cdp_bookmarks.mjs 9333 chrome://bookmarks 75 &
cargo run --release -p evreos-shell --example import_probe -- chrome /tmp/p/Default 60

python3 probe_lib.py /tmp/p/Default/History "select count(*) from urls"
python3 stress_copy.py /tmp/p/Default/History "select count(*) from urls" 150 main
```

and against the synthetic writers, each beside the shipped reader and,
with a writer of its own, beside single-pass copies through the SQLite
library:

```
python3 synth_writer.py rollback /tmp/s 75 &
cargo run --release -p evreos-shell --example import_probe -- chrome /tmp/s 60 synthetic
python3 synth_writer.py rollback /tmp/s2 70 &
python3 synth_naive.py rollback /tmp/s2 60

python3 synth_writer.py wal /tmp/w 75 &
cargo run --release -p evreos-shell --example import_probe -- firefox /tmp/w 60 synthetic
python3 synth_writer.py wal /tmp/w2 70 &
python3 synth_naive.py wal /tmp/w2 60
```
