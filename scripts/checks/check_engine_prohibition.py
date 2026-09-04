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
               release path -- and a LOCAL ACTION DEFINITION is read on the
               same terms, since a step written `uses: ./path` hands the runner
               whatever is defined there and a COMPOSITE action's `run:` steps
               are shell exactly as a workflow's are. What is read is the
               definition file; a node or docker action's own program is not,
               and the limits section below says so. It fails on each way one
               selects nightly or beta: installing it (`rustup install`, `rustup toolchain
               install`), defaulting to it (`rustup default`, `rustup
               override set`), invoking it (`cargo +nightly`, `rustup run
               nightly`), naming it in a `toolchain:` input or through a
               toolchain action pinned `@nightly`, setting RUSTUP_TOOLCHAIN to
               it, or listing it under a build-matrix key, since a matrix is
               where the channel is spelled when the step that uses it reads an
               expression. A value is read in every shape YAML gives it: an
               inline scalar, a block list, a flow mapping or sequence on one
               line or wrapped over several, and a value written below its key.
               RUSTC_BOOTSTRAP
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
               installer ships as surely as one build.rs fetched. Local actions are read here too, and
               for the same reason. A crate's
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

The nightly clause reads a workflow a logical line at a time -- a physical line
with its continuations folded in -- and keys on the channel's name. A channel
that reaches a step only through an expression whose source is
not a matrix list under one of the keys TOOLCHAIN_KEYS names -- a repository
variable, a `workflow_dispatch` input, a step output, a matrix under an
unusual key -- passes, as does a channel spelled by concatenation, and a
`rustup` call made by a script outside the tree. Those rest on review and on
the toolchain step build.yml carries, which installs stable and makes it the
default before anything builds, so the only way onto nightly is a later line
this clause does read.

A LOCAL ACTION is read as its definition file and no further. That is the whole
of a composite action, whose steps are written there. It is not the whole of a
node action, whose `main`, `pre` and `post` name JavaScript files, nor of a
docker action, whose `image` may name a Dockerfile -- and neither of those files
is read, so a `rustup default nightly` inside one passes. Nothing in this
repository is such an action; closing it is a change to the file set, and it
lands with the first one, where the diff that adds the action and the diff that
adds its reading are the same review. A THIRD-PARTY action's code is not in the
tree and cannot be read at all: what is read of one is the reference, which is
why a `@nightly` pin and a `toolchain:` input are matched where they are
written.

