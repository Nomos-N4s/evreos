#!/usr/bin/env python3
"""Tests for the fingerprinting check. The check is CI's authority to fail a
build over a read of a device, display, font, network or timing
characteristic, so its own behaviour is checked rather than assumed -- above
all that it FAILS: a clean tree passes, and a read of every category the check
covers fails.
The same read hashed under a rotating salt fails too, because FR-036a binds on
the derivation and not on what it produces.

The sources quoted in this file are Python string literals; the check reads
Rust source, script, markup and Cargo manifests, never Python, so quoting them
here is not a breach and needs no assembly trick.

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

# Passed as `allowlist` to leave the allowlist file out of the tree entirely.
MISSING = object()


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


def tree(files, allowlist=""):
    """Build the files in a temporary tree and run the check over it.

    `files` maps a relative POSIX path to its content; bytes are written raw,
    so a test can plant a file the check cannot decode. `allowlist` is the
    allowlist's text, written beside the tree rather than in it, or MISSING
    to leave it absent. Returns what `check_tree` returns.
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
        allowlist_path = base / "fingerprinting-allowlist.txt"
        if allowlist is not MISSING:
            allowlist_path.write_text(allowlist, encoding="utf-8")
        return check.check_tree(root, allowlist_path)


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

problems, read, allowlisted = check.check_tree(check.REPO)
report("the repository passes", problems == [])
report("...with no allowlisted use", allowlisted == 0)
report("...having read the update client",
       "crates/evreos-platform/src/update.rs" in read)
report("...the rollout draw", "crates/evreos-platform/src/update/rollout.rs" in read)
report("...the shell", any(path.startswith("crates/evreos-shell/src/") for path in read))
report("...and the workspace manifests", "Cargo.toml" in read
       and "crates/evreos-platform/Cargo.toml" in read)

check_problems = []
entries = check.read_allowlist(check.ALLOWLIST, check_problems)
report("the committed allowlist is readable", check_problems == [])
report("...and empty in v1, so the first entry is a visible diff", entries == {})

# --- the table itself ---------------------------------------------------------

names = [name for _, name, _ in check.COMPILED]
report("every source name is unique", len(names) == len(set(names)))
report("no source name holds whitespace, which an entry could not spell",
       all(name.split() == [name] for name in names))
report("every dependency is filed under a category the sources use",
       set(check.DEPENDENCY_SOURCES.values()) <= set(check.SOURCES))
report("the eight categories T061 names are all present", len(check.SOURCES) == 8)

# --- a clean tree -------------------------------------------------------------

problems, read = tree(passing_tree())[:2]
report("a clean tree passes: Instant, a scale factor, a longer identifier and "
       "comments naming sources are not reads", problems == [])
report("...reading its Rust source and its manifest",
       read == ["crates/x/Cargo.toml", "crates/x/src/lib.rs"])

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
    ("installed fonts", "EnumFontFamilies",
     "EnumFontFamiliesExW(dc, &logfont, Some(collect), 0, 0);\n"),
    ("installed fonts", "load_system_fonts",
     "database.load_system_fonts();\n"),
    ("timezone", "iana_time_zone",
     "let zone = iana_time_zone::get_timezone();\n"),
    ("timezone", "current_local_offset",
     "let offset = time::UtcOffset::current_local_offset();\n"),
    ("timezone", "TZ",
     'let zone = std::env::var("TZ");\n'),
    ("timezone", "TZ",
     'let zone = std::env::var_os(r"TZ");\n'),
    ("timezone", "now_local",
     "let now = time::OffsetDateTime::now_local()?;\n"),
    ("timezone", "chrono::Local",
     "use chrono::{DateTime, Local, Utc};\n"),
    ("timezone", "chrono::Local",
     "let shown: DateTime<Local> = stamp.into();\n"),
    ("timezone", "chrono::Local",
     "let shown = stamp.with_timezone(&Local);\n"),
    ("timezone", "chrono::Local",
     "let shown = Local.timestamp_opt(secs, 0);\n"),
    ("timezone", "localtime",
     "let parts = unsafe { libc::localtime(&t) };\n"),
    ("timezone", "tzname",
     "let name = unsafe { libc::tzname[0] };\n"),
    ("timezone", "TimeZone::system",
     "let zone = jiff::tz::TimeZone::system();\n"),
    ("timezone", "Zoned::now",
     "let now = jiff::Zoned::now();\n"),
    ("timezone", "libc::timezone",
     "let west = unsafe { libc::timezone };\n"),
    ("total memory", "GlobalMemoryStatus",
     "GlobalMemoryStatusEx(&mut status);\n"),
    ("total memory", "/proc/meminfo",
     'let memory = read_to_string("/proc/meminfo");\n'),
    ("processor model or count", "cpuid",
     "let leaf = unsafe { core::arch::x86_64::__cpuid(0x8000_0002) };\n"),
    ("processor model or count", "available_parallelism",
     "let cores = std::thread::available_parallelism();\n"),
    ("processor model or count", "brand_string",
     'let model = sysctl("machdep.cpu.brand_string");\n'),
    ("high-resolution timing correlator", "rdtsc",
     "let ticks = unsafe { core::arch::x86_64::_rdtsc() };\n"),
    ("high-resolution timing correlator", "QueryPerformanceFrequency",
     "QueryPerformanceFrequency(&mut frequency);\n"),
    ("high-resolution timing correlator", "mach_absolute_time",
     "let now = unsafe { mach_absolute_time() };\n"),
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

