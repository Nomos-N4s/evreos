#!/usr/bin/env python3
"""Tests for the crate policy check. The check is CI's authority to fail a
build over a manifest line, so its own behaviour is checked rather than
assumed -- in particular that it cannot pass vacuously, and that the allowlist
excuses exactly the crate it names and nothing beside it.

Run: python3 scripts/checks/test_check_crate_policy.py
"""
import importlib.util
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check_crate_policy.py"
spec = importlib.util.spec_from_file_location("policy", SCRIPT)
policy = importlib.util.module_from_spec(spec)
spec.loader.exec_module(policy)

PASSED = FAILED = 0


def check(name, condition):
    global PASSED, FAILED
    if condition:
        PASSED += 1
    else:
        FAILED += 1
        print(f"FAIL: {name}", file=sys.stderr)


# --- fixtures -----------------------------------------------------------------

FORBID = "#![forbid(unsafe_code)]\n"
LIB = FORBID + "pub fn f() {}\n"
MAIN = FORBID + "fn main() {}\n"
INHERIT = "[lints]\nworkspace = true\n"
ROOT_LINTS = '[workspace.lints.rust]\nunsafe_code = "forbid"\n'


def crate(name, lints=INHERIT, files=None, extra="", directory=None):
    """A member's description: its manifest tail and the files under it."""
    return {
        "name": name,
        "lints": lints,
        "files": {"src/lib.rs": LIB} if files is None else files,
        "extra": extra,
        "directory": directory or f"crates/{name}",
    }


def write_crate(root, spec):
    directory = root / spec["directory"]
    directory.mkdir(parents=True)
    (directory / "Cargo.toml").write_text(
        f'[package]\nname = "{spec["name"]}"\nversion = "0.0.0"\nedition = "2024"\n'
        f"{spec['extra']}\n{spec['lints']}",
        encoding="utf-8",
    )
    for relative, content in spec["files"].items():
        path = directory / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


def scenario(crates, root_lints=ROOT_LINTS, members=None, exclude=(), allowlist="", root_extra=""):
    """Build a workspace in a temporary directory and run the check over it.

    Returns (problems, crates checked, crates carved out), as the check does.
    """
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        if members is None:
            members = [spec["directory"] for spec in crates]
        listed = ", ".join(f'"{member}"' for member in members)
        excluded = ", ".join(f'"{path}"' for path in exclude)
        (root / "Cargo.toml").write_text(
            f'[workspace]\nresolver = "3"\nmembers = [{listed}]\nexclude = [{excluded}]\n'
            f"{root_lints}{root_extra}",
            encoding="utf-8",
        )
        for spec in crates:
            write_crate(root, spec)
        if allowlist is not None:
            (root / "allowlist.txt").write_text(allowlist, encoding="utf-8")
        return policy.check_workspace(root, root / "allowlist.txt")


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


# --- the repository itself ---------------------------------------------------

problems, crates, carved = policy.check_workspace(policy.REPO, policy.ALLOWLIST)
check("the repository passes under the committed allowlist", problems == [])
check("the repository has members to check", crates > 0)

parse_problems = []
policy.read_allowlist(policy.ALLOWLIST, parse_problems)
check("the committed allowlist parses cleanly", parse_problems == [])

# --- a clean workspace -------------------------------------------------------

problems, crates, carved = scenario(
    [crate("a"), crate("b", files={"src/main.rs": MAIN})]
)
check("a complete workspace passes", problems == [])
check("...and reports every member", crates == 2)
check("...with no carve-outs", carved == 0)

# --- INHERITANCE -------------------------------------------------------------

problems, _, _ = scenario([crate("a", lints="")])
check("omitting [lints] fails", problems != [])
check("...and names the manifest", mentions(problems, "crates/a/Cargo.toml"))

problems, _, _ = scenario([crate("a", lints="[lints]\nworkspace = false\n")])
check("[lints] workspace = false fails", mentions(problems, "crates/a/Cargo.toml"))

