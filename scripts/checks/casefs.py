#!/usr/bin/env python3
"""Ask the filesystem for a name the way the platforms this project ships to do.

Windows is tier 1 and macOS is tier 2, and both have a CASE-INSENSITIVE
filesystem by default -- NTFS and APFS. The release installers are built there.
These checks run on a case-sensitive Linux runner, so every name they build from
a literal, and every suffix they compare, asks that runner for a spelling the
author may never have used -- and the answer there is silence rather than a
failure: the file is not found, so nothing is read, so nothing is reported.

Four spellings of that one question have each produced a finding on this branch:
a suffix comparison, a glob, a constructed literal path, and a set membership.
So the question is asked in one place. A new reading of a file gets these rather
than writing a fifth.

What deliberately does NOT come through here is a path a MANIFEST declares.
That name is the author's own, Cargo resolves it exactly as a check would, and
where its case does not match the file the build fails on the case-sensitive
runner rather than passing quietly -- so a check is not the last line there. An
auto-discovered file needs no declaration and is compiled in silence, which is
the case these functions exist for.
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


def named_file(directory, name):
    """The file in `directory` called `name`, matched with case folded.

    None where the directory or the file is absent -- a member whose manifest is
    committed before its sources has no `src/` at all, and listing a directory
    that is not there raises, which puts a traceback where a verdict belongs.
    """
    return _named(directory, name, want_dir=False)


def named_dir(directory, name):
    """The subdirectory of `directory` called `name`, matched with case folded."""
    return _named(directory, name, want_dir=True)


def _named(directory, name, want_dir):
    directory = Path(directory)
    if not directory.is_dir():
        return None
    wanted = name.lower()
    for path in sorted(directory.iterdir()):
        if path.name.lower() == wanted and (path.is_dir() if want_dir else path.is_file()):
            return path
    return None