problems = tree(with_rust(
    'let path = user_data.join("Local State");\nout.push_str("# Local Bookmark Store");\n'
))[0]
report("the word Local in ordinary prose is not chrono's Local", problems == [])

problems = tree(with_rust("let a = 1;\nlet b = 2;\nlet guid = MachineGuid();\n"))[0]
report("a failure names the line the read is on", mentions(problems, "probe.rs:3"))

problems = tree(passing_tree({"crates/x/notes.md": "MachineGuid, /etc/machine-id\n",
                              "crates/x/tool.py": "GetVolumeInformationW()\n"}))[0]
report("markdown and Python are not read", problems == [])

# --- the rotating salt: the derivation is caught, not its lifetime -----------

ROTATING_SALT = (
    "/// A per-install key, re-derived every day under a fresh salt, so no\n"
    "/// value it produces outlives the day it was made.\n"
    "pub fn daily_install_key(day: u32) -> [u8; 32] {\n"
    '    let guid = registry.get_string("MachineGuid");\n'
    "    let salt = day.to_be_bytes();\n"
    "    sha256(&[guid.as_bytes(), &salt].concat())\n"
    "}\n"
)
problems = tree(with_rust(ROTATING_SALT))[0]
report("a value re-derived under a daily-rotated salt is caught by the same rule",
       mentions(problems, "probe.rs:4", "'MachineGuid'"))

problems = tree(with_rust(
    "fn bucket(salt: u64) -> u64 { hash(salt ^ mach_absolute_time()) % 100 }\n"
))[0]
report("a rollout bucket drawn from a timing counter and a salt fails",
       mentions(problems, "high-resolution timing correlator", "'mach_absolute_time'"))

# --- SOURCE: script and markup ------------------------------------------------

problems, read = tree(passing_tree({
    "crates/x/ui/chrome.mjs": "const started = performance.now();\n",
}))[:2]
report("performance.now in shipped script fails",
       mentions(problems, "crates/x/ui/chrome.mjs:1", "'performance.now'"))
report("...and the script is read", "crates/x/ui/chrome.mjs" in read)

problems = tree(passing_tree({
    "crates/x/ui/index.html": "<script>send(screen.width, navigator.hardwareConcurrency)</script>\n",
}))[0]
report("screen geometry in markup fails",
       mentions(problems, "index.html:1", "screen geometry"))
report("...and so does the processor count on the same line",
       mentions(problems, "index.html:1", "'hardwareConcurrency'"))

problems = tree(passing_tree({
    "crates/x/ui/zone.TS": "const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;\n",
}))[0]
report("a script suffix in another case is still read",
       mentions(problems, "zone.TS:1", "timezone"))

problems = tree(passing_tree({
    "crates/x/ui/notes.js": "// performance.now would be a correlator here\n",
}))[0]
report("a source named in a script comment fails, the loud direction by design",
       mentions(problems, "notes.js:1", "'performance.now'"))

# --- DEPENDENCY ---------------------------------------------------------------


def with_manifest(tail):
    return passing_tree({"crates/x/Cargo.toml": CLEAN_MANIFEST + tail})


problems = tree(with_manifest('sysinfo = "0.30"\n'))[0]
report("a direct dependency on sysinfo fails",
       mentions(problems, "crates/x/Cargo.toml", "total memory", "'sysinfo'"))

problems = tree(with_manifest('system = { package = "machine-uid", version = "0.5" }\n'))[0]
report("a renamed dependency is read under its real name",
       mentions(problems, "Cargo.toml", "'machine-uid'"))

problems = tree(with_manifest("iana_time_zone = \"0.1\"\n"))[0]
report("a dependency name compares with - and _ folded",
       mentions(problems, "Cargo.toml", "timezone", "'iana-time-zone'"))

problems = tree(passing_tree({"crates/x/Cargo.toml": CLEAN_MANIFEST + (
    "\n[target.'cfg(windows)'.dependencies]\nwmi = \"0.13\"\n"
    "\n[dev-dependencies]\nfont-kit = \"0.14\"\n"
    "\n[build-dependencies]\nnum_cpus = \"1\"\n"
)}))[0]
report("a target-specific dependency fails", mentions(problems, "'wmi'"))
report("a dev-dependency fails", mentions(problems, "'font-kit'"))
report("a build-dependency fails", mentions(problems, "'num_cpus'"))