Neither clause reads a comment: a comment fetches nothing and selects nothing.
"""
import argparse
import json
import os
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
# The window this is matched over has string literals blanked first, so a
# constant holding "#![" cannot reach an unrelated `feature(` below it. Bounding
# the pattern instead -- excluding `"` between the two -- looked equivalent and
# was not: it dropped `#![cfg_attr(feature = "nightly", feature(let_chains))]`,
# which is the standard way a crate puts nightly features behind a Cargo
# feature and so the commonest real gate there is. The `;` exclusion stays:
# no statement terminator falls inside one attribute head.
# Whitespace may sit between `#`, `!` and `[`, in any combination: rustc reads
# `#![feature(x)]`, `#! [feature(x)]`, `# ![feature(x)]` and `#!` on its own
# line as one and the same attribute -- verified against the compiler, which
# rejects every spelling on stable with the same E0554. Closing only the gap
# between `!` and `[` left the one to its left open, and a single space there
# still put the release path on nightly unseen.
FEATURE_ATTRIBUTE = re.compile(r'#\s*!\s*\[[^\];]*\bfeature\s*\(')


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
# Enough YAML for a workflow; nothing here is a parser. These two read the
# BLOCK form only, where a mapping's keys sit on their own lines; the flow form,
# `with: { toolchain: nightly }`, is one line's value and is read by flow_pairs.
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
# Matched against a lowercased name, so `Makefile`, `makefile` and `GNUmakefile`
# -- all three of which make itself reads -- are one entry rather than three.
HASH_NAMED = {"makefile", "gnumakefile", "justfile"}
# The suffixes whose contents are YAML, and so the only files whose lines fold
# by YAML's rules. Every other file the acquisition clause reads has its own
# syntax, in which indentation carries no meaning at all. These are also the
# only two Actions itself reads a workflow from, so the same set decides which
# files are workflows at all -- one rule rather than two spellings of it.
YAML_SUFFIXES = {".yml", ".yaml"}
# Not read by the acquisition clause. `.md` and `.txt` are prose: a design
# note describing a rejected engine is not an acquisition. `.lock` is
# GENERATED -- its contents restate the manifests the dependency clause already
# reads from the resolved graph, so a URL there is a consequence of a
# dependency that clause has already judged, and reading it would report the
# same engine twice under a weaker rule.
NOT_READ = {".md", ".txt", ".lock"}

# The file name a LOCAL ACTION is defined in, and the directories a walk for one
# never enters. A workflow step written `uses: ./path` runs the action defined
# there, on the same runner, on the same release path -- and a composite action
# may run `rustup default nightly` or download an engine exactly as a workflow
# step may. Reading only `.github/workflows/` left every one of them unread.
# The path is not fixed: `uses: ./tools/setup` is as valid as
# `./.github/actions/setup`, so the walk is over the tree rather than over one
# directory, and it keys on the name GitHub requires an action definition to
# carry.
ACTION_NAMES = ("action.yml", "action.yaml")
# Pruned at EVERY depth, not only at the root. Cargo writes a target directory
# per workspace and a nested one is still its output; a source directory that
# happens to be called `target` is pruned with them, which costs a file this
# clause would have read and is the safer of the two errors -- the other is
# reporting every vendored crate that ships an action definition.
NOT_WALKED = ("target", ".git", "node_modules")


class CheckError(Exception):
    """The check cannot reach a verdict. An unrun check is not a pass."""


# The one Rust scanner, shared with the crate policy check. A second, weaker
# copy lived here: it had no char-literal rule and no escape handling, so a
# quote inside `'"'` or behind a backslash desynchronised it -- rejecting
# compliant crate roots AND blanking away a genuine `feature(...)` gate.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from casefs import (  # noqa: E402
    folded_in, is_rust_source, named_dirs, named_files, suffix_of,
)
from rustlex import strip_non_code  # noqa: E402


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

    For `style` "hash" only, plus None to leave the line as it is. A hash
    comment ends at the newline, so a line is the whole of its context and this
    is exact.

    Rust is NOT handled here and must not be. A Rust comment spans lines and a
    line is not its context: reading one at a time ended a block comment where
    it opened and handed the interior back as code, which refused compliant
    crate roots. `code_lines` sends Rust to the shared scanner, which carries
    that state -- and this function's Rust branches were deleted rather than
    left as a second reading of Rust that nothing calls and someone could.
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
        if ch == '"' or ch == "'":
            quote = ch
        elif line.startswith("#", i) and (i == 0 or line[i - 1] in " \t"):
            # `#` opens a comment at a word start and nowhere else, in sh and in
            # YAML alike. Cutting at every unquoted one truncated a URL with a
            # fragment -- `https://x/a#b/chromium.zip` -- and a `$#` test on a
            # line that went on to select nightly, so the verdict turned on
            # whether the author had quoted the argument.
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
    if folded_in(path.name, HASH_NAMED):
        return "hash"
    return COMMENT_STYLE.get(suffix_of(path))


def code_lines(path):
    """(line number, line with its comment removed) for every line of `path`.

    Rust is scanned WHOLE, not line by line. A block comment spans lines, so a
    per-line reading ends the comment at the line that opened it and hands the
    interior back as code -- and a compliant file was then reported for words
    written inside a comment explaining why they are forbidden. The shared
    scanner carries that state and keeps every offset, so splitting after it
    yields the same lines with the comments gone.

    String literals are KEPT. The acquisition clause reads the engine's name
    from inside one, so blanking them would blank what it reads; the same is
    true of `RUSTC_BOOTSTRAP`, which appears as an argument to `set_var`.

    Every other style comments to end of line, where per-line is exactly right.
    """
    text = read_text(path)
    if text is None:
        return []
    style = comment_style(path)
    if style == "rust":
        return list(enumerate(strip_non_code(text, keep_literals=True).splitlines(), 1))
    return [
        (number, strip_comment(line, style))
        for number, line in enumerate(text.splitlines(), 1)
    ]


def load_toml(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def is_yaml(path):
    """Whether this file's contents are YAML, by the one rule both readings use.

    The glob that collected workflows was `*.y*ml` and the test that decided
    whether to fold by YAML's rules was a suffix set. Two spellings of one
    question, and they disagreed: `deploy.yeml` was collected as a workflow and
    read as YAML by the nightly clause, while the acquisition clause read it as
    plain text -- so a wrapped acquisition in it passed where the identical file
    named `.yml` failed. Actions reads a workflow from `.yml` and `.yaml` and
    nothing else, so that set is the answer to both questions.
    """
    return suffix_of(path) in YAML_SUFFIXES


def workflow_files(root):
    directory = root / ".github" / "workflows"
    if not directory.is_dir():
        return []
    return sorted(p for p in directory.iterdir() if p.is_file() and is_yaml(p))


def action_files(root):
    """Every local action definition in the tree.

    A workflow step written `uses: ./path` runs the action defined at that path,
    on the same runner and the same release path as the workflow that calls it.
    A composite action's `run:` steps are shell exactly as a workflow's are, so
    one may install a toolchain or fetch an engine on terms no clause here was
    reading: the two clauses read `.github/workflows/` and nothing else, and a
    `rustup default nightly` one directory across was invisible.

    The path is the author's -- `uses: ./tools/setup` is as valid as
    `./.github/actions/setup` -- so this walks the tree and keys on the file
    name GitHub requires an action definition to carry, folded like every other
    name here.
    """
    found = []
    for directory, subdirectories, names in os.walk(root):
        subdirectories[:] = [
            name for name in subdirectories if not folded_in(name, NOT_WALKED)
        ]
        for name in names:
            if folded_in(name, ACTION_NAMES):
                found.append(Path(directory) / name)
    return sorted(found)


def workflow_like_files(root):
    """Every YAML file the workflow clauses read: the workflows, and the local
    actions a workflow step can hand the runner."""
    files = workflow_files(root)
    for path in action_files(root):
        if path not in files:
            files.append(path)
    return files


def files_under(directory, skip=()):
    """Every regular file under `directory`, skipping the named top-level dirs.

    The skip is matched with case folded, like every other name here. Cargo
    writes its build output to `target/`, and on the case-insensitive
    filesystems the release runners use, `TARGET/` is that same directory --
    which a raw comparison did not skip. Every vendored crate source Cargo has
    unpacked there was then scanned for feature attributes, and a registry holds
    plenty that need nightly.

    Only the nightly caller passes a skip. The acquisition clause descends
    `src/` and `build/` rather than the crate directory, so Cargo's output is
    outside what it reads and needs no skipping.
    """
    found = []
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        parts = path.relative_to(directory).parts
        if folded_in(parts[0], skip):
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
    except tomllib.TOMLDecodeError as error:
        parsed = None
        # A `.toml` file that does not parse is a misread file, not a legacy
        # one. Falling silently through to the one-line reading made an
        # unparseable file read as the literal "[toolchain]", match no channel,
        # and be counted as read and clean -- which is the opposite of the rule
        # this function applies eight lines below to a non-table [toolchain].
        # The extensionless file has no such guarantee: its legacy form is not
        # TOML and failing to parse is the normal case there.
        # This path is built from a literal name a few lines below, so its
        # suffix cannot vary and no case can distinguish this from asking the
        # raw one. It goes through the shared reader anyway, so that the next
        # suffix test in this file is written the right way by default.
        if suffix_of(path) == ".toml":
            problems.append(f"{where}: {error}")
            return
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
    # Guarded like every other table two lines below: a top-level `target = "x"`
    # is valid TOML of the wrong type, and walking it raised.
    declared_targets = config.get("target")
    if isinstance(declared_targets, dict):
        tables += [(f"target.{name}", table) for name, table in declared_targets.items()]
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


# A block scalar header -- `key: |`, `key: >-`, `key: |+2` -- whose body is the
# more-indented lines beneath it.
# The header tail is an optional indentation indicator and an optional chomping
# indicator, IN EITHER ORDER -- YAML 1.2 allows `>2-` exactly as it allows `>-2`,
# and PyYAML folds both identically. Accepting only one order left `run: >2-`
# unfolded, which is the same evasion the folding exists to close.
BLOCK_SCALAR = re.compile(
    r"""^(\s*)(?:-\s+)?["']?([A-Za-z_][\w.-]*)["']?\s*:\s*([|>])"""
    r"""(?:[0-9][-+]?|[-+][0-9]?)?\s*$"""
)
# Keys whose block body is a SCRIPT: each line is its own command. That is true
# of a LITERAL body, `|`, and only of a literal body -- YAML folds a `>` body
# into one line itself, before the shell ever sees it, so declining to fold one
# declines to read what will actually run. Keying the carve-out on the key alone
# let `run: >` put the release path on nightly and pass, in the very spelling
# this repository's own build workflow uses to wrap a long command.
SCRIPT_KEYS = {"run", "script", "cmd", "shell", "entrypoint", "args"}


