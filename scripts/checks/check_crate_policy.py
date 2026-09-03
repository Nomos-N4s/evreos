#!/usr/bin/env python3
"""Enforce the crate policy under which exactly one crate may hold `unsafe`.

WHAT THIS CHECKS, and why each clause exists.

The workspace root sets `[workspace.lints.rust] unsafe_code = "forbid"`, and
every crate repeats `#![forbid(unsafe_code)]` at its root. That is the policy
specs/001-evreos-v1/research.md records in section 2.9: exactly one shipped
crate will hold `unsafe`, the exception is narrow, named and reviewable, and
"exactly one manifest differing is the property that makes it reviewable". A
policy that lives in a manifest a later crate may simply not inherit holds only
until the first crate forgets, so this check reads every member and fails on:

  INHERITANCE  a member whose Cargo.toml omits `[lints] workspace = true`. The
               workspace lint reaches a crate only through that line; a crate
               without it is built with no `unsafe_code` lint at all, whatever
               the root manifest says. Cargo rejects a manifest that inherits
               and overrides in the same table, so this line is the whole of
               the manifest-side policy.

  ROOT         a crate root -- src/lib.rs, src/main.rs, or any other root Cargo
               would compile -- without an unconditional
               `#![forbid(unsafe_code)]`. The manifest clause is what the
               compiler enforces; the attribute is what a reader sees, and it
               holds if the file is ever built outside this workspace.

  CARVE-OUT    a crate that lifts the workspace forbid without being named in
               scripts/checks/unsafe-carveout-allowlist.txt. Lifting means not
               inheriting the workspace lints, because that is the only form
               that builds: a source-level `#![allow(unsafe_code)]` against an
               inherited forbid is rustc error E0453. The allowlist is a
               committed file, empty today, so the first carve-out lands as a
               visible diff in one file rather than as a manifest nobody
               diffed.

The allowlist records carve-outs taken, not permissions granted ahead of them:
an entry naming a crate that still forbids `unsafe`, or naming no member, fails
too. Otherwise an entry could land quietly in one pull request and be used in
another, which is the retrofit the research rejects.

The workspace root itself must keep `unsafe_code = "forbid"`. A root that
lifts it lifts it for every member at once, and the allowlist names crates, so
nothing in it can excuse that.

Manifests are read with tomllib rather than through `cargo metadata`, so the
check runs on a machine with no toolchain and every failure names a file a
reader can open. Members are resolved the way Cargo resolves them: the
`members` list with its globs, less `exclude`, plus every path dependency under
the workspace root, which Cargo makes a member whether it is listed or not.
"""
import argparse
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
ALLOWLIST = HERE / "unsafe-carveout-allowlist.txt"

# An unconditional inner attribute forbidding unsafe_code, alone or beside
# other lints. A `cfg_attr` form is deliberately not matched: a forbid that
# holds only under some configuration is not the policy.
# Whitespace may sit between `#`, `!` and `[`, in any combination. rustc applies
# `#! [forbid(...)]`, `# ![forbid(...)]` and `#!` on its own line exactly as it
# applies the adjacent spelling -- verified against the compiler, which reports
# the unsafe block as an error under every one -- so requiring any pair adjacent
# reported a compliant crate as omitting a forbid it carries. The line anchor
# keeps to spaces and tabs, so `\\s*` before `#` would not admit a mid-file line.
FORBID = re.compile(r"^[ \t]*#\s*!\s*\[\s*forbid\s*\(([^)]*)\)\s*\]", re.MULTILINE)

# Every table Cargo reads path dependencies from, at the top level and under
# each `[target.<cfg>]`.
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


class Unreadable(Exception):
    """A file this check must read is not decodable as UTF-8."""


def read_text(path):
    """The file's text. Raises Unreadable rather than a decode traceback.

    Every caller already reports a manifest it cannot parse, so a file it
    cannot decode belongs on the same path: a traceback is not a verdict, and
    an operator reading the log cannot tell it from the check crashing.
    """
    try:
        # The BOM is stripped for the reason the engine prohibition strips it:
        # an editor that writes one is not a way past a check. It is not `\s`,
        # so a crate attribute on the first line would sit behind it and the
        # anchored pattern would miss a forbid rustc honours.
        return Path(path).read_text(encoding="utf-8").lstrip("\ufeff")
    except UnicodeDecodeError as error:
        raise Unreadable(f"not valid UTF-8 ({error.reason})") from None


