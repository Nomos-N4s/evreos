#!/usr/bin/env python3
"""Enforce the engine prohibition Principle III states and FR-044 spells out.

WHAT THIS CHECKS, and why each clause exists.

Principle III: "The shell MUST be stable Rust, with no nightly features on the
release path. Electron, CEF and any bundled Chromium are permanently rejected".
FR-044 extends the second sentence to any web engine Evreos itself fetches,
unpacks or installs -- at first run, on update or on demand -- and counts such
an engine as bundled whether or not it is present in the build output. Until
this check that was the one Principle-derived prohibition in the specification
with nothing enforcing it: the seam is proved by a second implementation
building in CI, the budgets by scripts/check-budgets.py, and the engine
prohibition by review alone. This check reads the tree and fails on:

  DEPENDENCY   a dependency, direct or transitive, on any crate named in
               scripts/checks/prohibited-engines.txt, read from the graph
               `cargo metadata` resolves with every feature enabled and every
               target's dependencies included. The graph rather than the
               manifests, because a rejected engine two dependencies down is
               as bundled as one named in evreos-shell's own Cargo.toml. Dev-
               and build-dependencies are in that graph and are not excused:
               a permanently rejected engine has no test-only use here, and an
               entry that excused one would be exactly the diff a review has
               to see. A declared dependency the graph does not reach -- one
               behind a target this machine is not -- fails on its name alone.
               The failure names the chain that reaches the crate.

  NIGHTLY      any file on the release path that selects a toolchain other
               than stable, or turns nightly features on: a toolchain file
               whose channel is nightly or beta, or that points at a custom
               toolchain; a Cargo configuration with an [unstable] table, a -Z
               flag, or an [env] entry setting RUSTC_BOOTSTRAP or pointing
               RUSTUP_TOOLCHAIN at nightly or beta; a manifest with
               cargo-features; and a Rust source file carrying an inner
               attribute that names a feature -- `#![feature(...)]`, or the
               same attribute behind a switch, `#![cfg_attr(.., feature(..))]`.
               Every .rs file of every member is read, tests included: the
               toolchain is one per workspace, so a single file that needs
               nightly to compile puts the whole workspace on it. A workflow
               is read too, since it is what actually puts a toolchain on the
               release path, and fails on each way one selects nightly or
               beta: installing it (`rustup install`, `rustup toolchain
               install`), defaulting to it (`rustup default`, `rustup
               override set`), invoking it (`cargo +nightly`, `rustup run
               nightly`), naming it in a `toolchain:` input or through a
               toolchain action pinned `@nightly`, setting RUSTUP_TOOLCHAIN to
               it, or listing it under a build-matrix key -- inline or as a
               block list, since a matrix is where the channel is spelled
               when the step that uses it reads an expression. RUSTC_BOOTSTRAP
               and CARGO_UNSTABLE_* variables, which turn nightly behaviour
               on under a stable toolchain, and a -Z flag on a cargo or rustc
               line fail on the same ground.

  ACQUISITION  a line on a workspace crate's build script or runtime path --
               its manifest, its build script, everything under src/ and any
               helper under build/ -- or in a workflow, that names a web
               engine and on the same line does something that acquires
               bytes: a URL, an archive or package suffix, a compile-time
               embedding such as include_bytes!, a vendored path, or one of
               the verbs FR-044 uses -- fetch, unpack, install -- in their
               everyday spellings. Both halves are required. An engine's name
               alone is not a breach: a Linux backend links the system
               WebKitGTK by name. A fetch alone is not one: the update check
               fetches. Workflows are read because packaging is part of the
               release path, and an archive a workflow unpacks into the
               installer ships as surely as one build.rs fetched. A crate's
               tests/, benches/ and examples/ are not its runtime path and are
               not read here; a vendored archive elsewhere in the crate is
               binary and fetches nothing until a line on the runtime path
               names it, which is the line that fails.

THE DENY-LIST is the plain-text form scripts/checks/README.md fixes for a
committed list -- one entry per line, `#` starting a comment, blank lines
ignored -- with one deliberate narrowing: it may not be empty. The convention
lets a list be empty so that a first entry lands as a diff to the list rather
than to code, which is the right rule for an allowlist, whose empty state is
its strictest. A deny-list's empty state is its weakest -- the DEPENDENCY
clause becomes a check over nothing and passes -- and this list is seeded from
the day it lands, so an empty file can only mean every name was deleted, a
change that should fail loudly rather than pass quietly. The narrowing is this
check's and is stated here; the convention itself is unchanged.

THE ONE CARVE-OUT, which FR-044 states and this check encodes. Triggering the
installation of the operating system's own shared web runtime -- which SC-003
requires at first run where that runtime is absent -- is not acquiring an
engine of Evreos's own. FR-044: "what the carve-out turns on is that the
runtime is the platform's and shared, never on who starts the installer." So
the names of the platforms' shared runtimes and of their installers are not
engine names here; they are SHARED_RUNTIME, and are blanked from a line before
the engine-name test runs, so that a first-run bootstrap which downloads the
Evergreen Bootstrapper, reports its progress and runs it passes however it
spells those steps. What is not shared has its own name and is an engine: the
WebView2 Fixed Version Runtime is a private copy of the runtime shipped with
the application -- which ADR-0001 records Microsoft's own documentation putting
at over 250 MB -- precisely the bundle Principle III rejects, so a
FixedVersionRuntime archive fails. On Linux the shared runtime and a private
build share the name WebKitGTK, so the name is an engine name and the shared
case passes on the absence of an acquisition marker, as above. The tests prove
both sides: the bootstrap path passes, a vendored engine archive fails. What
the carve-out costs the size budgets is SC-001's to count and is counted there.

WHAT THIS DOES NOT CATCH, stated so nothing is assumed of it.

The acquisition clause keys on an engine's name on the same line as the
acquisition. A fetch spelled without the name -- a URL held in a table, an
archive named by its digest -- passes this scan and rests on review, and on
SC-001's installed-footprint gate, which counts the bytes wherever they came
from and is the backstop FR-044 itself points to. The clause also fires on a
name beside a verb it did not mean: a package-manager invocation naming the
distribution's WebKitGTK package would fail here although the carve-out
permits it. No such path exists, Linux being a separate decision under
ADR-0001; when one does the remedy is a pattern added to SHARED_RUNTIME, a
visible diff, not a quieter spelling.

The nightly clause reads a workflow line by line and keys on the channel's
name. A channel that reaches a step only through an expression whose source is
not a matrix list under one of the keys TOOLCHAIN_KEYS names -- a repository
variable, a `workflow_dispatch` input, a step output, a matrix under an
unusual key -- passes, as does a channel spelled by concatenation, and a
`rustup` call made by a script outside the tree. Those rest on review and on
the toolchain step build.yml carries, which installs stable and makes it the
default before anything builds, so the only way onto nightly is a later line
this clause does read.

Neither clause reads a comment: a comment fetches nothing and selects nothing.
"""
import argparse
import json
import re
import subprocess
import sys
import tomllib
from collections import deque
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = Path(__file__).resolve().parents[2]
DENYLIST = HERE / "prohibited-engines.txt"