def logical_lines(lines, yaml):
    """`lines` with each continuation folded into the line it opens on.

    A workflow's logical line is not its physical line, and every reading below
    needs its two halves together: a `-Z` beside the word `cargo`, a channel
    beside the key it sits under. Wrapping is ordinary formatting, so a check
    that reads physical lines refuses the unwrapped form and passes the wrapped
    one -- which is what a release path put entirely on nightly looked like.

    Two kinds of continuation, each folded on its own terms:

    - A shell line ending in `\\` continues onto the next. That is the shell's
      rule and it holds wherever it appears.
    - A block scalar's body belongs to its key. A folded or literal scalar is
      ONE value, so it is joined with spaces -- except under a script key, where
      the body is a sequence of commands and joining them would put the words of
      one beside the words of another. Those keep their lines and get the
      backslash rule instead.
    - A flow collection continues until its bracket closes, whatever line that
      falls on. `with: { toolchain: nightly` and a `}` on the next line is one
      mapping to YAML and was two lines to every reading here, so the pair the
      first line opens was never completed and the channel was never seen.
    - A value need not sit on its key's line at all. A more-indented line that
      is neither a key nor a sequence item can only continue the value above
      it: the second half of a plain or quoted scalar (`run: rustup default`
      then `nightly`), or the whole value of a key written bare (`toolchain:`
      then `nightly`). Both are ordinary YAML, both name the channel, and both
      passed. Script bodies are exempt -- they are shell, not YAML, and joining
      two commands there is what the carve-out above exists to prevent.

    Each result carries the number of the physical line it opens on, so a report
    still names where a reader would look.

    Three of the four folds are YAML's and are skipped when `yaml` is false, and
    every caller states which it wants: a default would let the next reading of
    a file take YAML's rules without anyone deciding that it should, which is
    how they reached a Rust source file in the first place. The
    acquisition clause reads this file's Rust, TOML and shell as well as its
    workflows, and in none of those does indentation mean what it means in YAML:
    a deeper line is a function body or an array element, not the rest of the
    value above it. Folding them together reported two unrelated string literals
    in one Rust array -- one naming an engine, one naming an archive -- as a line
    that fetches an engine. The backslash rule is not YAML's and applies to all
    of them: a trailing backslash continues a line in the shell and inside a
    string literal alike.
    """
    numbered = list(lines)
    if not yaml:
        return fold_backslashes(numbered)
    out = []
    script_body = set()  # line numbers that are shell, and so not YAML values
    i = 0
    while i < len(numbered):
        number, text = numbered[i]
        header = BLOCK_SCALAR.match(text)
        if header:
            # The body boundary is the KEY's column, not the dash's. In a
            # sequence item -- `      - run: |` -- the dash sits at 6 and the
            # key at 8, and the step's sibling keys (`with:`, `env:`, `uses:`)
            # sit at 8 too. Measuring from the dash made every one of them
            # deeper than the header, so the body swallowed the rest of the
            # step and the walk never saw them: a `- run: >` beside a
            # `toolchain: nightly` passed, and whether a step carried a `name:`
            # line decided the verdict.
            key, style = header.group(2), header.group(3)
            indent = header.start(2)
            body, j = [], i + 1
            while j < len(numbered):
                following = numbered[j][1]
                if following.strip() and len(following) - len(following.lstrip()) <= indent:
                    break
                body.append(numbered[j])
                j += 1
            if key.lower() in SCRIPT_KEYS and style == "|":
                out.append((number, text))
                folded_body = fold_backslashes(body)
                script_body.update(each[0] for each in folded_body)
                out.extend(folded_body)
            else:
                folded = " ".join(line.strip() for _, line in body if line.strip())
                out.append((number, f"{text.rstrip()} {folded}".rstrip()))
            i = j
            continue
        out.append((number, text))
        i += 1
    return fold_flow(fold_continuations(fold_backslashes(out), script_body))