def load_toml(path):
    with open(path, "rb") as handle:
        try:
            return tomllib.load(handle)
        except UnicodeDecodeError as error:
            # tomllib decodes before it parses, so a non-UTF-8 manifest raises
            # this rather than a TOML error. Callers handle one kind; a
            # manifest that is unreadable for the other reason is the same
            # verdict and is reported through the same branch.
            raise tomllib.TOMLDecodeError(
                f"not valid UTF-8 ({error.reason})"
            ) from None


def lint_level(table, name):
    """The level a `[lints.rust]`-shaped table sets for `name`, or None.

    Cargo accepts both `name = "forbid"` and
    `name = { level = "forbid", priority = 1 }`.
    """
    value = table.get(name)
    if isinstance(value, dict):
        return value.get("level")
    return value


def crate_scoped(source, start):
    """Whether the match at `start` sits at crate scope, outside every brace.

    An inner attribute inside a function body applies to that block. Counting
    braces before the match is enough here: string and comment braces are
    already blanked by `strip_non_code`, and a crate root that does not parse is
    a compile error rather than this check's business.
    """
    return source.count("{", 0, start) == source.count("}", 0, start)


def forbids_unsafe(source):
    """Whether a crate root carries `#![forbid(unsafe_code)]` that binds."""
    source = strip_non_code(source)
    return any(
        lint.strip() == "unsafe_code" and crate_scoped(source, match.start())
        for match in FORBID.finditer(source)
        for lint in match.group(1).split(",")
    )


def as_table(value):
    """`value` when it is a table, an empty one otherwise.

    Every reader below walks a manifest by chained `get`. TOML admits a scalar
    where a table is expected -- `lints = "x"`, or `[bin]` written for `[[bin]]`
    -- and cargo reports that as a type error rather than crashing. This check
    is the reader when cargo is not there to say so, and it raised instead: a
    well-formed file with a wrong type ended the run with a traceback, which is
    neither a pass nor a breach nor a stated inability to decide.
    """
    return value if isinstance(value, dict) else {}


def inherits_workspace_lints(manifest):
    return as_table(as_table(manifest).get("lints")).get("workspace") is True


def lifts_forbid(manifest):
    """Whether a member is built without the workspace forbid on unsafe_code.

    Inheriting is the only way to carry it: Cargo rejects a manifest that sets
    `workspace = true` beside its own lints, and rustc rejects a source-level
    override of a forbid it was given on the command line. A crate that declares
    its own table with `unsafe_code = "forbid"` has not lifted the lint, though
    it has stopped inheriting, which the INHERITANCE clause reports on its own.
    """
    if inherits_workspace_lints(manifest):
        return False
    return lint_level(as_table(as_table(as_table(manifest).get("lints")).get("rust")), "unsafe_code") != "forbid"


# The one Rust scanner, shared with the engine prohibition check. Loaded by
# path so this file works both when run directly and when a test loads it
# through importlib from another directory.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rustlex import strip_non_code  # noqa: E402


def resolved(base, path):
    """`base / path` resolved, or None when the string names no path.

    Three call sites turn a manifest string into a path: the exclude list, the
    members list, and a dependency's `path`. Only the third was guarded against a
    NUL -- legal in TOML as an escape, illegal in a path, and `resolve()` raises
    on it -- so the same value reached a verdict written one way and a traceback
    written another. One resolver, so a fourth site cannot be added without it.

    `is_dir()` swallows the error on the glob branch, which is why `members`
    raised only for a literal pattern: the guard has to sit here rather than at
    whichever branch happened to surface it.
    """
    try:
        return (base / path).resolve()
    except (ValueError, OSError):
        return None


