#!/usr/bin/env python3
"""Enforce FR-042 outside the seam: no brand value in workspace Rust source.

Principle VIII allows no brand name, colour, endpoint or support address to be
hardcoded outside one brand configuration, and T034 makes that a checked fact
rather than an assertion. The brand files under brands/ are the configuration;
crates/evreos-shell/src/brand.rs is the one module that reads them. Everything
else is where a hardcoded copy would be a defect, because a copy is exactly
what the seam exists to prevent: change the brand file and a copy keeps the
old value, silently, in whatever surface it was pasted into.

HOW IT WORKS.

The forbidden literals are not a committed list beside this check: they are
READ from the brand files themselves, so a value added to a brand tomorrow is
forbidden outside the seam tomorrow, with no second list to forget. Every
`key = "value"` line of every brands/*.toml contributes its value, except two
the schema itself declares meaningless:

  - `unset`, the sentinel marking a field whose real value is undetermined
    (see brands/README.md) -- it is a placeholder, not a brand value, and the
    word appears legitimately wherever the sentinel is discussed;
  - the empty string, which the Rust schema refuses anyway and which would
    match every line of every file.

A value with a scheme -- a URL-shaped endpoint -- forbids three derived forms
besides itself: the value without its scheme, the value without its trailing
slash, and the bare host. Each is a natural partial copy of an endpoint -- a
request builder that hardcodes the host is exactly the drift the seam exists
to prevent -- and each is one dropped character or segment away from a
whole-value match. The limit that remains, stated so it stays honest: any
OTHER substring of a value passes -- a truncated host, a path segment, a
colour missing its hash -- because forbidding arbitrary substrings would
forbid fragments too short to mean anything. Whole values and those three
derived forms, nothing looser.

Then every Rust source file in the tree is scanned for those literals, except
the two permitted homes: anything under brands/ and the seam module itself.

The scanning decisions here are deliberate. The first two are the opposite of
what the other Rust-reading checks do:

  - RAW text, not `rustlex.strip_non_code`. A hardcoded brand value lives in
    a string literal -- that is the defect's natural habitat -- so blanking
    strings would blank precisely what this check reads. Comments are scanned
    too: a brand value in a comment is still a copy that drifts when the file
    changes, and over-reporting a comment is the safer direction.
  - Case-insensitive matching. The release platforms fold case in filenames
    and users do in text; a re-spelled copy of a brand value is still a copy
    of it, and the check that matched exactly would be dodged by one shifted
    letter.
  - Rust source only. T034's "workspace source" is the Cargo workspace's
    source, and that is Rust -- build scripts included, which the suite holds
    a case for. The tree's non-Rust files are tooling and prose: checks,
    workflows, specs and records, whose own fixtures and records must be able
    to name a brand value to state the rule it is behind -- Q-E2 names the
    search provider in the spec, and this check's own tests write brand files.
    Scanning them would forbid writing the rule's own record, so the scan is
    not widened past what ships.

Directory walking is case-folded through casefs for the same reason every
check folds: the release installers are built on case-insensitive
filesystems, so `TARGET/` is the build output and `BRAND.RS` is the seam
there, whatever this runner thinks. Hidden directories (leading dot) and
`target/` are never scanned -- neither is workspace source.

Exit codes follow the directory convention: 0 is a pass and says so in one
line; 1 means the tree breaks the rule, each breach printed with its file,
line, the value and the brand file field it came from; 2 means no verdict --
no brands/ directory, no brand file, a brand file outside the restricted
`key = "string"` schema the build would refuse too, or every value unset,
because a check with nothing to forbid is not a pass.
"""

import argparse
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent

# The one case-folded reader, shared with the other checks. Loaded by path so
# this file works both run directly and loaded through importlib.
sys.path.insert(0, str(HERE))
from casefs import folded_in, is_rust_source, named_files, resolve_dirs, suffix_of  # noqa: E402

# The values the schema declares meaningless; see the docstring.
NEVER_FORBIDDEN = ("", "unset")


class CheckError(Exception):
    """The check cannot reach a verdict; main() turns this into exit 2."""


def relative(root, path):
    """`path` named from the root, or absolutely when it is outside."""
    resolved = Path(path).resolve()
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError:
        return str(resolved)


