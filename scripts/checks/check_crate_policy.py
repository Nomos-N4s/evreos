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
FORBID = re.compile(r"^\s*#!\[\s*forbid\s*\(([^)]*)\)\s*\]", re.MULTILINE)

# Every table Cargo reads path dependencies from, at the top level and under
# each `[target.<cfg>]`.
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


def load_toml(path):
    with open(path, "rb") as handle:
        return tomllib.load(handle)


def lint_level(table, name):
    """The level a `[lints.rust]`-shaped table sets for `name`, or None.

    Cargo accepts both `name = "forbid"` and
    `name = { level = "forbid", priority = 1 }`.
    """
    value = table.get(name)
    if isinstance(value, dict):
        return value.get("level")
    return value


def strip_non_code(source):
    """`source` with every non-code region blanked, newlines kept.

    An inner attribute inside a comment or a string is not an attribute -- it
    is text the compiler never reads -- and one inside a function body is
    block-scoped rather than crate-scoped, so none of them forbids anything.
    Matching them let a crate pass by commenting its own forbid out, which is
    the realistic way in: comment it, try `unsafe`, forget to restore it.

    This is a scanner rather than a set of patterns because the regions nest
    and interleave. Line comments come FIRST: Rust's lexer ends one at the
    newline, so a `/*` or an `r"` written inside prose opens nothing. Reading
    those as real openers blanked everything to the end of the file, and a
    crate root that plainly carried its forbid was reported as omitting it --
    one character in a doc comment away from failing a build over a compliant
    file.
    """
    out, i, n = [], 0, len(source)

    def blank(text):
        return "".join(character if character == "\n" else " " for character in text)

    while i < n:
        rest = source[i:]
        if rest.startswith("//"):
            end = source.find("\n", i)
            end = n if end == -1 else end
            out.append(blank(source[i:end]))
            i = end
        elif rest.startswith("/*"):
            depth, j = 1, i + 2          # Rust block comments nest
            while j < n and depth:
                if source.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif source.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(blank(source[i:j]))
            i = j
        else:
            raw = re.match(r'(?:b|br|rb|r)(#*)"', rest)
            if raw and ("r" in raw.group(0)):
                hashes = raw.group(1)
                close = '"' + hashes
                end = source.find(close, i + raw.end())
                end = n if end == -1 else end + len(close)
                out.append(blank(source[i:end]))
                i = end
                continue
            plain = re.match(r'b?"', rest)
            if plain:
                j = i + plain.end()
                while j < n and source[j] != '"':
                    j += 2 if source[j] == "\\" else 1
                j = min(j + 1, n)
                out.append(blank(source[i:j]))
                i = j
                continue
            out.append(source[i])
            i += 1
    return "".join(out)


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


def inherits_workspace_lints(manifest):
    return manifest.get("lints", {}).get("workspace") is True


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
    return lint_level(manifest.get("lints", {}).get("rust", {}), "unsafe_code") != "forbid"


def relative(root, path):
    return path.resolve().relative_to(root).as_posix() or "."


def path_dependencies(manifest):
    """Every `path = ...` a manifest declares, from every table Cargo reads.

    `[workspace.dependencies]` is deliberately NOT one of them; see
    `inherited_workspace_paths`. Declaring a path there does not make the crate
    a member -- cargo reports it in `workspace_members` and builds it with
    `-p` only once some member inherits it -- so following it unconditionally
    failed workspaces that cargo builds cleanly.
    """
    tables = [manifest.get(table, {}) for table in DEPENDENCY_TABLES]
    for target in manifest.get("target", {}).values():
        if not isinstance(target, dict):
            continue
        tables.extend(target.get(table, {}) for table in DEPENDENCY_TABLES)
    for table in tables:
        if not isinstance(table, dict):
            continue
        for dependency in table.values():
            if isinstance(dependency, dict) and "path" in dependency:
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
    for table in DEPENDENCY_TABLES:
        entries = member_manifest.get(table, {})
        if not isinstance(entries, dict):
            continue
        for name, spec in entries.items():
            if not (isinstance(spec, dict) and spec.get("workspace") is True):
                continue
            source = declared.get(name)
            if isinstance(source, dict) and "path" in source:
                yield source["path"]


def workspace_members(root, manifest, problems):
    """The member directories, resolved as Cargo resolves them."""
    workspace = manifest.get("workspace")
    if workspace is None:
        problems.append("Cargo.toml: no [workspace] table; there are no members to check")
        return []

    excluded = {(root / path).resolve() for path in workspace.get("exclude", [])}
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
            dependency = (base / path).resolve()
            if dependency.is_relative_to(root):
                add(dependency, relative(root, manifest_path))

    if "package" in manifest:
        add(root, "Cargo.toml")
    for pattern in workspace.get("members", []):
        if any(character in pattern for character in "*?["):
            matches = sorted(path for path in root.glob(pattern) if path.is_dir())
            if not matches:
                problems.append(f"Cargo.toml: members pattern {pattern!r} matches nothing")
        else:
            matches = [root / pattern]
        for match in matches:
            add(match, "Cargo.toml")
    return members


def crate_roots(crate_dir, manifest):
    """Every crate root Cargo would compile for this package.

    The conventional two, `src/lib.rs` and `src/main.rs`, plus the auto-discovered
    binaries under `src/bin/` and any `[lib]` or `[[bin]]` path the manifest
    declares. Each is its own crate, so a forbid in one does not reach another.
    """
    lib = manifest.get("lib", {})
    candidates = [crate_dir / lib.get("path", "src/lib.rs"), crate_dir / "src" / "main.rs"]
    candidates += sorted((crate_dir / "src" / "bin").glob("*.rs"))
    candidates += sorted((crate_dir / "src" / "bin").glob("*/main.rs"))
    candidates += [
        crate_dir / target["path"] for target in manifest.get("bin", []) if "path" in target
    ]
    roots = []
    for candidate in candidates:
        if candidate.is_file() and candidate not in roots:
            roots.append(candidate)
    return roots


def check_root(manifest, problems):
    """The workspace root must itself keep the forbid."""
    lints = manifest.get("workspace", {}).get("lints", {}).get("rust", {})
    level = lint_level(lints, "unsafe_code")
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
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
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

    name = manifest.get("package", {}).get("name")
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
        if not forbids_unsafe(crate_root.read_text(encoding="utf-8")):
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