def relative(root, path):
    """`path` named from the workspace root, or absolutely when it is outside.

    A workspace may list a member outside its root -- `members = ["../sibling"]`
    is legal -- and `relative_to` raises on one. The path-dependency call site
    already refuses to follow a dependency outside the root; the members call
    site beside it did not, so a legal manifest ended the run with a ValueError
    where a verdict belonged.
    """
    resolved = path.resolve()
    try:
        return resolved.relative_to(root).as_posix() or "."
    except ValueError:
        return str(resolved)


def path_dependencies(manifest):
    """Every `path = ...` a manifest declares, from every table Cargo reads.

    `[workspace.dependencies]` is deliberately NOT one of them; see
    `inherited_workspace_paths`. Declaring a path there does not make the crate
    a member -- cargo reports it in `workspace_members` and builds it with
    `-p` only once some member inherits it -- so following it unconditionally
    failed workspaces that cargo builds cleanly.
    """
    tables = [manifest.get(table, {}) for table in DEPENDENCY_TABLES]
    for target in as_table(as_table(manifest).get("target")).values():
        if not isinstance(target, dict):
            continue
        tables.extend(target.get(table, {}) for table in DEPENDENCY_TABLES)
    for table in tables:
        if not isinstance(table, dict):
            continue
        for dependency in table.values():
            # A path must be a string, as `crate_roots` already requires of a
            # declared target path. Yielding another type sent it to `base / path`,
            # which raised where a verdict belonged.
            if isinstance(dependency, dict) and isinstance(dependency.get("path"), str):
                yield dependency["path"]


def inherited_workspace_paths(root_manifest, member_manifest):
    """Path dependencies this member inherits from [workspace.dependencies].

    Cargo makes such a crate a member only when a member actually inherits it.
    The declaration alone does not: `cargo metadata` omits it from
    `workspace_members` and `cargo build -p` reports no such package.
    """
    workspace = root_manifest.get("workspace")
    if not isinstance(workspace, dict):
        return
    declared = {}
    for table in DEPENDENCY_TABLES:
        entries = workspace.get(table, {})
        if isinstance(entries, dict):
            declared.update(entries)
    # Every table the member can inherit through, including those under a
    # `[target.<cfg>]` block. Reading only the top-level three let a crate
    # inherited under `[target.'cfg(unix)'.dependencies]` escape all three
    # clauses, though cargo reports it in workspace_members and builds it --
    # the same omission `path_dependencies` beside this already avoids.
    member_tables = [member_manifest.get(table, {}) for table in DEPENDENCY_TABLES]
    for target in as_table(as_table(member_manifest).get("target")).values():
        if isinstance(target, dict):
            member_tables.extend(target.get(table, {}) for table in DEPENDENCY_TABLES)
    for entries in member_tables:
        if not isinstance(entries, dict):
            continue
        for name, spec in entries.items():
            if not (isinstance(spec, dict) and spec.get("workspace") is True):
                continue
            source = declared.get(name)
            # Same string guard as its sibling above: a path of another type
            # reached `base / path` and raised.
            if isinstance(source, dict) and isinstance(source.get("path"), str):
                yield source["path"]