def read_brand_values(root):
    """The forbidden literals: {value: origin} over every brands/*.toml.

    The parse mirrors the restricted schema in
    crates/evreos-shell/src/brand/schema.rs -- `key = "value"` lines,
    full-line comments, blank lines -- and refuses anything else, because a
    brand file this check reads more loosely than the build does is a place
    for a value to hide. It deliberately does NOT require the Rust field
    list: which fields exist is the schema's business, and learning values by
    line is what lets a new field's value be forbidden with no edit here.
    """
    directories = resolve_dirs(root, "brands")
    if not directories:
        raise CheckError(
            "no brands/ directory; the seam this check guards does not exist in this tree"
        )
    values = {}
    files = 0
    for directory in directories:
        for path in sorted(directory.iterdir()):
            if not path.is_file() or suffix_of(path) != ".toml":
                continue
            files += 1
            where = f"brands/{path.name}"
            try:
                text = path.read_text(encoding="utf-8").lstrip("﻿")
            except UnicodeDecodeError as error:
                raise CheckError(f"{where}: not valid UTF-8 ({error.reason})") from None
            for number, line in enumerate(text.splitlines(), 1):
                stripped = line.strip()
                if not stripped or stripped.startswith("#"):
                    continue
                key, equals, value_part = stripped.partition("=")
                key = key.strip()
                value_part = value_part.strip()
                shaped = (
                    equals
                    and key
                    and all(c == "_" or ("a" <= c <= "z") for c in key)
                    and len(value_part) >= 2
                    and value_part.startswith('"')
                    and value_part.endswith('"')
                )
                inner = value_part[1:-1] if shaped else ""
                if not shaped or '"' in inner or "\\" in inner:
                    raise CheckError(
                        f'{where}:{number}: not a `key = "value"` line of the restricted '
                        "schema brands/README.md states; a brand file this check cannot "
                        "read the way the build does yields no verdict"
                    )
                if inner in NEVER_FORBIDDEN:
                    continue
                origin = f"{where}: {key}"
                values[inner] = f"{values[inner]}, {origin}" if inner in values else origin
    if files == 0:
        raise CheckError("brands/ holds no .toml file; there is no brand to enforce")
    if not values:
        raise CheckError(
            "every brand value is unset; a check with nothing to forbid is not a pass"
        )
    return values


def url_forms(value):
    """The derived forms of a URL-shaped value; empty for any other value.

    Where `value` is `scheme://rest`, the forms are the scheme-less `rest`,
    the value minus its trailing slash, and the bare host -- minus any that
    equals the value itself or repeats another. See the docstring for why
    exactly these three and for what still passes.
    """
    scheme, separator, rest = value.partition("://")
    if not separator or not scheme or not rest:
        return []
    forms = []
    for form in (rest, value.rstrip("/"), rest.split("/", 1)[0]):
        if form and form != value and form not in forms:
            forms.append(form)
    return forms


def permitted_files(root):
    """The seam module's path(s), resolved with case folded.

    A list rather than one path for casefs's usual reason: this runner can
    hold two spellings where the release platforms hold one.
    """
    permitted = set()
    for src in resolve_dirs(root, "crates/evreos-shell/src"):
        for path in named_files(src, "brand.rs"):
            permitted.add(path.resolve())
    return permitted


def rust_sources(root, skipped):
    """Every Rust source file under `root`, sorted, minus the skipped homes.

    Skips hidden directories and `target/` (case folded) everywhere, and any
    directory whose resolved path is in `skipped` -- the brands directories,
    whose files are the configuration rather than a copy of it.
    """
    stack = [Path(root)]
    found = []
    while stack:
        directory = stack.pop()
        for path in sorted(directory.iterdir()):
            if path.is_dir():
                if path.name.startswith(".") or folded_in(path.name, ("target",)):
                    continue
                if path.resolve() in skipped:
                    continue
                stack.append(path)
            elif path.is_file() and is_rust_source(path):
                found.append(path)
    return sorted(found)


def check_tree(root):
    """Run the check over the tree at `root`.

    Returns (problems, forbidden-value count, files scanned); an empty
    `problems` is a pass. Raises CheckError where no verdict is possible.
    """
    root = Path(root).resolve()
    values = read_brand_values(root)
    needles = []
    for value, origin in sorted(values.items()):
        forms = [(value.lower(), value)]
        forms += [(form.lower(), form) for form in url_forms(value)]
        needles.append((forms, value, origin))
    permitted = permitted_files(root)
    skipped = {directory.resolve() for directory in resolve_dirs(root, "brands")}

    problems = []
    scanned = 0
    for path in rust_sources(root, skipped):
        if path.resolve() in permitted:
            continue
        scanned += 1
        where = relative(root, path)
        try:
            text = path.read_text(encoding="utf-8").lstrip("﻿")
        except UnicodeDecodeError as error:
            problems.append(
                f"{where}: not valid UTF-8 ({error.reason}), so it is not Rust this "
                "check can read"
            )
            continue
        for number, line in enumerate(text.splitlines(), 1):
            low = line.lower()
            for forms, value, origin in needles:
                # One report per brand value per line, naming the longest
                # matching form: where the whole value is present, its
                # derived forms are substrings of it and are not the news.
                matched = max(
                    (display for needle, display in forms if needle in low),
                    key=len,
                    default=None,
                )
                if matched is None:
                    continue
                copy = (
                    f"the brand value {value!r}"
                    if matched == value
                    else f"{matched!r}, a partial copy of the brand value {value!r}"
                )
                problems.append(
                    f"{where}:{number}: carries {copy} ({origin}); "
                    "FR-042 permits it only in brands/ and "
                    "crates/evreos-shell/src/brand.rs"
                )
    return problems, len(values), scanned


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="workspace root; the repository by default"
    )
    args = parser.parse_args()

    try:
        problems, values, scanned = check_tree(args.root)
    except CheckError as error:
        print(f"Brand seam check could not run: {error}", file=sys.stderr)
        return 2

    if problems:
        for problem in problems:
            print(f"  FAIL  {problem}", file=sys.stderr)
        print(
            f"\nBrand seam check FAILED: {len(problems)} breach(es). See Principle VIII "
            "of .specify/memory/constitution.md, FR-042 in specs/001-evreos-v1/spec.md, "
            "and brands/README.md for the seam these values belong behind.",
            file=sys.stderr,
        )
        return 1

    print(
        f"Brand seam check passed: {values} brand values from brands/, "
        f"{scanned} Rust files scanned."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
