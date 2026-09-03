#!/usr/bin/env python3
"""Tests for casefs, the one reader that asks the filesystem for a name.

It is not a check and has no verdict of its own. What it asserts is the property
three clauses rest on: that a name is found the way the platforms this project
SHIPS to find it, on the case-sensitive runner these checks RUN on.

That gap produced eight findings across four rounds, in five spellings -- a
suffix comparison, a glob, a constructed literal path, a set membership, and a
first-match lookup -- so the cases below are stated as the inputs that
distinguish this reader from each of those.
"""

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import casefs

PASSED = 0
FAILED = []


def check(name, condition):
    global PASSED
    if condition:
        PASSED += 1
    else:
        FAILED.append(name)
        print(f"FAIL: {name}", file=sys.stderr)


def tree(*relatives):
    """A temporary directory holding each named file, or, for a name ending in
    `/`, each named directory. Returned open; the caller keeps the handle so the
    directory outlives the assertions made against it."""
    handle = tempfile.TemporaryDirectory()
    root = Path(handle.name)
    for relative in relatives:
        path = root / relative.rstrip("/")
        if relative.endswith("/"):
            path.mkdir(parents=True, exist_ok=True)
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("", encoding="utf-8")
    return handle, root


# --- suffix_of ----------------------------------------------------------------

check("a suffix comes back lowercased", casefs.suffix_of(Path("a/B.RS")) == ".rs")
check("...and one already lowercase is unchanged",
      casefs.suffix_of(Path("a/b.rs")) == ".rs")
check("a mixed suffix folds too", casefs.suffix_of(Path("Setup.MsI")) == ".msi")
check("only the last suffix is the suffix",
      casefs.suffix_of(Path("evreos.TAR.GZ")) == ".gz")
check("a name with no suffix has none", casefs.suffix_of(Path("Makefile")) == "")
check("a string is accepted where a path is", casefs.suffix_of("a/B.YAML") == ".yaml")


# --- is_rust_source -----------------------------------------------------------

check("a Rust file is one", casefs.is_rust_source(Path("src/lib.rs")))
check("...whatever the case of its name", casefs.is_rust_source(Path("src/LIB.RS")))
check("...and of only part of it", casefs.is_rust_source(Path("src/Lib.Rs")))
check("a file that merely contains .rs is not one",
      not casefs.is_rust_source(Path("src/lib.rs.bak")))
check("a directory ending in .rs does not make its contents Rust",
      not casefs.is_rust_source(Path("src/vendor.rs/notes.txt")))
check("a file under such a directory still is",
      casefs.is_rust_source(Path("src/vendor.rs/mod.rs")))


# --- folded_in ----------------------------------------------------------------

check("a name is in the set under its own spelling", casefs.folded_in("target", {"target"}))
check("...and under one that differs only in case", casefs.folded_in("TARGET", {"target"}))
check("...and where the SET carries the other case", casefs.folded_in("target", {"TARGET"}))
check("a name that differs otherwise is not in it",
      not casefs.folded_in("targets", {"target"}))
check("an empty set holds nothing", not casefs.folded_in("target", set()))
check("a tuple is accepted where a set is",
      casefs.folded_in("Justfile", ("makefile", "justfile")))


# --- named_files --------------------------------------------------------------

handle, root = tree("main.rs")
check("a file is found under its own name",
      [p.name for p in casefs.named_files(root, "main.rs")] == ["main.rs"])
check("...and under a name that differs only in case",
      [p.name for p in casefs.named_files(root, "MAIN.RS")] == ["main.rs"])
check("a name that differs otherwise is not found",
      casefs.named_files(root, "lib.rs") == [])
handle.cleanup()

# The reader answers with EVERY match rather than the first. The release runners
# can hold only one of these two names; the runner these checks run on can hold
# both, and answering with the first would leave one spelling read and its
# sibling silent. Sorting puts the upper-case one first, so a first-match
# reading returns the WRONG half in exactly the case this exists for.
handle, root = tree("main.rs", "MAIN.RS")
found = [path.name for path in casefs.named_files(root, "main.rs")]
check("where both spellings stand, both are answered", found == ["MAIN.RS", "main.rs"])
check("...and the first of them is the upper-case one, which a first-match "
      "reading would have returned alone", found[0] == "MAIN.RS")
handle.cleanup()

handle, root = tree("src/")
check("a directory is not a file", casefs.named_files(root, "src") == [])
check("a directory that is not there is empty rather than a raise",
      casefs.named_files(root / "absent", "main.rs") == [])
check("None as the directory is empty too, which is what lets a caller chain",
      casefs.named_files(None, "config.toml") == [])
handle.cleanup()


# --- named_dirs ---------------------------------------------------------------

handle, root = tree("SRC/", "build/")
check("a directory is found under a folded name",
      [p.name for p in casefs.named_dirs(root, "src")] == ["SRC"])
check("...and one whose case already matches",
      [p.name for p in casefs.named_dirs(root, "BUILD")] == ["build"])
check("a directory that is not there is not found",
      casefs.named_dirs(root, "tests") == [])
handle.cleanup()

handle, root = tree("src/", "SRC/")
check("both spellings of a directory are answered",
      [p.name for p in casefs.named_dirs(root, "src")] == ["SRC", "src"])
handle.cleanup()

handle, root = tree("config.toml")
check("a file is not a directory", casefs.named_dirs(root, "config.toml") == [])
check("a missing parent is empty rather than a raise",
      casefs.named_dirs(root / "absent", "src") == [])
handle.cleanup()

# A dotted directory is the ordinary case for `.cargo`, and a hidden entry is
# not skipped the way a wildcard glob skips one.
handle, root = tree(".CARGO/config.toml")
check("a hidden directory is found under a folded name",
      [p.name for p in casefs.named_dirs(root, ".cargo")] == [".CARGO"])
found = casefs.named_dirs(root, ".cargo")
check("...and its contents are then found through it",
      [p.name for p in casefs.named_files(found[0], "CONFIG.TOML")] == ["config.toml"])
handle.cleanup()

# --- resolve_dirs -------------------------------------------------------------

handle, root = tree("target/Packaging/Windows/")
check("every segment of a path is matched with case folded",
      [str(p.relative_to(root)) for p in
       casefs.resolve_dirs(root, "target/packaging/windows")]
      == ["target/Packaging/Windows"])
check("...and a path already matching resolves to itself",
      [str(p.relative_to(root)) for p in
       casefs.resolve_dirs(root, "target/Packaging/Windows")]
      == ["target/Packaging/Windows"])
check("a segment that is not there resolves to nothing",
      casefs.resolve_dirs(root, "target/packaging/linux") == [])
check("a segment that is a file rather than a directory resolves to nothing",
      casefs.resolve_dirs(root, "target/packaging/windows/absent") == [])
check("an empty relative path is the root itself",
      casefs.resolve_dirs(root, "") == [root])
handle.cleanup()

# Two directories differing only in case can stand side by side on the runner
# these checks run on, and each may hold a different artefact.
handle, root = tree("target/packaging/windows/", "target/Packaging/WINDOWS/")
check("both resolutions of one path are answered",
      sorted(str(p.relative_to(root)) for p in
             casefs.resolve_dirs(root, "target/packaging/windows"))
      == ["target/Packaging/WINDOWS", "target/packaging/windows"])
handle.cleanup()


total = PASSED + len(FAILED)
print(f"\n{PASSED}/{total} passed")
sys.exit(1 if FAILED else 0)