def workspace_members(root, manifest, problems):
    """The member directories, resolved as Cargo resolves them."""
    workspace = as_table(manifest).get("workspace")
    if workspace is None:
        problems.append("Cargo.toml: no [workspace] table; there are no members to check")
        return []

    excluded = set()
    for path in as_table(workspace).get("exclude", []):
        if not isinstance(path, str):
            continue
        directory = resolved(root, path)
        if directory is None:
            problems.append(f"Cargo.toml: exclude entry {path!r} names no path")
            continue
        excluded.add(directory)
    members = []

    def add(directory, origin):
        directory = directory.resolve()
        if directory in excluded or directory in members:
            return
        manifest_path = directory / "Cargo.toml"
        if not manifest_path.is_file():
            problems.append(f"{origin}: member {relative(root, directory)} has no Cargo.toml")
            return
        members.append(directory)
        # Cargo makes every path dependency under the workspace root a member,
        # listed or not, so a crate reached only that way is still bound by
        # the policy and is followed here the same way.
        try:
            member = load_toml(manifest_path)
        except tomllib.TOMLDecodeError:
            return  # reported when the member itself is checked
        # A crate's own `path = ...` is relative to that crate's directory.
        followed = [(directory, path) for path in path_dependencies(member)]
        # A path declared in the root's [workspace.dependencies] and INHERITED
        # here with `foo = { workspace = true }` is a member: cargo reports it
        # in workspace_members and `-p` builds it. Uninherited it is not, and
        # following it regardless failed workspaces cargo builds cleanly -- so
        # the inheritance is what is followed, not the declaration. That path
        # is written relative to the WORKSPACE ROOT, where it is declared, not
        # to the member that inherits it.
        followed += [(root, path)
                     for path in inherited_workspace_paths(manifest, member)]
        for base, path in followed:
            dependency = resolved(base, path)
            if dependency is None:
                problems.append(
                    f"{relative(root, manifest_path)}: dependency path {path!r} "
                    "names no directory"
                )
                continue
            if dependency.is_relative_to(root):
                add(dependency, relative(root, manifest_path))

    if "package" in manifest:
        add(root, "Cargo.toml")
    for pattern in as_table(workspace).get("members", []):
        if not isinstance(pattern, str):
            problems.append(f"Cargo.toml: members entry {pattern!r} is not a path")
            continue
        if any(character in pattern for character in "*?["):
            matches = sorted(path for path in root.glob(pattern) if path.is_dir())
            if not matches:
                problems.append(f"Cargo.toml: members pattern {pattern!r} matches nothing")
        else:
            directory = resolved(root, pattern)
            if directory is None:
                problems.append(f"Cargo.toml: members entry {pattern!r} names no path")
                continue
            matches = [directory]
        for match in matches:
            add(match, "Cargo.toml")
    return members


def crate_roots(crate_dir, manifest):
    """Every crate root Cargo would compile for this package.

    The conventional two, `src/lib.rs` and `src/main.rs`, plus the auto-discovered
    binaries under `src/bin/` and any `[lib]` or `[[bin]]` path the manifest
    declares. Each is its own crate, so a forbid in one does not reach another.
    """
    lib = as_table(as_table(manifest).get("lib"))
    path = lib.get("path")
    candidates = [crate_dir / (path if isinstance(path, str) else "src/lib.rs"),
                  crate_dir / "src" / "main.rs"]
    candidates += sorted((crate_dir / "src" / "bin").glob("*.rs"))
    candidates += sorted((crate_dir / "src" / "bin").glob("*/main.rs"))
    # `[bin]` written for `[[bin]]` is the ordinary slip, and it makes this a
    # table rather than a list of them. Cargo says "invalid type: map, expected
    # a sequence"; this took the string indices and raised.
    declared = as_table(manifest).get("bin")
    candidates += [
        crate_dir / target["path"]
        for target in (declared if isinstance(declared, list) else [])
        if isinstance(target, dict) and isinstance(target.get("path"), str)
    ]
    roots = []
    for candidate in candidates:
        if candidate.is_file() and candidate not in roots:
            roots.append(candidate)
    return roots


def check_root(manifest, problems):
    """The workspace root must itself keep the forbid."""
    lints = as_table(as_table(as_table(manifest).get("workspace")).get("lints")).get("rust")
    level = lint_level(as_table(lints), "unsafe_code")
    if level != "forbid":
        problems.append(
            f"Cargo.toml: [workspace.lints.rust] unsafe_code is {level!r}, must be "
            '"forbid"; lifting it here lifts it for every member at once, and the '
            "allowlist names crates, not the workspace"
        )


def read_allowlist(path, problems):
    """One package name per line; `#` starts a comment; blank lines are ignored."""
    if not path.is_file():
        problems.append(
            f"{path.name}: missing; the allowlist is a committed file read on every "
            "run, and a missing file is not an empty one"
        )
        return []
    names = []
    try:
        text = read_text(path)
    except Unreadable as error:
        problems.append(f"{path.name}: {error}")
        return []
    for number, line in enumerate(text.splitlines(), 1):
        entry = line.split("#", 1)[0].strip()
        if not entry:
            continue
        if entry in names:
            problems.append(f"{path.name}:{number}: {entry} is listed twice")
        names.append(entry)
    return names


