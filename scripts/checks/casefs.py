#!/usr/bin/env python3
"""Ask the filesystem for a name the way the platforms this project ships to do.

Windows is tier 1 and macOS is tier 2, and both have a CASE-INSENSITIVE
filesystem by default -- NTFS and APFS. The release installers are built there.
These checks run on a case-sensitive Linux runner, so every name they build from
a literal, and every suffix they compare, asks that runner for a spelling the
author may never have used -- and the answer there is silence rather than a
failure: the file is not found, so nothing is read, so nothing is reported.

Seven spellings of that one question have each produced a finding on this
branch: a suffix comparison, a glob, a constructed literal path, a membership in
a lookup table, a membership in an exclusion set, a first-match lookup, and a
multi-segment path. So the question is asked in one place, and a new reading of
a file gets these rather than writing an eighth.

The count is stated because it kept being wrong. Twice a round declared the
class closed and the next one found another spelling, once inside the function
the sweep had just edited; once a recorded, reasoned exception to keeping this
in one place produced a defect within a single round.

What deliberately does NOT come through here is a path a MANIFEST declares.
That name is the author's own, Cargo resolves it exactly as a check would, and
where its case does not match the file the build fails on the case-sensitive
runner rather than passing quietly -- so a check is not the last line there. An
auto-discovered file needs no declaration and is compiled in silence, which is
the case these functions exist for.

One manifest key is an exception to that reasoning and is left alone knowingly:
a `exclude` entry whose case does not match its directory fails nothing. It
excludes nothing instead, so a crate Cargo skips on the release platform is
CHECKED here -- an over-report on code that is in the tree either way, and the
safer direction of the two.
"""

from pathlib import Path

# The suffix a Rust source file carries.
RUST_SUFFIX = ".rs"


def suffix_of(path):
    """A path's suffix, lowercased."""
    return Path(path).suffix.lower()


def is_rust_source(path):
    """Whether `path` names a Rust source file."""
    return str(path).lower().endswith(RUST_SUFFIX)


def folded_in(name, names):
    """Whether `name` is one of `names`, compared with case folded.

    The set-membership spelling of the same question. `skip={"target"}` did not
    skip `TARGET/`, so Cargo's whole build output -- every vendored crate source
    it holds -- was read by two clauses that report on what they find there.
    """
    return name.lower() in {each.lower() for each in names}


def named_files(directory, name):
    """Every file in `directory` called `name`, matched with case folded.

    A LIST, not one path, and that is the point. The release runners can hold
    only one of `main.rs` and `MAIN.RS`; the case-sensitive runner these checks
    run on can hold both, and answering with the first would put the reading
    back where it started -- one spelling read, its sibling silent. Sorted, so
    a report is stable.

    Empty where the directory or the file is absent. A member whose manifest is
    committed before its sources has no `src/` at all, and listing a directory
    that is not there raises, which puts a traceback where a verdict belongs.
    """
    return _named(directory, name, want_dir=False)


def named_dirs(directory, name):
    """Every subdirectory of `directory` called `name`, matched with case folded."""
    return _named(directory, name, want_dir=True)


def resolve_dirs(root, relative):
    """Every directory `root / relative` names, each segment matched with case
    folded. Empty where any segment is absent.

    A multi-segment path is the same question asked once per segment, and it was
    asked raw: an installer published to `target/Packaging/Windows` was not found
    under `target/packaging/windows`, so the gate that measures it reported no
    artefact -- which is an unmeasured entry rather than a failure.

    A list, for the reason `named_files` is a list: the runner these checks run
    on can hold two directories differing only in case, and answering with one
    of them leaves the other unread.
    """
    current = [Path(root)]
    for segment in Path(relative).parts:
        current = [found for base in current for found in named_dirs(base, segment)]
    return current


def _named(directory, name, want_dir):
    if directory is None:
        return []
    directory = Path(directory)
    if not directory.is_dir():
        return []
    wanted = name.lower()
    return [
        path for path in sorted(directory.iterdir())
        if path.name.lower() == wanted
        and (path.is_dir() if want_dir else path.is_file())
    ]