# A crate that repeats the forbid in its own table has not lifted the lint, but
# it has stopped inheriting, and the task's clause is on inheritance.
problems, _, _ = scenario([crate("a", lints='[lints.rust]\nunsafe_code = "forbid"\n')])
check("a private lints table with forbid still fails", problems != [])
check("...as an inheritance failure", mentions(problems, "omits [lints] workspace = true"))
check("...not as a carve-out", not mentions(problems, "lifts"))

# --- ROOT --------------------------------------------------------------------

problems, _, _ = scenario([crate("a", files={"src/lib.rs": "pub fn f() {}\n"})])
check("src/lib.rs without forbid fails", mentions(problems, "crates/a/src/lib.rs", "forbid(unsafe_code)"))

problems, _, _ = scenario([crate("a", files={"src/main.rs": "fn main() {}\n"})])
check("src/main.rs without forbid fails", mentions(problems, "crates/a/src/main.rs"))

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": LIB, "src/main.rs": "fn main() {}\n"})]
)
check("with both roots, only the one that omits it is named",
      mentions(problems, "src/main.rs") and not mentions(problems, "src/lib.rs"))

problems, _, _ = scenario(
    [crate("a", files={"src/main.rs": MAIN, "src/bin/tool.rs": "fn main() {}\n"})]
)
check("a src/bin root is its own crate and needs its own forbid",
      mentions(problems, "crates/a/src/bin/tool.rs"))

problems, _, _ = scenario(
    [crate("a", files={"src/main.rs": MAIN, "src/bin/tool/main.rs": "fn main() {}\n"})]
)
check("a src/bin/<name>/main.rs root is found", mentions(problems, "src/bin/tool/main.rs"))

problems, _, _ = scenario(
    [crate("a", extra='[[bin]]\nname = "cli"\npath = "src/cli.rs"\n',
           files={"src/lib.rs": LIB, "src/cli.rs": "fn main() {}\n"})]
)
check("a declared [[bin]] path is a root", mentions(problems, "crates/a/src/cli.rs"))

problems, _, _ = scenario(
    [crate("a", extra='[lib]\npath = "src/other.rs"\n', files={"src/other.rs": "pub fn f() {}\n"})]
)
check("a declared [lib] path is a root", mentions(problems, "crates/a/src/other.rs"))

problems, _, _ = scenario(
    [crate("a", extra='[lib]\npath = "src/other.rs"\n', files={"src/other.rs": LIB})]
)
check("a declared [lib] path carrying the forbid passes", problems == [])

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": "//! #![forbid(unsafe_code)]\npub fn f() {}\n"})]
)
check("a forbid quoted in a doc comment does not count", problems != [])

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": "#![forbid(unsafe_code, missing_docs)]\npub fn f() {}\n"})]
)
check("a forbid grouped with another lint counts", problems == [])

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": "//! Docs.\n\n#![forbid(unsafe_code)]\npub fn f() {}\n"})]
)
check("a forbid after a doc comment counts", problems == [])

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": "#![deny(unsafe_code)]\npub fn f() {}\n"})]
)
check("deny is not forbid", problems != [])

problems, _, _ = scenario(
    [crate("a", files={"src/lib.rs": "#![cfg_attr(not(test), forbid(unsafe_code))]\npub fn f() {}\n"})]
)
check("a conditional forbid does not count", problems != [])

problems, _, _ = scenario([crate("a", files={"src/util.rs": LIB})])
check("a crate with no visible root fails rather than passing on nothing",
      mentions(problems, "crates/a", "no crate root"))

# --- CARVE-OUT ---------------------------------------------------------------

LIFTED = '[lints.rust]\nunsafe_code = "allow"\n'
UNSAFE_LIB = "pub fn f() -> u8 { unsafe { core::ptr::read(&1u8) } }\n"

problems, _, carved = scenario([crate("a"), crate("ffi", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB})])
check("lifting the forbid without an entry fails", mentions(problems, "crates/ffi/Cargo.toml", "lifts"))
check("...and the message names the allowlist", mentions(problems, "allowlist.txt"))
check("...and the clean crate is not blamed", not mentions(problems, "crates/a"))
check("...and nothing is counted as carved out", carved == 0)

problems, _, _ = scenario([crate("ffi", lints="", files={"src/lib.rs": UNSAFE_LIB})])
check("a crate with no lints table at all has lifted the forbid", mentions(problems, "lifts"))