# The directories of a member that are its runtime path or hold its build
# helpers. tests/, benches/ and examples/ are neither; target/ is build output.
RUNTIME_PATH = ("src", "build")

# --- nightly ------------------------------------------------------------------

# A toolchain channel that is not stable Rust. `stable`, `stable-<target>` and
# a version such as `1.85.0` pass; the classification is rustup's own.
UNSTABLE_CHANNEL = re.compile(r"^\s*(nightly|beta)\b", re.IGNORECASE)

# A non-stable channel wherever it stands in a value: `nightly`, `"beta"`,
# `nightly-2026-08-01`. Bounded on the left by anything but a word character or
# a hyphen, so `alphabeta` and `pre-beta` are not channels.
UNSTABLE_VALUE = re.compile(r"(?<![\w-])(?:nightly|beta)\b", re.IGNORECASE)

# An inner attribute that names a feature: `#![feature(...)]`, or the same
# attribute behind a switch, `#![cfg_attr(..., feature(...))]`. The paren is
# what separates it from `#![cfg(feature = "x")]`, which is a Cargo feature and
# stable.
# Bounded so the joined window cannot carry `#![` from one line to an unrelated
# `feature(` several lines later: no `"` and no `;` may fall between them, and
# neither appears inside a real attribute head. Without the bound, a string
# constant holding "#![" beside a function named `feature` read as a nightly
# feature gate.
FEATURE_ATTRIBUTE = re.compile(r'#!\[[^\]";]*\bfeature\s*\(')

