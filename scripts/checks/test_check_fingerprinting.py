#!/usr/bin/env python3
"""Tests for the fingerprinting check. The check is CI's authority to fail a
build over a read of a device, display, font, network or timing
characteristic, so its own behaviour is checked rather than assumed -- above
all that it FAILS: a clean tree passes, and a read of every category the check
covers fails.

The sources quoted in this file are Python string literals; the check reads
Rust source, never Python, so quoting them here is not a breach and needs no
assembly trick.

Run: python3 scripts/checks/test_check_fingerprinting.py
"""
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_fingerprinting as check  # noqa: E402

PASSED = FAILED = 0


def report(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

CLEAN_RUST = (
    "#![forbid(unsafe_code)]\n"
    "use std::time::{Duration, Instant};\n"
    "\n"
    "/// Loads time out after a while; the tab model reads an interval, never\n"
    "/// the counter under it. MachineGuid in a doc comment is not a read.\n"
    "pub fn timed_out(start: Instant) -> bool {\n"
    "    // Nor is /etc/machine-id named in a line comment.\n"
    "    /* Nor GetAdaptersAddresses in a block comment. */\n"
    "    start.elapsed() > Duration::from_secs(30)\n"
    "}\n"
    "\n"
    "pub fn draw() -> u32 {\n"
    "    let mysysinfo_cache = 7;\n"
    "    let scale = window_scale_factor();\n"
    "    mysysinfo_cache + scale\n"
    "}\n"
)

CLEAN_MANIFEST = (
    '[package]\nname = "x"\nversion = "0.0.0"\n\n'
    '[dependencies]\nserde = "1"\ngetrandom = "0.3"\n'
)


def tree(files):
    """Build the files in a temporary tree and run the check over it.

    `files` maps a relative POSIX path to its content; bytes are written raw,
    so a test can plant a file the check cannot decode. Returns what
    `check_tree` returns.
    """
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        root = base / "tree"
        root.mkdir()
        for relative, content in files.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            if isinstance(content, bytes):
                path.write_bytes(content)
            else:
                path.write_text(content, encoding="utf-8")
        return check.check_tree(root)


def passing_tree(extra=None):
    files = {
        "crates/x/src/lib.rs": CLEAN_RUST,
        "crates/x/Cargo.toml": CLEAN_MANIFEST,
    }
    if extra:
        files.update(extra)
    return files


def with_rust(body):
    """A passing tree plus one Rust file holding `body`."""
    return passing_tree({"crates/x/src/probe.rs": body})


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


def run_main(*arguments):
    return subprocess.run(
        [sys.executable, str(HERE / "check_fingerprinting.py"), *arguments],
        capture_output=True,
        text=True,
    )


# --- the repository itself ----------------------------------------------------

problems, read = check.check_tree(check.REPO)
report("the repository passes", problems == [])
report("...having read the update client",
       "crates/evreos-platform/src/update.rs" in read)
report("...the rollout draw", "crates/evreos-platform/src/update/rollout.rs" in read)
report("...the shell", any(path.startswith("crates/evreos-shell/src/") for path in read))

# --- the table itself ---------------------------------------------------------

names = [name for _, name, _ in check.COMPILED]
report("every source name is unique", len(names) == len(set(names)))
report("no source name holds whitespace, which an entry could not spell",
       all(name.split() == [name] for name in names))

# --- a clean tree -------------------------------------------------------------

problems, read = tree(passing_tree())[:2]
report("a clean tree passes: Instant, a scale factor, a longer identifier and "
       "comments naming sources are not reads", problems == [])
report("...reading its Rust source", read == ["crates/x/src/lib.rs"])

# --- SOURCE: one caught case per category ------------------------------------

CAUGHT = (
    ("machine and volume identifier", "MachineGuid",
     'let id = registry.get_string("MachineGuid");\n'),
    ("machine and volume identifier", "machine-id",
     'let id = std::fs::read_to_string("/etc/machine-id");\n'),
    ("machine and volume identifier", "IOPlatformUUID",
     'let key = "IOPlatformUUID";\n'),
    ("machine and volume identifier", "GetVolumeInformation",
     "unsafe { GetVolumeInformationW(root, None, Some(&mut serial), None, None, None) };\n"),
    ("MAC address or network characteristic", "GetAdaptersAddresses",
     "let adapters = GetAdaptersAddresses(AF_UNSPEC, 0, None, buffer, &mut size);\n"),
    ("MAC address or network characteristic", "/sys/class/net",
     'let mac = read("/sys/class/net/eth0/address");\n'),
    ("screen geometry", "primary_monitor",
     "let size = event_loop.primary_monitor().map(|m| m.size());\n"),
    ("screen geometry", "GetSystemMetrics",
     "let width = GetSystemMetrics(SM_CXSCREEN);\n"),
)

for category, name, body in CAUGHT:
    problems = tree(with_rust(body))[0]
    report(f"a read of {name} fails as a {category} source",
           mentions(problems, "crates/x/src/probe.rs:1", category, repr(name)))

problems = tree(with_rust('let id = read("/ETC/MACHINE-ID");\nlet g = "machineguid";\n'))[0]
report("a source spelled in another case is the same source",
       mentions(problems, "probe.rs:1", "'machine-id'")
       and mentions(problems, "probe.rs:2", "'MachineGuid'"))

problems = tree(with_rust('let key = r"SOFTWARE\\Microsoft\\Cryptography";\n'))[0]
report("the registry key that holds MachineGuid fails through a raw string",
       mentions(problems, "probe.rs:1", "Cryptography"))

problems = tree(with_rust("let a = 1;\nlet b = 2;\nlet guid = MachineGuid();\n"))[0]
report("a failure names the line the read is on", mentions(problems, "probe.rs:3"))

problems = tree(passing_tree({"crates/x/notes.md": "MachineGuid, /etc/machine-id\n",
                              "crates/x/tool.py": "GetVolumeInformationW()\n"}))[0]
report("markdown and Python are not read", problems == [])

# --- unreadable input and an unreached verdict -------------------------------

problems = tree(passing_tree({"crates/x/src/bad.rs": b"\xff\xfe MachineGuid"}))[0]
report("Rust that is not UTF-8 fails rather than passing unread",
       mentions(problems, "bad.rs", "not valid UTF-8"))

problems = tree(passing_tree({
    "crates/x/src/bom.rs": "﻿let g = \"MachineGuid\";\n",
}))[0]
report("a byte-order mark is not a way past the check",
       mentions(problems, "bom.rs:1", "'MachineGuid'"))

problems = tree(passing_tree({
    "target/debug/build/dep.rs": 'let g = "MachineGuid";\n',
    "TARGET/vendor/dep.rs": 'let g = "MachineGuid";\n',
    ".cache/dep.rs": 'let g = "MachineGuid";\n',
}))[0]
report("target/ in any case and dot-directories are not read", problems == [])

try:
    tree({"README.md": "nothing this check reads\n"})
    report("a tree with nothing to read raises rather than passing", False)
except check.CheckError:
    report("a tree with nothing to read raises rather than passing", True)

try:
    check.check_tree(Path(tempfile.gettempdir()) / "no-such-tree-for-this-check")
    report("a root that is not a directory raises", False)
except check.CheckError:
    report("a root that is not a directory raises", True)

# --- the command line ---------------------------------------------------------

result = run_main()
report("the default invocation passes on the repository", result.returncode == 0)
report("...ending in one line saying so",
       result.stdout.strip().startswith("Fingerprinting check passed:")
       and len(result.stdout.strip().splitlines()) == 1)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "src").mkdir()
    (root / "src" / "lib.rs").write_text('let g = "MachineGuid";\n', encoding="utf-8")
    result = run_main("--root", str(root))
    report("a breach exits 1", result.returncode == 1)
    report("...naming the file and line on standard error",
           "src/lib.rs:1" in result.stderr and "MachineGuid" in result.stderr)

    empty = root / "empty"
    empty.mkdir()
    result = run_main("--root", str(empty))
    report("an unreached verdict exits 2", result.returncode == 2)

print(f"{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