problems, crates, carved = scenario(
    [crate("a"), crate("ffi", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB})],
    allowlist="ffi\n",
)
check("a listed crate may lift the forbid", problems == [])
check("...and may omit the root attribute", problems == [])
check("...and is counted as a carve-out", carved == 1)
check("...among all the members", crates == 2)

problems, _, _ = scenario([crate("a")], allowlist="a\n")
check("an entry for a crate that still forbids fails",
      mentions(problems, "allowlist.txt", "a is listed but does not lift"))

problems, _, _ = scenario([crate("a")], allowlist="ghost\n")
check("an entry naming no member fails", mentions(problems, "ghost names no workspace member"))

problems, _, _ = scenario(
    [crate("ffi", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB})],
    allowlist="# the one carve-out\n\nffi  # trailing note\n",
)
check("comments and blank lines in the allowlist are ignored", problems == [])

problems, _, _ = scenario(
    [crate("ffi", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB})],
    allowlist="ffi\nffi\n",
)
check("a duplicated entry fails", mentions(problems, "allowlist.txt:2", "listed twice"))

problems, _, _ = scenario([crate("a")], allowlist=None)
check("a missing allowlist fails rather than reading as empty", mentions(problems, "allowlist.txt", "missing"))

# The allowlist excuses exactly the crate it names. A second lifting crate is
# still caught.
problems, _, carved = scenario(
    [
        crate("ffi", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB}),
        crate("sneaky", lints=LIFTED, files={"src/lib.rs": UNSAFE_LIB}),
    ],
    allowlist="ffi\n",
)
check("a second lifting crate is caught beside a listed one",
      mentions(problems, "crates/sneaky/Cargo.toml") and not mentions(problems, "crates/ffi"))
check("...and only the listed one is counted", carved == 1)

# --- the workspace root ------------------------------------------------------

problems, _, _ = scenario([crate("a")], root_lints='[workspace.lints.rust]\nunsafe_code = "deny"\n')
check("a root that relaxes the forbid fails even when every crate is clean",
      mentions(problems, "Cargo.toml: [workspace.lints.rust] unsafe_code is 'deny'"))

problems, _, _ = scenario([crate("a")], root_lints="")
check("a root with no workspace lints fails", mentions(problems, "[workspace.lints.rust]"))

problems, _, _ = scenario(
    [crate("a")],
    root_lints='[workspace.lints.rust]\nunsafe_code = { level = "forbid", priority = -1 }\n',
)
check("the table form of a lint level is read", problems == [])

# --- member resolution -------------------------------------------------------

problems, crates, _ = scenario([], members=[])
check("a workspace with no members fails rather than passing vacuously",
      mentions(problems, "no workspace members"))
check("...and reports zero crates", crates == 0)

problems, crates, _ = scenario([crate("a"), crate("b")], members=["crates/*"])
check("glob members are resolved", problems == [] and crates == 2)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = ["crates/*"]\n{ROOT_LINTS}', encoding="utf-8"
    )
    write_crate(root, crate("a"))
    (root / "crates" / "notacrate").mkdir()
    (root / "allowlist.txt").write_text("", encoding="utf-8")
    problems, _, _ = policy.check_workspace(root, root / "allowlist.txt")
check("a globbed directory with no Cargo.toml fails, as it does under cargo",
      mentions(problems, "crates/notacrate has no Cargo.toml"))

problems, _, _ = scenario([crate("a")], members=["crates/*", "extra/*"])
check("a members pattern matching nothing fails", mentions(problems, "'extra/*' matches nothing"))

problems, crates, _ = scenario(
    [crate("a"), crate("vendored", lints="", files={"src/lib.rs": UNSAFE_LIB})],
    members=["crates/a"],
    exclude=["crates/vendored"],
)
check("an excluded directory is not a member", problems == [] and crates == 1)

