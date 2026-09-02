#!/usr/bin/env python3
"""Tests for the engine prohibition check. The check is CI's authority to fail
a build over a dependency, a toolchain line or a fetch, so its own behaviour is
checked rather than assumed -- above all that the one carve-out FR-044 allows
passes exactly the first-run bootstrap path and nothing wider, and that a
vendored engine archive fails however it is spelled.

Run: python3 scripts/checks/test_check_engine_prohibition.py
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import check_engine_prohibition as engine

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check_engine_prohibition.py"
REPO = HERE.parent.parent

PASSED = FAILED = 0


def check(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

def metadata(root, members, edges=(), declared=(), extra=()):
    """A `cargo metadata` shape: `members` are workspace crates under `root`,
    `edges` are (from, to) resolved dependencies, `declared` are (from, to)
    dependencies a manifest names whether or not the resolve reached them, and
    `extra` are registry packages present in the graph."""
    names = list(dict.fromkeys(list(members) + [b for _, b in edges] + list(extra)))

    def identity(name):
        if name in members:
            return f"path+file://{root}/crates/{name}#0.0.0"
        return f"registry+https://github.com/rust-lang/crates.io-index#{name}@1.0.0"

    packages = []
    for name in names:
        dependencies = [{"name": b, "optional": False} for a, b in edges if a == name]
        dependencies += [{"name": b, "optional": True} for a, b in declared if a == name]
        manifest = Path(root) / "crates" / name / "Cargo.toml"
        packages.append({
            "id": identity(name),
            "name": name,
            "manifest_path": str(manifest),
            "dependencies": dependencies,
            "targets": [],
        })
    nodes = [
        {
            "id": identity(name),
            "deps": [{"name": b, "pkg": identity(b)} for a, b in edges if a == name],
        }
        for name in names
    ]
    return {
        "packages": packages,
        "workspace_members": [identity(name) for name in members],
        "resolve": {"nodes": nodes},
    }


def write(root, relative, text):
    path = Path(root) / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path


def workspace(root, members=("evreos-shell",)):
    """A minimal stable workspace with the given members, each with a crate root."""
    write(root, "Cargo.toml", "[workspace]\nmembers = [%s]\n" % ", ".join(
        f'"crates/{m}"' for m in members))
    for member in members:
        write(root, f"crates/{member}/Cargo.toml", f'[package]\nname = "{member}"\nversion = "0.0.0"\n')
        write(root, f"crates/{member}/src/main.rs", "#![forbid(unsafe_code)]\nfn main() {}\n")
    return metadata(root, list(members))


def denylist(root, names=("cef", "chromiumoxide", "electron")):
    return write(root, "prohibited-engines.txt", "# test list\n" + "\n".join(names) + "\n")


def run_check(*args):
    return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True)


# --- the deny-list ------------------------------------------------------------

names, problems = engine.read_denylist(engine.DENYLIST)
check("the committed deny-list reads cleanly", problems == [])
for seed in ("cef", "cef-dll-sys", "cef-sys", "libcef-sys", "download-cef",
             "chromium", "chromiumoxide", "headless-chrome", "electron", "electron-sys"):
    check(f"the committed deny-list is seeded with {seed}", seed in names)
for permitted in ("wry", "webview2-com", "webkit2gtk", "tauri", "servo"):
    check(f"the committed deny-list does not name {permitted}", permitted not in names)

with tempfile.TemporaryDirectory() as tmp:
    path = write(tmp, "list.txt", "# comment\n\nCef  # trailing\nHeadless_Chrome\n")
    names, problems = engine.read_denylist(path)
    check("comments and blank lines are ignored, names normalised", names == ["cef", "headless-chrome"] and problems == [])

    path = write(tmp, "twice.txt", "cef\nnot a crate\ncef\n")
    names, problems = engine.read_denylist(path)
    check("a duplicated entry is a problem", any("listed twice" in p for p in problems))
    check("a malformed entry is a problem", any("not a crate name" in p for p in problems))
    check("...and is not added to the list", names == ["cef", "cef"])

    # The one narrowing of the README's convention on committed lists, stated
    # in the docstring: an empty deny-list is the weakest state, not the
    # strictest, so it is a problem rather than a pass.
    path = write(tmp, "empty.txt", "# nothing here\n")
    names, problems = engine.read_denylist(path)
    check("an empty deny-list is a problem, not a pass", names == [] and problems)
    check("...and the problem says why", problems and "seeded" in problems[0])

    try:
        engine.read_denylist(Path(tmp) / "absent.txt")
        check("a missing deny-list is a CheckError", False)
    except engine.CheckError:
        check("a missing deny-list is a CheckError", True)

# A workspace with no members is a check over nothing, which is not a pass.
# Nothing pinned that guard, so removing it turned an empty metadata set into
# "0 crates, 0 packages ... passed".
try:
    engine.check_repository(
        REPO, HERE / "prohibited-engines.txt",
        metadata={"workspace_members": [], "packages": []}
    )
    check("a workspace with no members is a CheckError", False)
except engine.CheckError as error:
    check("a workspace with no members is a CheckError",
          "not a pass" in str(error))

# --- DEPENDENCY ---------------------------------------------------------------

DENY = ["cef", "cef-dll-sys", "chromiumoxide", "headless-chrome", "electron"]

m = metadata("/w", ["evreos-shell", "evreos-engine"], edges=[("evreos-shell", "evreos-engine")])
check("a clean graph has no prohibited dependency", engine.prohibited_dependencies(m, DENY) == [])

m = metadata("/w", ["evreos-shell"], edges=[("evreos-shell", "cef")])
problems = engine.prohibited_dependencies(m, DENY)
check("a direct dependency on a prohibited crate fails", len(problems) == 1)
check("...and is reported as direct", problems and "directly" in problems[0] and "evreos-shell -> cef" in problems[0])

m = metadata("/w", ["evreos-shell"], edges=[("evreos-shell", "helper"), ("helper", "cef-dll-sys")])
problems = engine.prohibited_dependencies(m, DENY)
check("a transitive dependency on a prohibited crate fails", len(problems) == 1)
check("...and the chain that reaches it is named",
      problems and "evreos-shell -> helper -> cef-dll-sys" in problems[0] and "through helper" in problems[0])

m = metadata("/w", ["evreos-shell"], edges=[("evreos-shell", "Headless_Chrome")])
check("case and underscores do not hide a prohibited name", engine.prohibited_dependencies(m, DENY) != [])

m = metadata("/w", ["evreos-shell"], edges=[("evreos-shell", "cefalo")])
check("a name that merely contains a prohibited one is not prohibited", engine.prohibited_dependencies(m, DENY) == [])

m = metadata("/w", ["evreos-shell"], declared=[("evreos-shell", "electron")])
problems = engine.prohibited_dependencies(m, DENY)
check("a declared dependency the resolve did not reach fails on its name", len(problems) == 1 and "does not reach" in problems[0])

m = metadata("/w", ["evreos-shell", "evreos-probe"],
             edges=[("evreos-shell", "evreos-engine"), ("evreos-probe", "chromiumoxide")])
problems = engine.prohibited_dependencies(m, DENY)
check("a dev-only member is not excused", len(problems) == 1 and problems[0].startswith("evreos-probe"))

m = metadata("/w", ["evreos-shell"], edges=[("evreos-shell", "a"), ("a", "b"), ("b", "a")])
check("a cycle in the graph terminates", engine.prohibited_dependencies(m, DENY) == [])

# --- NIGHTLY ------------------------------------------------------------------

def nightly_problems(build):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        workspace(root)
        build(root)
        problems, _ = engine.scan_nightly(root, [root / "crates" / "evreos-shell"])
        return problems


check("a stable workspace passes the nightly clause", nightly_problems(lambda r: None) == [])

check("rust-toolchain.toml on nightly fails",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = "nightly"\n')))
check("a dated nightly fails",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = "nightly-2026-08-01"\n')))
check("beta is not stable",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = "beta"\n')))
# The pattern allows leading whitespace before the channel name and the error
# message strips it, so the two agree; nothing pinned the allowance. Without it
# one space is all it takes to select nightly unseen -- and rustup reads the
# value as TOML, which does not trim inside the quotes, so this is a spelling
# that behaves differently from how it reads.
check("a channel padded with a space is still nightly",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = " nightly"\n')))
check("a custom toolchain path fails",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\npath = "/opt/rust"\n')))
check("a pinned stable version passes",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = "1.85.0"\n')) == [])
check("stable with a target triple passes",
      nightly_problems(lambda r: write(r, "rust-toolchain.toml", '[toolchain]\nchannel = "stable-x86_64-unknown-linux-gnu"\n')) == [])
check("the legacy rust-toolchain file on nightly fails",
      nightly_problems(lambda r: write(r, "rust-toolchain", "nightly\n")))
check("the legacy rust-toolchain file on stable passes",
      nightly_problems(lambda r: write(r, "rust-toolchain", "stable\n")) == [])
check("a member's own toolchain file is read too",
      nightly_problems(lambda r: write(r, "crates/evreos-shell/rust-toolchain.toml", '[toolchain]\nchannel = "nightly"\n')))

check(".cargo/config.toml with [unstable] fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", "[unstable]\nbuild-std = true\n")))
check("a -Z rustflag fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[build]\nrustflags = ["-Zbuild-std"]\n')))
check("a -Z rustflag on a target table fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[target.x86_64-pc-windows-msvc]\nrustflags = "-C target-feature=+crt-static -Zshare-generics"\n')))
check("RUSTC_BOOTSTRAP in [env] fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[env]\nRUSTC_BOOTSTRAP = "1"\n')))
check("RUSTUP_TOOLCHAIN in [env] pointing at nightly fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[env]\nRUSTUP_TOOLCHAIN = "nightly"\n')))
check("RUSTUP_TOOLCHAIN in [env] in table form fails too",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[env]\nRUSTUP_TOOLCHAIN = { value = "beta", force = true }\n')))
check("RUSTUP_TOOLCHAIN in [env] pointing at stable passes",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[env]\nRUSTUP_TOOLCHAIN = "stable"\n')) == [])
check("a nightly rustc named in [build] fails",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[build]\nrustc = "/opt/nightly/bin/rustc"\n')))
check("an ordinary .cargo/config.toml passes",
      nightly_problems(lambda r: write(r, ".cargo/config.toml", '[build]\njobs = 2\nrustflags = ["-C", "target-cpu=native"]\n')) == [])
check("the legacy .cargo/config is read too",
      nightly_problems(lambda r: write(r, ".cargo/config", "[unstable]\nmtime-on-use = true\n")))

check("cargo-features in a manifest fails",
      nightly_problems(lambda r: write(r, "crates/evreos-shell/Cargo.toml",
                                       'cargo-features = ["profile-rustflags"]\n[package]\nname = "evreos-shell"\nversion = "0.0.0"\n')))
check("cargo-features in the root manifest fails",
      nightly_problems(lambda r: write(r, "Cargo.toml", 'cargo-features = ["codegen-backend"]\n[workspace]\nmembers = ["crates/evreos-shell"]\n')))


def rs(root, text, name="src/lib.rs"):
    return write(root, f"crates/evreos-shell/{name}", text)


check("#![feature(...)] fails", nightly_problems(lambda r: rs(r, "#![feature(specialization)]\n")))
check("a feature attribute behind cfg_attr fails",
      nightly_problems(lambda r: rs(r, "#![cfg_attr(docsrs, feature(doc_cfg))]\n")))
check("a feature attribute in a test file fails, since the toolchain is one per workspace",
      nightly_problems(lambda r: rs(r, "#![feature(test)]\n", "tests/bench.rs")))
check("a feature attribute in a build script fails",
      nightly_problems(lambda r: rs(r, "#![feature(let_chains)]\nfn main() {}\n", "build.rs")))
check("RUSTC_BOOTSTRAP in source fails",
      nightly_problems(lambda r: rs(r, 'fn main() { std::env::set_var("RUSTC_BOOTSTRAP", "1"); }\n')))
check("#![cfg(feature = ...)] is a Cargo feature and passes",
      nightly_problems(lambda r: rs(r, '#![cfg(feature = "fixture-brand")]\n')) == [])
check("#[cfg(feature = \"nightly\")] is a Cargo feature and passes",
      nightly_problems(lambda r: rs(r, '#[cfg(feature = "nightly")]\nfn f() {}\n')) == [])
# `feature` is a crate-level attribute and exists only in the inner `#![...]`
# form, so the pattern anchors on `#!`. An OUTER attribute is an attribute
# macro's argument list, where `feature(...)` is that macro's own vocabulary
# and means nothing about the toolchain. Dropping the anchor would report
# these, and the pair below is what says so: same argument list, one form
# reported and the other not.
check("feature(...) as an outer attribute macro's argument passes",
      nightly_problems(lambda r: rs(r, "#[wrapper(feature(A))]\nstruct T;\n")) == [])
check("the same argument list in the inner form fails",
      nightly_problems(lambda r: rs(r, "#![wrapper(feature(A))]\nstruct T;\n")))
check("#![forbid(unsafe_code)] passes", nightly_problems(lambda r: rs(r, "#![forbid(unsafe_code)]\n")) == [])
check("a commented-out feature attribute passes",
      nightly_problems(lambda r: rs(r, "// #![feature(specialization)] was tried and rejected\n")) == [])
check("a doc comment quoting the attribute passes",
      nightly_problems(lambda r: rs(r, "//! Never `#![feature(...)]`: Principle III.\n")) == [])
check("a feature attribute after a URL in a string is still seen",
      nightly_problems(lambda r: rs(r, 'const U: &str = "https://x/"; #![feature(x)]\n')))


def wf(root, text):
    return write(root, ".github/workflows/build.yml", text)


# Installing a non-stable toolchain, under either spelling rustup accepts.
check("a workflow installing nightly fails",
      nightly_problems(lambda r: wf(r, "run: rustup toolchain install nightly --profile minimal\n")))
check("rustup install, the alias of toolchain install, fails",
      nightly_problems(lambda r: wf(r, "run: rustup install nightly\n")))
check("flags between the verb and the channel do not hide it",
      nightly_problems(lambda r: wf(r, "run: rustup toolchain install --profile minimal --no-self-update nightly\n")))
check("rustup update nightly fails",
      nightly_problems(lambda r: wf(r, "run: rustup update beta\n")))
# Defaulting to one.
check("a workflow defaulting to nightly fails",
      nightly_problems(lambda r: wf(r, "run: rustup default nightly\n")))
check("rustup override set nightly fails",
      nightly_problems(lambda r: wf(r, "run: rustup override set nightly-2026-08-01\n")))
# Invoking one.
check("cargo +nightly fails", nightly_problems(lambda r: wf(r, "run: cargo +nightly fmt --check\n")))
check("cargo +beta fails", nightly_problems(lambda r: wf(r, "run: cargo +beta test\n")))
check("rustup run nightly fails",
      nightly_problems(lambda r: wf(r, "run: rustup run nightly cargo build --release\n")))
# Naming one as a value.
check("a toolchain: nightly input fails",
      nightly_problems(lambda r: wf(r, "with:\n  toolchain: nightly\n")))
check("a quoted toolchain input fails",
      nightly_problems(lambda r: wf(r, 'with:\n  toolchain: "beta"\n')))
check("a toolchain action pinned to nightly fails",
      nightly_problems(lambda r: wf(r, "uses: dtolnay/rust-toolchain@nightly\n")))
check("RUSTUP_TOOLCHAIN in a workflow's env fails",
      nightly_problems(lambda r: wf(r, "env:\n  RUSTUP_TOOLCHAIN: nightly\n")))
check("RUSTUP_TOOLCHAIN set inline on a command fails",
      nightly_problems(lambda r: wf(r, "run: RUSTUP_TOOLCHAIN=nightly cargo build\n")))
# A matrix, in each form YAML gives it.
check("an inline matrix that includes nightly fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  rust: [stable, nightly]\n")))
check("a block-list matrix that includes nightly fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  rust:\n    - stable\n    - nightly\n")))
check("a dated nightly in a block-list matrix fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  toolchain:\n    - stable\n    - nightly-2026-08-01\n")))
check("beta in a block-list matrix fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  rust:\n    - 1.85.0\n    - beta\n")))
check("a matrix under the key version fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  version: [stable, nightly]\n")))
check("a matrix include entry naming nightly fails",
      nightly_problems(lambda r: wf(r, "matrix:\n  include:\n    - rust: nightly\n      os: ubuntu-latest\n")))
# Nightly behaviour under a stable toolchain.
# Each of the five below was the sole exercise of a branch nothing pinned:
# mutating that branch left the suite green.
check("an unstable flag in RUSTDOCFLAGS fails",
      nightly_problems(lambda r: wf(r, 'env:\n  RUSTDOCFLAGS: "-Z unstable-options"\n')))
check("a channel key in a workflow fails",
      nightly_problems(lambda r: wf(r, "jobs:\n  b:\n    steps:\n      - uses: x\n        with:\n          channel: nightly\n")))
check("a doubled colon is not a key and passes",
      nightly_problems(lambda r: wf(r, "jobs:\n  b:\n    steps:\n      - run: |\n          channel:: nightly\n")) == [])
check("a sibling list item is not the previous key's value",
      nightly_problems(lambda r: wf(r, "matrix:\n  - rust: stable\n  - nightly\n")) == [])
check("a hyphen-prefixed value is not the nightly channel",
      nightly_problems(lambda r: wf(r, "jobs:\n  b:\n    steps:\n      - with:\n          toolchain: pre-beta\n")) == [])

check("RUSTC_BOOTSTRAP in a workflow fails",
      nightly_problems(lambda r: wf(r, "env:\n  RUSTC_BOOTSTRAP: 1\n")))
check("a CARGO_UNSTABLE_ variable fails",
      nightly_problems(lambda r: wf(r, "env:\n  CARGO_UNSTABLE_BUILD_STD: std,core\n")))
check("a -Z cargo flag in a workflow fails",
      nightly_problems(lambda r: wf(r, "run: cargo build -Zunstable-options --release\n")))
check("a -Z in RUSTFLAGS fails",
      nightly_problems(lambda r: wf(r, "env:\n  RUSTFLAGS: -Zbuild-std\n")))
check("a -Z in an inline RUSTFLAGS assignment fails",
      nightly_problems(lambda r: wf(r, "run: RUSTFLAGS=-Zbuild-std cargo build --release\n")))
# What passes: stable, and words that are not a toolchain.
check("a -Z on another tool is not a cargo flag",
      nightly_problems(lambda r: wf(r, "run: ls -Z /tmp\n")) == [])
check("installing stable passes",
      nightly_problems(lambda r: wf(r, "run: |\n  rustup toolchain install stable --profile minimal\n  rustup default stable\n")) == [])
check("a matrix of stable and a pinned version passes",
      nightly_problems(lambda r: wf(r, "matrix:\n  rust:\n    - stable\n    - 1.85.0\n")) == [])
check("a branch named beta is not a toolchain, inline",
      nightly_problems(lambda r: wf(r, "on:\n  push:\n    branches: [main, beta]\n")) == [])
check("a branch named beta is not a toolchain, as a block list",
      nightly_problems(lambda r: wf(r, "on:\n  push:\n    branches:\n      - main\n      - beta\n")) == [])
check("a block list is judged by its own key, not by an earlier one",
      nightly_problems(lambda r: wf(r, "matrix:\n  rust:\n    - stable\n  os:\n    - nightly-runner\n")) == [])
check("an expression reading the matrix is judged at the matrix, not at the step",
      nightly_problems(lambda r: wf(r, "with:\n  toolchain: ${{ matrix.rust }}\n")) == [])
check("a comment about nightly passes",
      nightly_problems(lambda r: wf(r, "# Principle III: stable Rust, no nightly features on the release path.\nrun: cargo build\n")) == [])
check("a trailing comment about nightly passes",
      nightly_problems(lambda r: wf(r, "run: cargo build  # never cargo +nightly here\n")) == [])

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    workspace(root)
    wf(root, "run: rustup default nightly\n")
    problems, _ = engine.scan_nightly(root, [root / "crates" / "evreos-shell"])
    check("a nightly failure names its file and line",
          len(problems) == 1 and problems[0].startswith(".github/workflows/build.yml:1:"))

lines = list(enumerate("matrix:\n  rust:\n    - stable\n    - nightly\n".splitlines(), 1))
check("a block-list breach is reported on the item's line, not the key's",
      [n for n, _ in engine.workflow_nightly_lines(lines)] == [4])

# --- ACQUISITION --------------------------------------------------------------

def acquisition_problems(build, members=("evreos-shell",)):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        m = workspace(root, members)
        build(root)
        problems, _ = engine.scan_acquisition(m, root)
        return problems


check("a clean workspace passes the acquisition clause", acquisition_problems(lambda r: None) == [])

# `.lock` is excluded because it is generated and restates manifests the
# dependency clause already judges from the resolved graph. Nothing pinned that
# exclusion, so removing it would have silently widened the clause.
# The shared-runtime blanking. A line naming the operating system's own web
# runtime is judged on whatever ELSE it names, and the runtime's own name must
# not be read as an engine -- `MicrosoftEdgeWebView2RuntimeInstaller_chromium`
# contains one. No fixture distinguished the blanking from its absence, so the
# carve-out's mechanism was untested; only its no-op on the fixtures was.
check("the runtime's own installer name is not an engine acquisition",
      acquisition_problems(lambda r: rs(
          r,
          'download("https://x/MicrosoftEdgeWebView2RuntimeInstaller_chromium_120.zip");\n',
          "build.rs")) == [])
check("...while a real engine archive beside it still fails",
      acquisition_problems(lambda r: rs(
          r,
          'download("https://x/MicrosoftEdgeWebView2RuntimeInstaller.exe");\n'
          'download("https://x/cef_binary_120.tar.bz2");\n',
          "build.rs")))

# A binary file under a member's src/ is not read. Without the NUL guard its
# bytes are decoded and an engine name inside them reads as an acquisition.
# --- every engine name and every acquisition verb ----------------------------
# Six alternatives were the sole catcher of nothing: each could be deleted and a
# real acquisition the check catches today would go unreported, suite green.
# That is the false-PASS direction, which is the one that matters in a clause
# enforcing a NON-NEGOTIABLE principle.
for label, line in (
    ("libxul, Gecko's shipped library",
     'let url = "https://ftp.mozilla.org/pub/libxul.tar.gz";\n'),
    ("wpewebkit", 'download("https://wpe/wpewebkit-2.44.tar.xz");\n'),
    # The docstring singles the Fixed Version Runtime out as the private-copy
    # form that must fail, and only the separator-free spelling was fixtured.
    ("the fixed-version runtime, hyphenated",
     'fetch("https://msedge/Fixed-Version-Runtime_120.cab");\n'),
    ("the fixed-version runtime, underscored",
     'fetch("https://msedge/fixed_version_runtime.zip");\n'),
    ("a plain-http mirror", 'const M: &str = "http://mirror.local/chromium";\n'),
    ("include_dir!", 'static CEF: Dir = include_dir!("$CARGO_MANIFEST_DIR/cef");\n'),
    ("third-party, hyphenated", 'include!("third-party/chromium/mod.rs");\n'),
    ("third_party, underscored", 'include!("third_party/cef/mod.rs");\n'),
    # Four more the round after this one found still unpinned: the two WebKit
    # library spellings, and the two directory names `vendor` does not cover
    # because the pattern requires the slash to follow immediately.
    ("webkit2gtk", 'download("https://x/webkit2gtk-4.1.tar.xz");\n'),
    ("libwebkit", 'download("https://x/libwebkit.so.tar.gz");\n'),
    ("a vendored directory", 'include!("vendored/chromium/mod.rs");\n'),
    ("a bundled directory", 'include!("bundled/cef/mod.rs");\n'),
):
    check(f"{label} is an engine acquisition",
          acquisition_problems(lambda r, line=line: rs(r, line, "build.rs")))

# The `cef` bounds, in the passing direction the docstring names by word.
check("cefalo is not cef", acquisition_problems(
    lambda r: rs(r, 'download("https://x/cefalo-1.0.zip");\n', "build.rs")) == [])

# --- which files the acquisition clause reads --------------------------------
# Each exclusion is a deliberate narrowing and only `.lock` was pinned. These
# fail in the over-report direction -- a design note or a comment read as an
# acquisition -- so no assertion about the clause failing could reach them.
for label, path, text in (
    ("a markdown note", "crates/evreos-shell/src/NOTES.md",
     "See https://x/chromium.zip for what we rejected.\n"),
    ("a text note", "crates/evreos-shell/src/NOTES.txt",
     "Do not fetch https://x/chromium.zip.\n"),
):
    check(f"{label} is prose and is not read",
          acquisition_problems(lambda r, p=path, t=text: write(r, p, t)) == [])

for label, path, text in (
    ("a justfile", "crates/evreos-shell/build/justfile",
     "# TODO: download https://x/chromium.zip\n"),
    ("a Justfile", "crates/evreos-shell/build/Justfile",
     "# TODO: download https://x/chromium.zip\n"),
    ("a Makefile", "crates/evreos-shell/build/Makefile",
     "# TODO: download https://x/chromium.zip\n"),
    ("a python script", "crates/evreos-shell/build/gen.py",
     "# do not download https://x/chromium.zip\n"),
    ("a shell script", "crates/evreos-shell/build/fetch.sh",
     "# do not download https://x/chromium.zip\n"),
    ("a powershell script", "crates/evreos-shell/build/fetch.ps1",
     "# do not download https://x/chromium.zip\n"),
    ("a yaml file", "crates/evreos-shell/build/conf.yaml",
     "# do not download https://x/chromium.zip\n"),
):
    check(f"a hash comment in {label} is not an acquisition",
          acquisition_problems(lambda r, p=path, t=text: write(r, p, t)) == [])
    check(f"...while the same line as code in {label} is",
          acquisition_problems(
              lambda r, p=path, t=text: write(r, p, t.lstrip("# "))))

check("a binary file is not read by the acquisition clause",
      acquisition_problems(lambda r: write(
          r, "crates/evreos-shell/src/blob.dat",
          "\x00\x01chromium.zip\x00")) == [])

check("a lockfile is not read by the acquisition clause",
      acquisition_problems(lambda r: write(
          r, "crates/evreos-shell/src/Cargo.lock",
          'url = "https://example.com/chromium.zip"\n')) == [])

# An engine embedded by an include macro, with no archive extension and no
# vendor path beside it, so that branch is the only thing that can catch it.
# The two existing include_bytes! fixtures also carry `.zip` AND `vendor/`, so
# it was never the sole matcher.
check("an engine embedded by include_bytes! alone fails",
      acquisition_problems(lambda r: rs(r, 'static E: &[u8] = include_bytes!("chromium.pak");\n')))
check("a chromium blob included by include_flate alone fails",
      acquisition_problems(lambda r: rs(r, 'include_flate!(static CEF from "libcef.bin");\n')))

# The failing side: a vendored or fetched engine, however spelled.
check("a build script downloading a CEF archive fails",
      acquisition_problems(lambda r: rs(r, 'fn main() { download("https://cef-builds.spotifycdn.com/cef_binary_120.0.0.tar.bz2"); }\n', "build.rs")))
check("a vendored Chromium archive embedded at compile time fails",
      acquisition_problems(lambda r: rs(r, 'static ENGINE: &[u8] = include_bytes!("../vendor/chromium-embedded.zip");\n')))
check("a vendored path to an engine fails",
      acquisition_problems(lambda r: rs(r, 'let lib = Path::new("vendor/libcef/libcef.dll");\n')))
check("the WebView2 Fixed Version Runtime archive fails",
      acquisition_problems(lambda r: rs(r, 'const RUNTIME: &str = "Microsoft.WebView2.FixedVersionRuntime.130.0.2849.80.x64.cab";\n')))
check("a private WebKitGTK build fetched at first run fails",
      acquisition_problems(lambda r: rs(r, 'download("https://webkitgtk.org/releases/webkitgtk-2.44.0.tar.xz")\n', "src/bootstrap.rs")))
check("a Gecko runtime unpacked at run time fails",
      acquisition_problems(lambda r: rs(r, 'Command::new("tar").args(["xf", "gecko-runtime.tar.xz"]).status()?;\n')))
check("an Electron fetch spelled as a verb fails",
      acquisition_problems(lambda r: rs(r, "fetch_electron_binaries(&target_dir)?;\n")))
check("an engine acquired by a helper crate's verb fails",
      acquisition_problems(lambda r: rs(r, "download_cef::download(version, &out)?;\n", "build.rs")))

# The same call the other way round. A plain `\b` never fires after an
# underscore, so a verb suffixed to an engine's name would not be seen as an
# acquisition at all while the prefixed spelling above is.
SUFFIXED_VERBS = (
    "cef_download(url)?;\n",
    "electron_fetch()?;\n",
    "chromium_unpack(&dst)?;\n",
    "engine_install_chromium(path)?;\n",
)
for spelling in SUFFIXED_VERBS:
    check(
        "a verb suffixed to an engine name fails: " + spelling.strip(),
        acquisition_problems(lambda r, t=spelling: rs(r, t, "build.rs")),
    )

# The other side of that boundary: a verb inside a longer word is not an
# acquisition marker, so a line naming an engine beside one still passes.
check(
    "a verb embedded in a longer word is not an acquisition",
    acquisition_problems(
        lambda r: rs(r, "let cef_preinstalled = probe_runtime();\n")
    )
    == [],
)
check("a Servo build fetched on demand fails",
      acquisition_problems(lambda r: rs(r, 'let servo = fetch("https://download.servo.org/nightly/linux/servo-latest.tar.gz");\n')))
check("a QtWebEngine package installed at run time fails",
      acquisition_problems(lambda r: rs(r, "install_qtwebengine_runtime()?;\n")))
check("a vendored engine listed in the manifest's include fails",
      acquisition_problems(lambda r: write(r, "crates/evreos-shell/Cargo.toml",
                                           '[package]\nname = "evreos-shell"\nversion = "0.0.0"\ninclude = ["src/**", "vendor/cef/**"]\n')))
check("a helper script beside the build script is read",
      acquisition_problems(lambda r: write(r, "crates/evreos-shell/build/fetch.sh",
                                           "#!/bin/sh\ncurl -L https://cef-builds.spotifycdn.com/cef_binary_120.tar.bz2 | tar xj\n")))
check("a workflow unpacking an engine into the installer fails",
      acquisition_problems(lambda r: wf(r, "run: curl -sSL https://cef-builds.spotifycdn.com/cef_binary_120.tar.bz2 | tar xj -C dist/\n")))
check("the shared runtime's name does not launder an engine beside it",
      acquisition_problems(lambda r: rs(r, 'let webview2 = fetch("https://example.invalid/chromium.zip");\n')))

# The passing side: the carve-out, and browser vocabulary.
BOOTSTRAP = '''\
//! SC-003: where the system web runtime is absent, first run acquires it by
//! triggering the operating system's own installer. FR-044's carve-out.
use std::process::Command;

/// Microsoft's link to the WebView2 Evergreen Bootstrapper.
const BOOTSTRAPPER_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

pub struct RuntimeInstall { pub bytes_expected: u64, pub bytes_so_far: u64 }

pub fn download_webview2_runtime(progress: &mut RuntimeInstall) -> std::io::Result<std::path::PathBuf> {
    let installer = download_resumable(BOOTSTRAPPER_URL, "MicrosoftEdgeWebview2Setup.exe", progress)?;
    Ok(installer)
}

pub fn install_webview2_runtime(installer: &std::path::Path) -> std::io::Result<()> {
    Command::new(installer).args(["/silent", "/install"]).status()?;
    Ok(())
}

pub fn install_standalone(installer_dir: &std::path::Path) -> std::io::Result<()> {
    let standalone = installer_dir.join("MicrosoftEdgeWebView2RuntimeInstallerX64.exe");
    Command::new(standalone).arg("/silent").arg("/install").status()?;
    Ok(())
}
'''
check("the first-run bootstrap path passes: triggering the OS's own runtime installer is not acquiring an engine",
      acquisition_problems(lambda r: rs(r, BOOTSTRAP, "src/bootstrap.rs")) == [])
check("a bootstrapper stub carried in the binary passes; it installs the shared runtime and is not an engine",
      acquisition_problems(lambda r: rs(r, 'static STUB: &[u8] = include_bytes!("../vendor/MicrosoftEdgeWebview2Setup.exe");\n')) == [])
check("checking for the WebView2 runtime by name passes",
      acquisition_problems(lambda r: rs(r, 'fn webview2_runtime_present() -> bool { webview2_com::get_available_browser_version_string().is_ok() }\n')) == [])
check("linking the system WebKitGTK by name passes",
      acquisition_problems(lambda r: rs(r, "use webkit2gtk::WebView;\nlet view = webkit2gtk::WebView::new();\n", "src/linux.rs")) == [])
check("a WKWebView configuration passes",
      acquisition_problems(lambda r: rs(r, "let config = WKWebViewConfiguration::new(); install_handlers(&config);\n", "src/macos.rs")) == [])
check("a comment about a Chromium archive fetches nothing",
      acquisition_problems(lambda r: rs(r, "// never fetch chromium.zip here: FR-044\nfn f() {}\n")) == [])
check("a doc comment quoting FR-044 passes",
      acquisition_problems(lambda r: rs(r, "//! Electron, CEF and any bundled Chromium are permanently rejected; nothing here downloads one.\n")) == [])
check("a manifest comment about an engine passes",
      acquisition_problems(lambda r: write(r, "crates/evreos-shell/Cargo.toml",
                                           '[package]\nname = "evreos-shell"\nversion = "0.0.0"\n# no chromium archive is ever fetched into this crate\n')) == [])
check("the browser's own chrome is not an engine",
      acquisition_problems(lambda r: rs(r, 'let chrome = ChromeSurface::load("chrome_assets.zip"); // the browser chrome\n')) == [])
check("a WebKit CSS prefix is not an engine",
      acquisition_problems(lambda r: rs(r, 'const CSS: &str = "-webkit-appearance: none; -webkit-user-select: none";\n')) == [])
check("a Firefox profile import is not an engine acquisition",
      acquisition_problems(lambda r: rs(r, 'let bookmarks = firefox_profile.join("places.sqlite"); import_bookmarks_and_history(bookmarks)?;\n')) == [])
check("an engine's name without an acquisition passes",
      acquisition_problems(lambda r: rs(r, 'log::info!("engine: {}", if is_chromium { "chromium" } else { "webkit" });\n')) == [])
check("a fetch with no engine name passes",
      acquisition_problems(lambda r: rs(r, 'download("https://updates.example.invalid/evreos-1.2.0.msi")?;\n')) == [])
check("a crate's tests are not its runtime path",
      acquisition_problems(lambda r: rs(r, 'assert!(check_rejects("vendor/chromium.zip"));\n', "tests/prohibition.rs")) == [])
check("a crate's README is prose and is not read",
      acquisition_problems(lambda r: write(r, "crates/evreos-shell/README.md", "Never fetch a chromium.zip.\n")) == [])
check("a file outside the runtime path directories is not read; the line that names it is",
      acquisition_problems(lambda r: write(r, "crates/evreos-shell/assets/notes.sh", "curl https://x/chromium.zip\n")) == [])
check("...so the runtime-path line invoking such a helper is what fails",
      acquisition_problems(lambda r: rs(r, 'Command::new("sh").arg("assets/fetch-cef.sh").status()?;\n', "build.rs")))

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    m = workspace(root)
    script = write(root, "build-support/fetch.rs", 'fn main() { download("https://x/cef_binary.tar.bz2"); }\n')
    m["packages"][0]["targets"] = [{"kind": ["custom-build"], "name": "build-script-build", "src_path": str(script)}]
    problems, _ = engine.scan_acquisition(m, root)
    check("a build script declared outside its crate directory fails when it fetches an engine",
          len(problems) == 1 and problems[0].startswith("build-support/fetch.rs:1:"))

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    m = workspace(root)
    rs(root, 'download("https://webkitgtk.org/webkitgtk-2.44.0.tar.xz")\n', "src/bootstrap.rs")
    problems, _ = engine.scan_acquisition(m, root)
    check("an acquisition failure names its file and line",
          len(problems) == 1 and problems[0].startswith("crates/evreos-shell/src/bootstrap.rs:1:"))

# The carve-out's invariant: nothing SHARED_RUNTIME blanks may itself be an
# engine name, or the blanking would be hiding a breach rather than a runtime.
for shared in ("MicrosoftEdgeWebview2Setup.exe", "MicrosoftEdgeWebView2RuntimeInstallerX64.exe",
               "webview2", "WebView2", "WKWebView"):
    check(f"{shared} is a shared runtime, not an engine name", not engine.ENGINE_NAMES.search(shared))
    check(f"{shared} is recognised as a shared runtime", engine.SHARED_RUNTIME.fullmatch(shared))

# --- comment stripping --------------------------------------------------------

check("a URL inside a string survives Rust comment stripping",
      engine.strip_comment('let u = "https://a/b.zip"; // c', "rust") == 'let u = "https://a/b.zip"; ')
check("a block comment is removed", engine.strip_comment("a /* chromium.zip */ b", "rust") == "a  b")
check("a char literal does not open a string", engine.strip_comment("let c = '\"'; // x", "rust") == "let c = '\"'; ")
check("a hash inside quotes survives hash stripping",
      engine.strip_comment('url = "https://x/#frag" # note', "hash") == 'url = "https://x/#frag" ')
check("a shebang is a comment", engine.strip_comment("#!/bin/sh", "hash") == "")
check("no style leaves the line whole", engine.strip_comment("a # b // c", None) == "a # b // c")

# --- end to end ---------------------------------------------------------------

if shutil.which("cargo"):
    result = run_check()
    check("the repository passes the check", result.returncode == 0)
    check("...and says so in one line", result.stdout.startswith("Engine prohibition check passed:"))
else:
    result = run_check()
    check("with no cargo the check exits 2 rather than passing", result.returncode == 2)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    m = workspace(root)
    m["resolve"]["nodes"][0]["deps"] = [{"name": "cef", "pkg": "registry+x#cef@1.0.0"}]
    m["packages"].append({"id": "registry+x#cef@1.0.0", "name": "cef", "manifest_path": "/x/Cargo.toml",
                          "dependencies": [], "targets": []})
    m["resolve"]["nodes"].append({"id": "registry+x#cef@1.0.0", "deps": []})
    saved = write(root, "metadata.json", json.dumps(m))
    deny = denylist(root)

    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(saved))
    check("a prohibited dependency exits 1", result.returncode == 1)
    check("...naming the crate", "evreos-shell depends on cef directly" in result.stderr)
    check("...with the breaches before one summary line",
          result.stderr.rstrip().splitlines()[-1].startswith("Engine prohibition check FAILED: 1 breach"))

    clean = write(root, "clean.json", json.dumps(workspace(root)))
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(clean))
    check("a clean synthetic workspace exits 0", result.returncode == 0)

    write(root, "rust-toolchain.toml", '[toolchain]\nchannel = "nightly"\n')
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(clean))
    check("a nightly toolchain exits 1", result.returncode == 1 and "rust-toolchain.toml" in result.stderr)
    (root / "rust-toolchain.toml").unlink()

    rs(root, 'include_bytes!("vendor/chromium.zip");\n')
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(clean))
    check("a vendored engine archive exits 1", result.returncode == 1 and "FR-044 counts as bundling" in result.stderr)
    rs(root, BOOTSTRAP)
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(clean))
    check("the bootstrap path exits 0 end to end", result.returncode == 0)

    result = run_check("--root", str(root), "--denylist", str(root / "absent.txt"), "--metadata", str(clean))
    check("a missing deny-list exits 2, not 0", result.returncode == 2)

    empty = write(root, "empty.txt", "# nothing\n")
    result = run_check("--root", str(root), "--denylist", str(empty), "--metadata", str(clean))
    check("an empty deny-list exits 1", result.returncode == 1)

    result = run_check("--root", str(root / "nowhere"), "--denylist", str(deny), "--metadata", str(clean))
    check("a root with no Cargo.toml exits 2", result.returncode == 2)

    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(root / "missing.json"))
    check("an unreadable metadata file exits 2", result.returncode == 2)


# --- the toolchain file is parsed by content, not by extension ---------------
# rustup honours the TOML form in the extensionless `rust-toolchain` too, and
# reading that file's first line as a channel yields the literal "[toolchain]",
# which matches no channel -- so the file that most directly puts the release
# path on nightly was read and then misparsed into silence.
for label, name, body, caught in (
    ("TOML in the extensionless file", "rust-toolchain",
     '[toolchain]\nchannel = "nightly"\n', True),
    ("TOML in the .toml file", "rust-toolchain.toml",
     '[toolchain]\nchannel = "nightly"\n', True),
    ("the legacy plain form", "rust-toolchain", "nightly\n", True),
    # Each of these is TOML rustup honours, and each fell through the literal
    # `[toolchain]` prefix test to the legacy line-one-as-channel reading,
    # which matched nothing. A comment above the header is the ordinary way a
    # human writes this file.
    ("a comment above the header", "rust-toolchain",
     '# pinned for the release path\n[toolchain]\nchannel = "nightly"\n', True),
    ("a spaced header", "rust-toolchain",
     '[ toolchain ]\nchannel = "nightly"\n', True),
    ("a UTF-8 byte-order mark", "rust-toolchain",
     '\ufeff[toolchain]\nchannel = "nightly"\n', True),
    ("a dotted key", "rust-toolchain",
     'toolchain.channel = "nightly"\n', True),
    ("the beta channel", "rust-toolchain",
     '[toolchain]\nchannel = "beta"\n', True),
    ("a comment above a legacy channel name", "rust-toolchain",
     "# why we pin\nnightly\n", False),
    # A .toml that does not parse is a MISREAD file, not a legacy one. Falling
    # through to the one-line reading made it read as the literal "[toolchain]",
    # match no channel, and be counted as read and clean.
    ("an unparseable .toml is reported", "rust-toolchain.toml",
     '[toolchain]\nchannel = nightly\n', True),
    ("a duplicated table is reported", "rust-toolchain.toml",
     '[toolchain]\nchannel = "1.85.0"\n[toolchain]\nchannel = "1.85.0"\n', True),
    # The extensionless file has no such guarantee: its legacy form is not TOML
    # and failing to parse is the normal case there.
    ("an extensionless legacy file is not a parse error", "rust-toolchain",
     "stable-x86_64-unknown-linux-gnu\n", False),
    ("a custom toolchain path", "rust-toolchain",
     '[toolchain]\npath = "/opt/rust-nightly"\n', True),
    ("a pinned stable release", "rust-toolchain",
     '[toolchain]\nchannel = "1.85.0"\n', False),
    ("a table written as a string", "rust-toolchain.toml",
     'toolchain = "nightly"\n', True),
):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        path = root / name
        path.write_text(body)
        found = []
        engine.check_toolchain_file(root, path, found)
        check(f"toolchain, {label}", bool(found) is caught)

# --- a feature attribute rustfmt has split across lines ----------------------
# The per-line match needed `#![` and `feature(` on one line. rustfmt splits a
# long cfg_attr exactly where that match breaks, so the spelling a formatter
# actually produces was the one that passed.
for label, source, caught in (
    ("on one line", "#![cfg_attr(docsrs, feature(doc_cfg))]\n", True),
    ("split by rustfmt", "#![cfg_attr(\n    nightly,\n    feature(let_chains)\n)]\n", True),
    ("a split bare attribute", "#![\n    feature(let_chains)\n]\n", True),
    ("a split argument list", "#![feature(\n    let_chains\n)]\n", True),
    # The joined window must not carry `#![` from one line to an unrelated
    # `feature(` several lines below it.
    # No `;` between the decoy and the word, so only string blanking can stop
    # this reaching `feature(`. With a semicolon the pattern's own bound
    # catches it and the case proves nothing about blanking.
    ("a string decoy with no semicolon before the word",
     'fn doc() -> &str { "#![" }\nfn feature(n: u32) -> u32 { n }\n', False),
    ("a string decoy in an attribute-shaped constant",
     'const A: [&str; 2] = ["#![", "feature("]\n', False),
    # The commonest real gate: nightly features behind a Cargo feature. The
    # quoted predicate sits between `#![` and `feature(`, so excluding `"` from
    # the pattern dropped it -- which is why the window blanks strings instead.
    ("a cfg_attr with a quoted predicate",
     '#![cfg_attr(feature = "nightly", feature(let_chains))]\n', True),
    ("the same, split by rustfmt",
     '#![cfg_attr(\n    feature = "nightly",\n    feature(let_chains)\n)]\n', True),
    ("an all() of two quoted predicates",
     '#![cfg_attr(all(feature = "a", feature = "b"), feature(never_type))]\n', True),
    ("a cfg that gates nothing on nightly",
     '#![cfg(feature = "x")]\npub fn f() {}\n', False),
    # The window is blanked by the shared Rust scanner, and this check depends
    # on every rule that scanner has. A second, weaker copy lived here with no
    # char-literal rule and no escape handling; it both rejected compliant
    # crate roots and blanked away a genuine gate. These cases fail if the
    # engine check stops using the shared scanner, and the crate-policy suite
    # fails if the scanner itself loses a rule.
    ("a doc attribute with an escaped quote, beside a method named feature",
     '#![forbid(unsafe_code)]\n#![doc = "pass \\" to quote an argument"]\n'
     'pub fn configure(b: &mut B) { b.feature("html") }\n', False),
    ("a char literal beside a decoy and a function named feature",
     'fn doc(c: char) -> &str { if c == \'"\' { "#![" } else { "" } }\n'
     'fn feature(n: u32) -> u32 { n }\n', False),
    ("an escaped quote inside a decoy string",
     'const MSG: &str = "he said \\"#![";\npub fn feature(n: u32) -> u32 { n }\n', False),
    ("a real gate after a doc string holding an escaped quote",
     '#![cfg_attr(docsrs, doc = "needs \\" quoting", feature(doc_cfg))]\n', True),
    ("a real gate after a char literal holding a quote",
     'fn q(c: char) -> bool { c == \'"\' }\n#![feature(let_chains)]\n', True),
    ("the word in prose", "// this feature (of the API) is stable\npub fn f() {}\n", False),
    ("ordinary source", "pub fn f() {}\n", False),
):
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        path = root / "lib.rs"
        path.write_text(source)
        found = []
        engine.check_rust_source_nightly(root, path, found)
        check(f"feature attribute, {label}", bool(found) is caught)

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