# What a workflow does, on a command line or in a variable, to put the release
# path on a non-stable toolchain. The rustup forms accept any flags and values
# between the verb and the channel, up to a command separator, so the channel
# is found wherever the flags leave it.
WORKFLOW_NIGHTLY = re.compile(
    r"\+(?:nightly|beta)\b"
    r"|\brustup\s+(?:toolchain\s+)?(?:install|default|run|update)\s+"
    r"(?:[^\s;&|]+\s+)*?[\"']?(?:nightly|beta)\b"
    r"|\brustup\s+override\s+set\s+(?:[^\s;&|]+\s+)*?[\"']?(?:nightly|beta)\b"
    r"|\bRUSTUP_TOOLCHAIN\s*[:=]\s*[\"']?(?:nightly|beta)\b"
    r"|rust-toolchain@(?:nightly|beta)\b"
    r"|\bRUSTC_BOOTSTRAP\b"
    r"|\bCARGO_UNSTABLE_\w+",
    re.IGNORECASE,
)
# A -Z flag on a line that drives cargo or rustc. `-Z` alone is too common a
# letter in other tools' flags to match on its own.
WORKFLOW_UNSTABLE_FLAG = re.compile(
    r"(?=.*\b(?:cargo|rustc|RUSTFLAGS|RUSTDOCFLAGS)\b).*(?:^|[\s=\"'])-Z\s*[a-z]",
    re.IGNORECASE,
)

# The keys under which a workflow names a toolchain as a value: the
# `toolchain` input of the toolchain actions, and the names a build matrix
# ordinarily gives its Rust axis. A value under one of these that is nightly or
# beta -- a scalar, an inline list, or the items of a block list beneath it --
# selects that channel for whatever step reads it. `branches`, `tags` and the
# like are absent on purpose: a branch named beta is not a toolchain.
TOOLCHAIN_KEYS = {
    "toolchain", "rust", "rust-toolchain", "rust_toolchain", "channel",
    "rust-version", "rust_version", "rustc", "version",
}
# A YAML mapping line, `key: value` or `- key: value`, and a block-list item.
# Enough YAML for a workflow; nothing here is a parser.
YAML_KEY = re.compile(r"""^(\s*)(?:-\s+)?["']?([A-Za-z_][\w.-]*)["']?\s*:(?!:)(.*)$""")
YAML_ITEM = re.compile(r"""^(\s*)-\s+(.*)$""")

# --- acquisition --------------------------------------------------------------

# The engines FR-044 rejects, by the names they carry in a URL, an archive, a
# crate or a function. Deliberately absent: `chrome`, which is the browser's
# own chrome in a browser project; `blink`, which is what a caret does;
# `firefox`, one of the import sources the Assumptions name for FR-012's
# bookmark and history import; and the shared runtimes, which SHARED_RUNTIME
# holds and the docstring explains. `servo` is present: compiling it in is the
# experimental backend Principle III allows, fetching a build of it at run
# time is not. `cef` is bounded by anything but a letter or digit, so
# `download_cef` and `cef-sys` are seen and `cefalo` is not; a plain word
# boundary would treat the underscore as part of the word. The last entry is
# Microsoft's own name for the app-local WebView2 distribution.
ENGINE_NAMES = re.compile(
    r"chromium"
    r"|(?<![a-z0-9])cef(?![a-z0-9])|libcef"
    r"|electron"
    r"|gecko|libxul"
    r"|webkitgtk|webkit2gtk|libwebkit|wpewebkit"
    r"|qtwebengine"
    r"|servo"
    r"|fixed[\s_-]?version[\s_-]?runtime",
    re.IGNORECASE,
)

# The platforms' shared web runtimes and the installers that put them there.
# Blanked from a line before ENGINE_NAMES is tested, which is the whole of the
# carve-out's mechanism: WebView2 is the tier-1 runtime and its Evergreen
# Bootstrapper and Standalone Installer are Microsoft's installers for it;
# WKWebView is the tier-2 runtime and is always present. None of these may
# match ENGINE_NAMES, and the tests hold that invariant.
SHARED_RUNTIME = re.compile(
    r"MicrosoftEdgeWebview2Setup(?:\.exe)?"
    r"|MicrosoftEdgeWebView2RuntimeInstaller\w*(?:\.exe)?"
    r"|webview2"
    r"|wkwebview",
    re.IGNORECASE,
)