problems = tree(passing_tree({
    "Cargo.toml": '[workspace]\nmembers = []\n\n[workspace.dependencies]\nmac_address = "1"\n',
}))[0]
report("a workspace dependency fails", mentions(problems, "Cargo.toml", "'mac_address'"))

problems = tree(passing_tree({
    "Cargo.toml": (
        '[workspace]\nmembers = ["crates/x"]\n\n'
        '[workspace.dependencies]\nsys = { package = "sysinfo", version = "0.30" }\n'
    ),
    "crates/x/Cargo.toml": CLEAN_MANIFEST + "sys = { workspace = true }\n",
}))[0]
report("a member inheriting a renamed workspace dependency is read under its real name",
       mentions(problems, "crates/x/Cargo.toml", "'sysinfo'"))

problems = tree(passing_tree({"crates/x/Cargo.toml": "[package\nname = \n"}))[0]
report("a manifest that is not TOML fails rather than passing unread",
       mentions(problems, "Cargo.toml", "not TOML"))

# --- ALLOWLIST ----------------------------------------------------------------

problems, _, allowlisted = tree(
    with_rust('let zone = std::env::var("TZ");\n'),
    allowlist="# the history view shows local time\ncrates/x/src/probe.rs TZ\n",
)
report("an allowlisted use passes", problems == [])
report("...and is counted", allowlisted == 1)

problems = tree(
    with_rust('let zone = std::env::var("TZ");\nlet g = "MachineGuid";\n'),
    allowlist="crates/x/src/probe.rs TZ\n",
)[0]
report("an entry permits its source only, not every source in the file",
       mentions(problems, "probe.rs:2", "'MachineGuid'")
       and not mentions(problems, "'TZ'"))

problems = tree(
    passing_tree({"crates/x/src/other.rs": 'let zone = std::env::var("TZ");\n'}),
    allowlist="crates/x/src/probe.rs TZ\n",
)[0]
report("an entry permits its file only",
       mentions(problems, "other.rs:1", "'TZ'"))
report("...and an entry permitting nothing fails as stale",
       mentions(problems, "fingerprinting-allowlist.txt:1", "permits nothing"))

problems = tree(with_manifest('sysinfo = "0.30"\n'),
                allowlist="crates/x/Cargo.toml sysinfo\n")[0]
report("a dependency is allowlisted by manifest path and crate name", problems == [])

problems = tree(
    passing_tree({"crates/x/src/with space.rs": "let t = GetTickCount64();\n"}),
    allowlist="crates/x/src/with space.rs GetTickCount\n",
)[0]
report("an entry's path may hold a space", problems == [])

problems = tree(passing_tree(), allowlist="crates/x/src/lib.rs NotASource\n")[0]
report("an entry naming an unknown source fails",
       mentions(problems, "fingerprinting-allowlist.txt:1", "names no source"))

problems = tree(passing_tree(), allowlist="MachineGuid\n")[0]
report("an entry that is not `<path> <source>` fails",
       mentions(problems, "fingerprinting-allowlist.txt:1", "<path> <source>"))

problems = tree(
    with_rust("let t = GetTickCount64();\n"),
    allowlist="crates/x/src/probe.rs GetTickCount\ncrates/x/src/probe.rs GetTickCount\n",
)[0]
report("an entry listed twice fails",
       mentions(problems, "fingerprinting-allowlist.txt:2", "listed twice"))

problems = tree(passing_tree(), allowlist=MISSING)[0]
report("a missing allowlist fails; a missing file is not an empty one",
       mentions(problems, "missing"))

problems = tree(passing_tree(), allowlist="# comments only\n\n   \n")[0]
report("an allowlist of comments and blank lines is empty and passes", problems == [])

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
    "crates/x/target/debug/build/dep.rs": 'let g = "MachineGuid";\n',
    "crates/x/TARGET/vendor/dep.rs": 'let g = "MachineGuid";\n',
    ".git/dep.rs": 'let g = "MachineGuid";\n',
}))[0]
report("Cargo's target/ beside a manifest, in any case, and .git/ are not read",
       problems == [])

problems = tree(passing_tree({".probe/src/lib.rs": 'let g = "MachineGuid";\n'}))[0]
report("any other dot-directory is read, since a workspace member may live there",
       mentions(problems, ".probe/src/lib.rs:1", "'MachineGuid'"))

problems = tree(passing_tree({
    "crates/x/src/target/mod.rs": 'let g = "MachineGuid";\n',
    "crates/target/src/lib.rs": 'let g = "MachineGuid";\n',
}))[0]
report("a module named target is read, since Cargo builds it",
       mentions(problems, "crates/x/src/target/mod.rs:1", "'MachineGuid'"))
report("...and so is a crate named target",
       mentions(problems, "crates/target/src/lib.rs:1", "'MachineGuid'"))

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