# Cargo makes a path dependency under the workspace root a member whether or
# not `members` lists it, and builds it with whatever lints it declares. A
# crate reached only that way is where a forbid would quietly go missing.
problems, crates, _ = scenario(
    [
        crate("a", extra='[dependencies]\ny = { path = "../../lib/y" }\n'),
        crate("y", lints="", files={"src/lib.rs": UNSAFE_LIB}, directory="lib/y"),
    ],
    members=["crates/a"],
)
check("an unlisted path dependency under the root is a member", crates == 2)
check("...and is held to the policy", mentions(problems, "lib/y/Cargo.toml", "lifts"))

problems, crates, _ = scenario(
    [
        crate("a", extra='[target."cfg(windows)".dependencies]\ny = { path = "../../lib/y" }\n'),
        crate("y", lints="", files={"src/lib.rs": UNSAFE_LIB}, directory="lib/y"),
    ],
    members=["crates/a"],
)
check("a target-specific path dependency is followed too", crates == 2 and problems != [])

problems, crates, _ = scenario(
    [
        crate("a", extra='[dependencies]\nelsewhere = { path = "../../../elsewhere" }\n'),
    ],
)
check("a path dependency outside the root is not a member", crates == 1 and problems == [])

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = ["crates/a"]\n{ROOT_LINTS}', encoding="utf-8"
    )
    (root / "crates" / "a" / "src").mkdir(parents=True)
    (root / "crates" / "a" / "Cargo.toml").write_text("[package\nname = ", encoding="utf-8")
    (root / "crates" / "a" / "src" / "lib.rs").write_text(LIB, encoding="utf-8")
    (root / "allowlist.txt").write_text("", encoding="utf-8")
    problems, _, _ = policy.check_workspace(root, root / "allowlist.txt")
check("a malformed member manifest is a reported problem, not a traceback",
      mentions(problems, "crates/a/Cargo.toml"))

# --- end to end --------------------------------------------------------------


def run_check(*args):
    return subprocess.run(
        [sys.executable, str(SCRIPT), *args], capture_output=True, text=True
    )


result = run_check()
check("end-to-end: the repository passes", result.returncode == 0)
check("...and says so on stdout", "Crate policy check passed" in result.stdout)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "Cargo.toml").write_text(
        f'[workspace]\nmembers = ["crates/a"]\n{ROOT_LINTS}', encoding="utf-8"
    )
    write_crate(root, crate("a", lints=""))
    (root / "allowlist.txt").write_text("", encoding="utf-8")
    result = run_check("--root", str(root), "--allowlist", str(root / "allowlist.txt"))
check("end-to-end: a lifting crate fails the run", result.returncode == 1)
check("...with the failure on stderr", "Crate policy check FAILED" in result.stderr)
check("...naming the manifest", "crates/a/Cargo.toml" in result.stderr)


# --- a crate reached only through [workspace.dependencies] ------------------
# `cargo metadata` reports it in workspace_members and `cargo build -p` builds
# it, so it is a member and is bound by all three clauses. It was reached by no
# path at all: the per-crate tables do not mention it, and in a virtual
# workspace -- one whose root is not itself a package, which is this
# repository's shape -- nothing walked the root manifest either.
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/app"]\nresolver = "2"\n'
        '[workspace.lints.rust]\nunsafe_code = "forbid"\n'
        '[workspace.dependencies]\nshared = { path = "lib/shared" }\n'
    )
    for name, lints, source in (
        # The inheriting table is what makes `shared` a member. Without it
        # cargo omits the crate from workspace_members entirely.
        ("crates/app",
         "[lints]\nworkspace = true\n[dependencies]\nshared = { workspace = true }\n",
         "#![forbid(unsafe_code)]\n"),
        ("lib/shared", "", "pub unsafe fn boom() {}\n"),
    ):
        directory = root / name
        (directory / "src").mkdir(parents=True)
        (directory / "Cargo.toml").write_text(
            f'[package]\nname = "{name.split("/")[-1]}"\nversion = "0.0.0"\n'
            f'edition = "2021"\n{lints}'
        )
        (directory / "src" / "lib.rs").write_text(source)
    allowlist = root / "allow.txt"
    allowlist.write_text("")
    found, crates, _ = policy.check_workspace(root, allowlist)
    check("an INHERITED [workspace.dependencies] path crate is a member", crates == 2)
    check("...and its lifted lint is reported",
          any("lifts unsafe_code" in problem for problem in found))
    check("...and its missing forbid is reported",
          any("omits #![forbid(unsafe_code)]" in problem for problem in found))