def check_crate(root, crate_dir, allowlist, allowlist_name, problems):
    """Apply the three clauses to one member. Returns (package name, carved out)."""
    manifest_path = crate_dir / "Cargo.toml"
    where = relative(root, manifest_path)
    try:
        manifest = load_toml(manifest_path)
    except tomllib.TOMLDecodeError as error:
        problems.append(f"{where}: {error}")
        return None, False

    name = as_table(as_table(manifest).get("package")).get("name")
    if not isinstance(name, str):
        name = None
    if not name:
        problems.append(f"{where}: no [package] name")
        return None, False

    lifts = lifts_forbid(manifest)
    if name in allowlist:
        if lifts:
            # The carve-out the allowlist records: no inherited lint and no
            # forbid at the root. What the carve-out must carry instead --
            # `#![deny(unsafe_op_in_unsafe_fn)]` and a `// SAFETY:` note on
            # every block -- is the crate's own obligation, reviewed with it.
            return name, True
        problems.append(
            f"{allowlist_name}: {name} is listed but does not lift the forbid; an "
            "entry records a carve-out taken, not one granted ahead of it, so it "
            "lands in the commit that lifts the lint"
        )

    if lifts:
        problems.append(
            f"{where}: does not inherit the workspace lints and so lifts "
            f'unsafe_code = "forbid" without an entry in {allowlist_name}'
        )
    elif not inherits_workspace_lints(manifest):
        problems.append(
            f"{where}: omits [lints] workspace = true; the workspace lints reach a "
            "crate only through that line"
        )

    roots = crate_roots(crate_dir, manifest)
    if not roots:
        problems.append(
            f"{relative(root, crate_dir)}: no crate root at src/lib.rs, src/main.rs, "
            "src/bin/ or a declared [lib] or [[bin]] path; a crate whose roots this "
            "check cannot see is one it cannot vouch for"
        )
    for crate_root in roots:
        try:
            source = read_text(crate_root)
        except Unreadable as error:
            problems.append(f"{relative(root, crate_root)}: {error}, so it is not Rust this check can read")
            continue
        if not forbids_unsafe(source):
            problems.append(f"{relative(root, crate_root)}: omits #![forbid(unsafe_code)]")
    return name, False


def check_workspace(root, allowlist_path):
    """Run every clause over the workspace at `root`.

    Returns (problems, crates checked, crates carved out). An empty `problems`
    is a pass.
    """
    root = Path(root).resolve()
    allowlist_path = Path(allowlist_path)
    problems = []

    manifest_path = root / "Cargo.toml"
    if not manifest_path.is_file():
        problems.append(f"{root}: no Cargo.toml")
        return problems, 0, 0
    try:
        manifest = load_toml(manifest_path)
    except tomllib.TOMLDecodeError as error:
        problems.append(f"Cargo.toml: {error}")
        return problems, 0, 0

    check_root(manifest, problems)
    allowlist = read_allowlist(allowlist_path, problems)
    members = workspace_members(root, manifest, problems)
    if not members:
        problems.append("Cargo.toml: no workspace members; a check over nothing is not a pass")

    names = []
    carved_out = 0
    for member in members:
        name, carved = check_crate(root, member, allowlist, allowlist_path.name, problems)
        names.append(name)
        carved_out += carved

    for entry in allowlist:
        if entry not in names:
            problems.append(f"{allowlist_path.name}: {entry} names no workspace member")

    return problems, len(members), carved_out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="workspace root; the repository by default"
    )
    parser.add_argument(
        "--allowlist", default=str(ALLOWLIST), help="the carve-out allowlist to read"
    )
    args = parser.parse_args()

    problems, crates, carved_out = check_workspace(args.root, args.allowlist)

    if problems:
        print("Crate policy check FAILED:\n", file=sys.stderr)
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "\nSee the docstring of scripts/checks/check_crate_policy.py for the three "
            "clauses and what satisfies each.",
            file=sys.stderr,
        )
        return 1

    print(f"Crate policy check passed: {crates} crates, {carved_out} carve-outs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