# What, beside an engine's name, makes a line an acquisition.
ACQUISITION = re.compile(
    r"https?://"
    r"|\.(?:zip|7z|rar|tar|tgz|txz|tbz2?|gz|xz|bz2|zst|cab|msi|msix|nupkg|dmg"
    r"|pkg|deb|rpm|appimage)\b"
    r"|include_(?:bytes|str|dir)!|include_flate"
    r"|\b(?:vendor|vendored|third[_-]party|bundled)/"
    # Not `\b`: a word boundary treats the underscore as a word character, so
    # `\b` never fires after one and a verb suffixed to an engine's name --
    # `cef_download`, `electron_fetch`, `chromium_unpack` -- would not be seen
    # as an acquisition at all. The engine names above are bounded the same way
    # and for the same reason.
    r"|(?<![A-Za-z0-9])(?:download|fetch|unpack|extract|unzip|untar"
    r"|decompress|install)\w*",
    re.IGNORECASE,
)

# How a comment starts in each kind of file the acquisition clause reads. A
# file with no entry is read whole. `.md` and `.txt` are prose and are not read:
# a sentence forbidding a Chromium archive is not one acquiring it.
COMMENT_STYLE = {
    ".rs": "rust",
    ".toml": "hash",
    ".sh": "hash", ".bash": "hash", ".zsh": "hash",
    ".py": "hash",
    ".yml": "hash", ".yaml": "hash",
    ".ps1": "hash",
    ".cfg": "hash", ".ini": "hash", ".in": "hash", ".mk": "hash", ".cmake": "hash",
}
HASH_NAMED = {"Makefile", "justfile", "Justfile"}
NOT_READ = {".md", ".txt", ".lock"}


class CheckError(Exception):
    """The check cannot reach a verdict. An unrun check is not a pass."""


def relative(root, path):
    try:
        return Path(path).resolve().relative_to(root).as_posix()
    except ValueError:
        return Path(path).as_posix()


def normalise(name):
    """crates.io treats names case-insensitively and `-` as `_`; so does this."""
    return name.strip().lower().replace("_", "-")


# --- deny-list ----------------------------------------------------------------

def read_denylist(path):
    """One crate name per line; `#` starts a comment; blank lines are ignored.

    Returns (names, problems). A missing file is a CheckError: the list is a
    committed input read on every run, and a missing input is not an empty one.
    An empty list is a problem rather than a pass -- the narrowing of the
    README's convention the docstring states -- because it makes the
    DEPENDENCY clause a check over nothing.
    """
    path = Path(path)
    if not path.is_file():
        raise CheckError(
            f"{path.name}: missing; the deny-list is a committed file read on every run"
        )
    names, problems = [], []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        entry = line.split("#", 1)[0].strip()
        if not entry:
            continue
        if not re.fullmatch(r"[A-Za-z0-9_-]+", entry):
            problems.append(f"{path.name}:{number}: {entry!r} is not a crate name")
            continue
        name = normalise(entry)
        if name in names:
            problems.append(f"{path.name}:{number}: {entry} is listed twice")
        names.append(name)
    if not names:
        problems.append(
            f"{path.name}: names no crate; an empty deny-list is a dependency clause over "
            "nothing, and this list is seeded, so an empty file means every name was deleted"
        )
    return names, problems


# --- cargo metadata -----------------------------------------------------------

def cargo_metadata(root):
    """The graph Cargo resolves for the workspace at `root`.

    Every feature, so an engine behind an optional dependency is seen; every
    target, which is `cargo metadata`'s default; and --locked, so the verdict
    is about the Cargo.lock that is committed rather than about a graph this
    run resolved and nobody reviewed.
    """
    command = ["cargo", "metadata", "--format-version", "1", "--all-features", "--locked"]
    try:
        completed = subprocess.run(command, cwd=root, capture_output=True, text=True)
    except FileNotFoundError:
        raise CheckError("cargo not found; the DEPENDENCY clause reads `cargo metadata`") from None
    if completed.returncode != 0:
        detail = (completed.stderr or "").strip().splitlines()
        raise CheckError(
            "`cargo metadata` failed: " + (detail[-1] if detail else f"exit {completed.returncode}")
        )
    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise CheckError(f"`cargo metadata` produced no JSON: {error}") from None


def load_metadata_file(path):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        raise CheckError(f"{path}: {error}") from None


def member_packages(metadata):
    """The workspace members, as package records, in workspace order."""
    by_id = {package["id"]: package for package in metadata.get("packages", [])}
    members = []
    for identifier in metadata.get("workspace_members", []):
        package = by_id.get(identifier)
        if package is None:
            raise CheckError(
                f"workspace member {identifier} is not among the packages in the metadata"
            )
        members.append(package)
    return members


def member_directories(metadata):
    return [Path(package["manifest_path"]).parent for package in member_packages(metadata)]