# ...and one no member inherits is NOT a member. cargo omits it from
# workspace_members and `cargo build -p` reports no such package, so following
# the declaration alone failed workspaces cargo builds cleanly. This fixture is
# the previous one with the inheriting [dependencies] table removed.
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    (root / "Cargo.toml").write_text(
        '[workspace]\nmembers = ["crates/app"]\nresolver = "2"\n'
        '[workspace.lints.rust]\nunsafe_code = "forbid"\n'
        '[workspace.dependencies]\nshared = { path = "lib/shared" }\n'
    )
    for name, lints, source in (
        ("crates/app", "[lints]\nworkspace = true\n", "#![forbid(unsafe_code)]\n"),
        ("lib/shared", "", "pub unsafe fn boom() {}\n"),
    ):
        directory = root / name
        (directory / "src").mkdir(parents=True)
        (directory / "Cargo.toml").write_text(
            f'[package]\nname = "{name.split("/")[-1]}"\nversion = "0.0.0"\n'
            f'edition = "2021"\n{lints}'
        )
        (directory / "src" / "lib.rs").write_text(source)
    allowlist = root / "allow.txt"
    allowlist.write_text("")
    found, crates, _ = policy.check_workspace(root, allowlist)
    check("an UNINHERITED [workspace.dependencies] path crate is not a member",
          crates == 1)
    check("...and nothing is reported against it", found == [])

# --- where a forbid attribute does and does not bind ------------------------
# Commenting the attribute out to try `unsafe`, then forgetting to restore it,
# is the realistic way a crate loses its forbid. Each of these read as forbidden
# before the matcher blanked comments and raw strings and required crate scope.
for label, source, forbids in (
    ("a crate-scoped attribute", "#![forbid(unsafe_code)]\npub fn f() {}\n", True),
    ("one after a closed function", "pub fn f() {}\n#![forbid(unsafe_code)]\n", True),
    ("a multi-lint attribute", "#![forbid(unsafe_code, missing_docs)]\n", True),
    ("one inside a block comment", "/*\n#![forbid(unsafe_code)]\n*/\n", False),
    # The nested case must put the attribute at a line start INSIDE the outer
    # comment; `/* /* #![...] */ */` never matched anyway, because the pattern
    # is anchored to the start of a line, so it proved nothing about nesting.
    ("one inside a nested block comment", "/* /* */\n#![forbid(unsafe_code)]\n*/\n", False),
    ("one inside an ordinary string", 'const H: &str = "\n#![forbid(unsafe_code)]\n";\n', False),
    ("one commented out with a line comment",
     "// #![forbid(unsafe_code)] -- while I try unsafe\npub fn f() {}\n", False),
    # A line comment ends at the newline, so an unpaired opener inside prose
    # opens nothing. Reading these as real openers blanked the rest of the file
    # and reported a crate that plainly complies as omitting its forbid.
    ("a doc comment holding an unpaired /*",
     "//! a `/*` that never closes\n\n#![forbid(unsafe_code)]\n", True),
    ("a doc comment holding an unpaired raw-string opener",
     '//! opens with r" and closes on the matching quote\n\n#![forbid(unsafe_code)]\n', True),
    ("a doc comment holding an unbalanced brace",
     "//! a block `{ ... }` with an unclosed {\n\n#![forbid(unsafe_code)]\n", True),
    ("a brace supplied by a line comment inside a body",
     "pub fn f() {\n    // closing }\n    #![forbid(unsafe_code)]\n}\n", False),
    ("one inside a raw string", 'const S: &str = r#"\n#![forbid(unsafe_code)]\n"#;\n', False),
    ("one inside a function body", "pub fn f() {\n    #![forbid(unsafe_code)]\n}\n", False),
    ("one in a doc comment", "//! #![forbid(unsafe_code)]\n", False),
):
    check(f"forbid binds ({forbids}): {label}",
          policy.forbids_unsafe(source) is forbids)

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