def fold_backslashes(lines):
    """`lines` with each shell continuation joined onto the line it opens on."""
    out = []
    pending = None
    for number, text in lines:
        stripped = text.rstrip()
        if pending is not None:
            number, text = pending[0], f"{pending[1]} {text.strip()}"
            stripped = text.rstrip()
            pending = None
        if stripped.endswith("\\"):
            pending = (number, stripped[:-1].rstrip())
            continue
        out.append((number, text))
    if pending is not None:
        out.append(pending)
    return out


def flow_scan(text):
    """The `key: value` pairs of every flow collection in one line, and the
    bracket depth left open at its end.

    YAML writes a mapping two ways and Actions reads both: `with:` with an
    indented `toolchain:` beneath it, or `with: { toolchain: nightly }` all on
    one line. The key chain in workflow_nightly_lines walks the first, and
    cannot reach into the second -- the whole mapping is one line's value, and
    the key it hangs under is `with`, which no toolchain key list contains. So
    the block form was caught and the flow form, the same two tokens with the
    same meaning to the workflow parser, was not.

    Not a YAML parser either. It tracks quoting and bracket depth, takes a key
    as the text before a colon that ends a token, and takes the value as the
    text up to the next comma or closing bracket outside any deeper bracket --
    which keeps a flow sequence with the key that introduces it, so
    `rust: [stable, nightly]` is one pair and not two valueless items. Pairs at
    depth zero are left out on purpose: those are the block form, which the
    caller has already judged.

    Returns the depth still open at the end of the text alongside the pairs.
    `fold_flow` needs exactly that number and nothing else, and taking it from
    this scan rather than a second one keeps the bracket and quote rules in a
    single place -- the third time on this branch that two readers of one thing
    disagreed and the weaker one decided a verdict.
    """
    pairs = []
    stack = []
    depth = 0
    quote = None
    start = None  # where the element being read begins
    colon = None  # where its key ends, once a separating colon is seen
    # Whether a value may begin here, which at depth zero is the only place a
    # bracket opens a flow collection. `run: echo {` is a plain scalar with a
    # brace in it -- to YAML and to the shell alike -- and reading it as an
    # opened mapping would fold every following line into it and lose them.
    may_open = True
    i = 0
    while i < len(text):
        ch = text[i]
        if quote is not None:
            # Only a double-quoted scalar has escapes; inside a single-quoted
            # one a backslash is an ordinary character.
            if ch == "\\" and quote == '"':
                i += 2
                continue
            if ch == quote:
                quote = None
            i += 1
        elif ch in "\"'":
            quote = ch
            may_open = False
            i += 1
        elif ch in "{[" and (depth or may_open):
            stack.append((start, colon))
            depth += 1
            start, colon = i + 1, None
            i += 1
        elif ch in "}]" and depth:
            if colon is not None:
                pairs.append((text[start:colon], text[colon + 1:i]))
            depth -= 1
            start, colon = stack.pop()
            i += 1
        elif ch == "," and depth:
            if colon is not None:
                pairs.append((text[start:colon], text[colon + 1:i]))
            start, colon = i + 1, None
            i += 1
        elif ch == ":" and depth and colon is None and text[i + 1:i + 2] == " ":
            # In flow context YAML separates a key from its value with a colon
            # and a SPACE, and only that: a tab there is a scanner error, and a
            # colon with anything else after it is part of a plain scalar --
            # `version:beta` is one argument token, not the beta channel under
            # the key `version`. The empty-value spellings, `{toolchain:}` and
            # `{toolchain:, os: x}`, need no branch of their own: an absent
            # value is never a channel, and the pair that follows is read the
            # same whether or not this colon was taken.
            # `colon is None` fixes the key at the FIRST separating colon,
            # which is YAML's rule. No well-formed flow element carries a
            # second one -- a plain scalar may not contain `: `, a quoted one
            # is stepped over, and a nested collection gets its own depth --
            # so the suite cannot tell that guard from its absence. It is here
            # because it states the rule, not because a case pins it. Clearing
            # `may_open` when a quoted scalar opens is the same: after a quoted
            # value nothing may open, and no valid YAML puts a bracket there --
            # what it buys is on shell text folded out of a script body, which
            # is not YAML and where a stray brace would otherwise read as a
            # collection.
            colon = i
            i += 1
        else:
            if not depth:
                # Where a value may begin, outside any collection: after a key's
                # colon, after the dash that heads a block sequence item, and at
                # the head of the line -- with the spaces between them. A plain
                # scalar's first character ends it, and a quoted scalar IS the
                # value, so nothing after it opens anything either.
                if ch == ":" and text[i + 1:i + 2] in (" ", "\t", ""):
                    may_open = True
                elif ch in " \t":
                    pass
                elif ch == "-" and may_open and text[i + 1:i + 2] in (" ", "\t"):
                    pass
                else:
                    may_open = False
            i += 1
    if colon is not None:
        # A collection still open at the end of the text. That is malformed
        # YAML and the workflow would not load -- but a reading that goes quiet
        # on a malformed file is the wrong direction for a check whose whole
        # job is to refuse something, so the pair it was in the middle of is
        # reported rather than dropped. `colon` can only be set inside a
        # collection, so a well-formed line never reaches this.
        pairs.append((text[start:colon], text[colon + 1:]))
    return [(key.strip().strip("\"'"), value) for key, value in pairs], depth