def prohibited_dependencies(metadata, denylist):
    """DEPENDENCY: every chain from a member to a crate on the deny-list.

    Walks the resolved graph outward from each member and reports the first
    chain that reaches each prohibited crate. Then reads every package's
    declared dependencies, so a name the resolve did not reach -- an optional
    dependency, or one behind a target this machine is not -- fails too.
    """
    denied = set(denylist)
    by_id = {package["id"]: package for package in metadata.get("packages", [])}
    resolve = metadata.get("resolve") or {}
    nodes = resolve.get("nodes", [])
    edges = {node["id"]: [dep["pkg"] for dep in node.get("deps", [])] for node in nodes}
    if not edges:
        # Older shapes carry `dependencies` instead of `deps`; read either.
        edges = {node["id"]: list(node.get("dependencies", [])) for node in nodes}

    def name_of(identifier):
        return by_id.get(identifier, {}).get("name", identifier)

    problems = []
    for member in member_packages(metadata):
        parent = {member["id"]: None}
        queue = deque([member["id"]])
        while queue:
            current = queue.popleft()
            for dependency in edges.get(current, []):
                if dependency in parent:
                    continue
                parent[dependency] = current
                if normalise(name_of(dependency)) in denied:
                    chain = []
                    node = dependency
                    while node is not None:
                        chain.append(name_of(node))
                        node = parent[node]
                    chain.reverse()
                    how = "directly" if len(chain) == 2 else "through " + " -> ".join(chain[1:-1])
                    problems.append(
                        f"{member['name']} depends on {name_of(dependency)} {how}: "
                        + " -> ".join(chain)
                        + "; Principle III rejects it permanently"
                    )
                queue.append(dependency)

    reached = {normalise(name_of(identifier)) for identifier in edges}
    for package in metadata.get("packages", []):
        for declared in package.get("dependencies", []):
            name = normalise(declared.get("name", ""))
            if name in denied and name not in reached:
                problems.append(
                    f"{package['name']} declares a dependency on {declared['name']} that the "
                    "resolved graph does not reach (optional, or behind a target this machine "
                    "is not); the name is prohibited either way"
                )
    return problems


# --- reading files ------------------------------------------------------------

def strip_comment(line, style):
    """The part of `line` before its comment, leaving string literals whole.

    A URL is `//` inside a string, and an attribute is `#` outside one, so a
    comment is recognised only outside quotes. `style` is "rust" for `//` and
    `/* */`, "hash" for `#`, or None to leave the line as it is.
    """
    if style is None:
        return line
    out = []
    quote = None
    i, n = 0, len(line)
    while i < n:
        ch = line[i]
        if quote:
            out.append(ch)
            if ch == "\\" and i + 1 < n:
                out.append(line[i + 1])
                i += 2
                continue
            if ch == quote:
                quote = None
            i += 1
            continue
        if ch == '"' or (ch == "'" and style == "hash"):
            quote = ch
        elif ch == "'" and style == "rust":
            # A char literal, `'x'` or `'\n'`; a lifetime is left alone.
            if i + 2 < n and line[i + 2] == "'":
                out.append(line[i:i + 3])
                i += 3
                continue
            if i + 3 < n and line[i + 1] == "\\" and line[i + 3] == "'":
                out.append(line[i:i + 4])
                i += 4
                continue
        elif style == "rust" and line.startswith("//", i):
            break
        elif style == "rust" and line.startswith("/*", i):
            end = line.find("*/", i + 2)
            if end == -1:
                break
            i = end + 2
            continue
        elif style == "hash" and ch == "#":
            break
        out.append(ch)
        i += 1
    return "".join(out)


def read_text(path):
    """The file's text, or None when it is binary or unreadable."""
    try:
        data = Path(path).read_bytes()
    except OSError:
        return None
    if b"\0" in data:
        return None
    return data.decode("utf-8", errors="replace")


def comment_style(path):
    path = Path(path)
    if path.name in HASH_NAMED:
        return "hash"
    return COMMENT_STYLE.get(path.suffix)


def code_lines(path):
    """(line number, line with its comment removed) for every line of `path`."""
    text = read_text(path)
    if text is None:
        return []
    style = comment_style(path)
    return [
        (number, strip_comment(line, style))
        for number, line in enumerate(text.splitlines(), 1)
    ]