def flow_pairs(text):
    """The pairs of `flow_scan`, for a caller that does not need the depth."""
    return flow_scan(text)[0]


def fold_continuations(lines, script_body):
    """`lines` with each value continuation joined onto the line it begins on.

    YAML lets a value sit below its key -- `toolchain:` with `nightly` on the
    next line -- and lets a plain or quoted scalar run across lines the same
    way. Both are one value, and both looked like a line naming no key and
    holding no channel: the walk in workflow_nightly_lines reads a key line or a
    sequence item, and a continuation is neither, so it was never read at all.

    A line continues the one above when it is deeper than it and is neither. The
    line numbers in `script_body` are exempt: a block scalar's script body is
    shell, where a deeper line is an indented command rather than the rest of a
    value, and joining two commands is what the SCRIPT_KEYS carve-out exists to
    prevent. Only the line being folded is tested against that set; a body's
    lines are all deeper than their header and the line ending a body is never
    deeper than the body, so the line a body line could be folded into is never
    outside it.

    In valid YAML the depth test is implied by the other two -- a line that is
    neither a key nor an item can only be a continuation, and a continuation is
    always deeper. It is what keeps a file that is NOT valid from collapsing
    into one line, where two lines' words become one line's verdict.

    Blank lines are dropped rather than carried. A value may sit a blank line
    below its key, and a blank line has an indent of zero however deep the key
    above it is -- so the value measured itself against nothing, folded onto the
    blank, and the key was read as carrying none. Nothing downstream reads a
    blank line: every reading here skips it.
    """
    out = []
    for number, text in lines:
        if not text.strip():
            continue
        if (
            out
            and number not in script_body
            and indent_of(text) > indent_of(out[-1][1])
            and not YAML_KEY.match(text)
            and not YAML_ITEM.match(text)
        ):
            out[-1] = (out[-1][0], f"{out[-1][1].rstrip()} {text.strip()}")
            continue
        out.append((number, text))
    return out


def indent_of(text):
    return len(text) - len(text.lstrip())


def fold_flow(lines):
    """`lines` with a flow collection that spans lines joined onto the line it
    opens on. The third continuation, on the same terms as the other two: the
    line reported is the one the collection opens on."""
    out = []
    pending = None
    for number, text in lines:
        if pending is not None:
            number, text = pending[0], f"{pending[1]} {text.strip()}"
            pending = None
        if flow_scan(text)[1] > 0:
            pending = (number, text.rstrip())
            continue
        out.append((number, text))
    if pending is not None:
        out.append(pending)
    return out


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
        if any(
            key in TOOLCHAIN_KEYS and UNSTABLE_VALUE.search(value)
            for key, value in flow_pairs(line)
        ):
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
    for number, line in workflow_nightly_lines(logical_lines(code_lines(path), True)):
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
    # The window is built from the WHOLE file, not from `lines`. Both a block
    # comment and a raw string span lines, and this clause must read neither:
    # scanning per line ended each at the line that opened it and handed the
    # interior back as code, so an attribute quoted inside a multi-line comment
    # was reported as one carried.
    text = read_text(path)
    scanned = strip_non_code(text).splitlines() if text is not None else []
    joined, offsets = [], []
    for number, line in enumerate(scanned, 1):
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
        # Each of these is found by name, and the release runners find a name
        # with case folded. `RUST-TOOLCHAIN.TOML` selects the channel for the
        # build that ships and was read by nothing here; so was a `[unstable]`
        # table in `.CARGO/CONFIG.TOML`. `Cargo.toml` is not folded: Cargo
        # itself requires that spelling, so no other one names a package.
        for name in ("rust-toolchain.toml", "rust-toolchain"):
            for path in named_files(directory, name):
                check_toolchain_file(root, path, problems)
                read += 1
        for cargo_dir in named_dirs(directory, ".cargo"):
            for name in ("config.toml", "config"):
                for path in named_files(cargo_dir, name):
                    check_cargo_config(root, path, problems)
                    read += 1
        manifest = directory / "Cargo.toml"
        if manifest.is_file():
            check_manifest_features(root, manifest, problems)
            read += 1
    for path in workflow_like_files(root):
        check_workflow_nightly(root, path, problems)
        read += 1
    for directory in member_dirs:
        for path in files_under(Path(directory), skip={"target"}):
            if is_rust_source(path):
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
    # The same folding the nightly clause reads through, for the same reason and
    # over the same files: this clause needs an engine's name and an acquisition
    # marker on one line, so a wrapped command splits the two halves and the
    # verdict turns on formatting. Fixing one of the two readings of one file
    # was the whole of that fix, and this is the other.
    # Only a workflow folds by YAML's rules, though. This clause reads Rust,
    # TOML and shell too, and in those a deeper line is a function body or an
    # array element rather than the rest of the value above it -- folding them
    # made one finding out of two unrelated literals.
    for number, line in logical_lines(code_lines(path), is_yaml(path)):
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
        if path.is_file() and suffix_of(path) not in NOT_READ and path not in files:
            files.append(path)

    for package in member_packages(metadata):
        directory = Path(package["manifest_path"]).parent
        add(directory / "Cargo.toml")
        # `build.rs` and the runtime directories are conventions Cargo resolves
        # on the release runner's own filesystem, where `BUILD.RS` is the build
        # script and `SRC` is the source directory. Naming them in lower case
        # left a build script that fetches an engine, and every file beneath a
        # source directory, unread.
        for path in named_files(directory, "build.rs"):
            add(path)
        for target in package.get("targets", []):
            if "custom-build" in target.get("kind", []):
                add(Path(target["src_path"]))
        for name in RUNTIME_PATH:
            for found in named_dirs(directory, name):
                for path in files_under(found):
                    add(path)
    for path in workflow_like_files(Path(root)):
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