def load_toml(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def workflow_files(root):
    directory = root / ".github" / "workflows"
    return sorted(p for p in directory.glob("*.y*ml") if p.is_file()) if directory.is_dir() else []


def files_under(directory, skip=()):
    """Every regular file under `directory`, skipping the named top-level dirs."""
    found = []
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        parts = path.relative_to(directory).parts
        if parts[0] in skip:
            continue
        found.append(path)
    return found


# --- nightly ------------------------------------------------------------------

def check_toolchain_file(root, path, problems):
    where = relative(root, path)
    text = read_text(path)
    if text is None:
        return
    # Discriminate on CONTENT, not on the extension. rustup honours the TOML
    # form in the extensionless `rust-toolchain` too, and reading that file's
    # first line as a channel yields the literal "[toolchain]", which matches
    # no channel pattern -- so the single file that most directly puts the
    # release path on nightly was read and then silently misparsed.
    # Parse as TOML and fall back to the legacy one-line reading only when the
    # file is not TOML at all. A prefix comparison is not content
    # discrimination: a comment line above the header -- the ordinary way a
    # human writes this file -- and the equally valid `[ toolchain ]`, a UTF-8
    # BOM, and the dotted `toolchain.channel = ...` all fell through it to the
    # legacy reading, which takes line one as a channel and so matched nothing.
    # Parsed from the decoded text with any byte-order mark removed, not from
    # the file handle: tomllib rejects a BOM, so a BOM'd file fell through to
    # the legacy reading and its first line read as "\ufeff[toolchain]", which
    # matches no channel. An editor that writes one is not a way past this.
    stripped = text.lstrip("\ufeff").strip()
    try:
        parsed = tomllib.loads(stripped)
    except tomllib.TOMLDecodeError:
        parsed = None
    if parsed is not None and "toolchain" in parsed:
        toolchain = parsed["toolchain"]
        if not isinstance(toolchain, dict):
            problems.append(
                f"{where}: [toolchain] is {type(toolchain).__name__}, not a table; "
                "a verdict over a misread file is a verdict on nothing"
            )
            return
        channel = str(toolchain.get("channel", ""))
        if "path" in toolchain:
            problems.append(
                f"{where}: selects a custom toolchain at {toolchain['path']!r}; the release "
                "path builds on the stable channel and nothing this check can classify"
            )
    elif parsed is None:
        # Not TOML: the legacy form, whose whole content is a channel name.
        channel = stripped.splitlines()[0] if stripped else ""
    else:
        # Valid TOML with no [toolchain] table. rustup honours neither, so
        # there is no channel here to judge.
        channel = ""
    if UNSTABLE_CHANNEL.match(channel):
        problems.append(
            f"{where}: selects the {channel.strip()!r} toolchain; Principle III requires stable "
            "Rust with no nightly features on the release path"
        )


def flags_of(value):
    if isinstance(value, str):
        return value.split()
    if isinstance(value, list):
        return [str(item) for item in value]
    return []


def check_cargo_config(root, path, problems):
    where = relative(root, path)
    try:
        config = load_toml(path)
    except (tomllib.TOMLDecodeError, OSError) as error:
        problems.append(f"{where}: {error}")
        return
    if "unstable" in config:
        problems.append(f"{where}: carries an [unstable] table, which only a nightly Cargo reads")
    tables = [("build", config.get("build", {}))]
    tables += [(f"target.{name}", table) for name, table in config.get("target", {}).items()]
    for label, table in tables:
        if not isinstance(table, dict):
            continue
        for key in ("rustflags", "rustdocflags"):
            for flag in flags_of(table.get(key)):
                if flag.startswith("-Z"):
                    problems.append(
                        f"{where}: [{label}] {key} passes {flag!r}, a nightly-only flag"
                    )
        for key in ("rustc", "rustc-wrapper", "rustc-workspace-wrapper"):
            if "nightly" in str(table.get(key, "")).lower():
                problems.append(f"{where}: [{label}] {key} names a nightly compiler")
    env = config.get("env", {})
    if not isinstance(env, dict):
        env = {}
    if "RUSTC_BOOTSTRAP" in env:
        problems.append(
            f"{where}: sets RUSTC_BOOTSTRAP, which turns nightly features on under a stable "
            "compiler; that is a nightly feature on the release path however it is switched on"
        )
    toolchain = env.get("RUSTUP_TOOLCHAIN")
    if isinstance(toolchain, dict):
        toolchain = toolchain.get("value", "")
    if toolchain is not None and UNSTABLE_VALUE.search(str(toolchain)):
        problems.append(
            f"{where}: [env] points RUSTUP_TOOLCHAIN at {str(toolchain)!r}, which is what every "
            "rustc this Cargo spawns through the rustup proxy would then run"
        )


def check_manifest_features(root, path, problems):
    where = relative(root, path)
    try:
        manifest = load_toml(path)
    except (tomllib.TOMLDecodeError, OSError) as error:
        problems.append(f"{where}: {error}")
        return
    if "cargo-features" in manifest:
        problems.append(
            f"{where}: declares cargo-features = {manifest['cargo-features']!r}, which only a "
            "nightly Cargo honours"
        )


def workflow_nightly_lines(lines):
    """The (number, line) pairs of a workflow that select nightly or beta.

    Three readings of each comment-free line. The command-line and variable
    forms are WORKFLOW_NIGHTLY and WORKFLOW_UNSTABLE_FLAG. The value forms need
    the key a value sits under, which for a block-list item is on an earlier
    line, so the walk keeps the chain of keys enclosing the current line by
    indentation -- the one piece of YAML structure this needs -- and judges a
    scalar, an inline list or a block item by the key it belongs to.
    """
    found = []
    enclosing = []  # (indent, key), strictly increasing in indent
    for number, line in lines:
        if not line.strip():
            continue
        if WORKFLOW_NIGHTLY.search(line) or WORKFLOW_UNSTABLE_FLAG.search(line):
            found.append((number, line))
            continue
        key_match = YAML_KEY.match(line)
        if key_match:
            indent, key, value = len(key_match.group(1)), key_match.group(2), key_match.group(3)
            if key_match.group(0)[indent:].startswith("-"):
                indent += 2  # `- key: value` places the key two columns in
            while enclosing and enclosing[-1][0] >= indent:
                enclosing.pop()
            enclosing.append((indent, key))
            if key in TOOLCHAIN_KEYS and UNSTABLE_VALUE.search(value):
                found.append((number, line))
            continue
        item_match = YAML_ITEM.match(line)
        if item_match:
            indent, value = len(item_match.group(1)), item_match.group(2)
            parents = [key for at, key in enclosing if at <= indent]
            if parents and parents[-1] in TOOLCHAIN_KEYS and UNSTABLE_VALUE.search(value):
                found.append((number, line))
    return found


def check_workflow_nightly(root, path, problems):
    where = relative(root, path)
    for number, line in workflow_nightly_lines(code_lines(path)):
        problems.append(
            f"{where}:{number}: puts the toolchain on a non-stable channel or turns nightly "
            f"features on: {line.strip()!r}"
        )


def check_rust_source_nightly(root, path, problems):
    """Feature attributes and RUSTC_BOOTSTRAP in one crate root.

    The feature attribute is matched over a JOINED window rather than one line
    at a time. rustfmt splits a long `#![cfg_attr(nightly, feature(...))]`
    across lines, and a per-line match needs `#![` and `feature(` on the same
    one -- so the split form, which is what a formatter actually produces, was
    not caught while the single-line form was. The window is the comment-free
    lines of the file joined with spaces; the reported line is the one the
    attribute opens on.
    """
    where = relative(root, path)
    lines = list(code_lines(path))
    joined, offsets = [], []
    for number, line in lines:
        offsets.append((len("".join(joined)) + len(joined), number))
        joined.append(line.strip())
    window = " ".join(joined)

    def line_of(position):
        found = lines[0][0] if lines else 0
        for start, number in offsets:
            if start <= position:
                found = number
            else:
                break
        return found

    for match in FEATURE_ATTRIBUTE.finditer(window):
        number = line_of(match.start())
        problems.append(
            f"{where}:{number}: carries a feature attribute, "
            f"{window[match.start():match.start() + 60].strip()!r}; nightly "
            "features are not permitted on the release path"
        )
    for number, line in lines:
        if "RUSTC_BOOTSTRAP" in line:
            problems.append(
                f"{where}:{number}: names RUSTC_BOOTSTRAP, whose only use is nightly features"
            )


def scan_nightly(root, member_dirs):
    """NIGHTLY over the whole release path. Returns (problems, files read)."""
    root = Path(root).resolve()
    problems = []
    read = 0
    directories = [root] + [Path(d) for d in member_dirs]
    for directory in directories:
        for name in ("rust-toolchain.toml", "rust-toolchain"):
            path = directory / name
            if path.is_file():
                check_toolchain_file(root, path, problems)
                read += 1
        for name in ("config.toml", "config"):
            path = directory / ".cargo" / name
            if path.is_file():
                check_cargo_config(root, path, problems)
                read += 1
        manifest = directory / "Cargo.toml"
        if manifest.is_file():
            check_manifest_features(root, manifest, problems)
            read += 1
    for path in workflow_files(root):
        check_workflow_nightly(root, path, problems)
        read += 1
    for directory in member_dirs:
        for path in files_under(Path(directory), skip={"target"}):
            if path.suffix == ".rs":
                check_rust_source_nightly(root, path, problems)
                read += 1
    return problems, read


# --- acquisition --------------------------------------------------------------

def acquires_engine(line):
    """Whether one comment-free line names an engine and acquires bytes.

    The shared runtimes and their installers are blanked first, so a line
    about them is judged on whatever else it names.
    """
    remainder = SHARED_RUNTIME.sub(" ", line)
    return bool(ENGINE_NAMES.search(remainder)) and bool(ACQUISITION.search(line))


def check_acquisition_file(root, path, problems):
    where = relative(root, path)
    for number, line in code_lines(path):
        if acquires_engine(line):
            problems.append(
                f"{where}:{number}: fetches, unpacks, embeds or installs a web engine, which "
                f"FR-044 counts as bundling: {line.strip()!r}"
            )


def acquisition_files(metadata, root):
    """Every file the ACQUISITION clause reads, in a stable order.

    Per member: its manifest, its build script -- build.rs by convention, and
    whatever `cargo metadata` reports as the custom-build target, which may
    sit outside the crate directory -- and every file under src/ and build/.
    Then every workflow. The set is named rather than "everything under the
    crate" so that a package at the workspace root would not put the whole
    repository, this check's own tests included, on the runtime path.
    """
    files = []

    def add(path):
        if path.is_file() and path.suffix not in NOT_READ and path not in files:
            files.append(path)

    for package in member_packages(metadata):
        directory = Path(package["manifest_path"]).parent
        add(directory / "Cargo.toml")
        add(directory / "build.rs")
        for target in package.get("targets", []):
            if "custom-build" in target.get("kind", []):
                add(Path(target["src_path"]))
        for name in RUNTIME_PATH:
            if (directory / name).is_dir():
                for path in files_under(directory / name):
                    add(path)
    for path in workflow_files(Path(root)):
        add(path)
    return files


def scan_acquisition(metadata, root):
    """ACQUISITION over every build script, runtime path and workflow."""
    root = Path(root).resolve()
    problems = []
    files = acquisition_files(metadata, root)
    for path in files:
        check_acquisition_file(root, path, problems)
    return problems, len(files)


# --- the whole check ----------------------------------------------------------

def check_repository(root, denylist_path, metadata=None):
    """Run the three clauses. Returns (problems, summary); no problems is a pass.

    Raises CheckError when a verdict cannot be reached.
    """
    root = Path(root).resolve()
    if not (root / "Cargo.toml").is_file():
        raise CheckError(f"{root}: no Cargo.toml; there is no workspace to check")
    denylist, problems = read_denylist(denylist_path)
    if metadata is None:
        metadata = cargo_metadata(root)
    members = member_directories(metadata)
    if not members:
        raise CheckError(
            "the metadata names no workspace member; a check over nothing is not a pass"
        )

    problems += prohibited_dependencies(metadata, denylist)
    nightly, nightly_read = scan_nightly(root, members)
    problems += nightly
    acquisition, acquisition_read = scan_acquisition(metadata, root)
    problems += acquisition

    summary = (
        f"{len(members)} crates, {len(metadata.get('packages', []))} packages in the graph, "
        f"{len(denylist)} prohibited names, {nightly_read + acquisition_read} files read"
    )
    return problems, summary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="workspace root; the repository by default"
    )
    parser.add_argument(
        "--denylist", default=str(DENYLIST), help="the prohibited-crates list to read"
    )
    parser.add_argument(
        "--metadata",
        help="a saved `cargo metadata --format-version 1` JSON to read instead of running cargo",
    )
    args = parser.parse_args()

    try:
        metadata = load_metadata_file(args.metadata) if args.metadata else None
        problems, summary = check_repository(args.root, args.denylist, metadata)
    except CheckError as error:
        print(f"Engine prohibition check could not run: {error}", file=sys.stderr)
        return 2

    if problems:
        for problem in problems:
            print(f"  FAIL  {problem}", file=sys.stderr)
        print(
            f"\nEngine prohibition check FAILED: {len(problems)} breach(es). See Principle III "
            "of .specify/memory/constitution.md, FR-044 in specs/001-evreos-v1/spec.md, and "
            "the docstring of scripts/checks/check_engine_prohibition.py for the three clauses "
            "and the one carve-out.",
            file=sys.stderr,
        )
        return 1

    print(f"Engine prohibition check passed: {summary}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
