#!/usr/bin/env python3
"""Tests for the egress chokepoint check. The check is CI's authority to fail
a build over a dependency edge, so its own behaviour is checked rather than
assumed -- above all that the one exemption is exactly evreos-net and the
paths through it, and nothing wider: a route around the chokepoint fails
however many other routes pass through it.

Run: python3 scripts/checks/test_check_egress_chokepoint.py
"""
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import check_egress_chokepoint as egress

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check_egress_chokepoint.py"
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

def metadata(root, members, edges=(), declared=()):
    """A `cargo metadata` shape: `members` are workspace crates under `root`,
    `edges` are (from, to) resolved dependencies, and `declared` are (from, to)
    dependencies a manifest names whether or not the resolve reached them."""
    names = list(dict.fromkeys(list(members) + [b for _, b in edges]))

    def identity(name):
        if name in members:
            return f"path+file://{root}/crates/{name}#0.0.0"
        return f"registry+https://github.com/rust-lang/crates.io-index#{name}@1.0.0"

    packages = []
    for name in names:
        dependencies = [{"name": b, "optional": False} for a, b in edges if a == name]
        dependencies += [{"name": b, "optional": True} for a, b in declared if a == name]
        packages.append({
            "id": identity(name),
            "name": name,
            "manifest_path": str(Path(root) / "crates" / name / "Cargo.toml"),
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


def denylist(root, names=("reqwest", "hyper", "tokio", "socket2")):
    return write(root, "network-capable.txt", "# test list\n" + "\n".join(names) + "\n")


def run_check(*args):
    return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True)


# --- the deny-list ------------------------------------------------------------

names, problems = egress.read_denylist(egress.DENYLIST)
check("the committed deny-list reads cleanly", problems == [])
for seed in ("reqwest", "hyper", "hyper-util", "ureq", "curl", "curl-sys", "isahc",
             "attohttpc", "surf", "minreq", "tokio-tungstenite", "tungstenite",
             "quinn", "h2", "h3", "trust-dns-resolver", "hickory-resolver",
             "socket2", "mio", "tokio"):
    check(f"the committed deny-list is seeded with {seed}", seed in names)
check("the committed deny-list does not name the chokepoint", "evreos-net" not in names)

with tempfile.TemporaryDirectory() as tmp:
    path = write(tmp, "list.txt", "# comment\n\nReqwest  # trailing\nHyper_Util\n")
    names, problems = egress.read_denylist(path)
    check("comments and blank lines are ignored, names normalised",
          names == ["reqwest", "hyper-util"] and problems == [])

    path = write(tmp, "twice.txt", "tokio\nnot a crate\ntokio\n")
    names, problems = egress.read_denylist(path)
    check("a duplicated entry is a problem", any("listed twice" in p for p in problems))
    check("a malformed entry is a problem", any("not a crate name" in p for p in problems))

    # The exemption and the denial may not name the same crate: whichever won
    # would be an accident of ordering, so the entry itself is refused.
    path = write(tmp, "self.txt", "tokio\nevreos_net\n")
    names, problems = egress.read_denylist(path)
    check("an entry naming the chokepoint is a problem",
          any("chokepoint itself" in p for p in problems))
    check("...and is not added to the list", names == ["tokio"])

    path = write(tmp, "empty.txt", "# nothing here\n")
    names, problems = egress.read_denylist(path)
    check("an empty deny-list is a problem, not a pass", names == [] and problems)
    check("...and the problem says why", problems and "seeded" in problems[0])

    try:
        egress.read_denylist(Path(tmp) / "absent.txt")
        check("a missing deny-list is a CheckError", False)
    except egress.CheckError:
        check("a missing deny-list is a CheckError", True)

# --- REACH --------------------------------------------------------------------

DENY = ["reqwest", "hyper", "tokio", "socket2"]
MEMBERS = ["evreos-shell", "evreos-net", "evreos-engine"]


def paths(members, edges=(), declared=()):
    problems, _ = egress.second_egress_paths(metadata("/w", members, edges, declared), DENY)
    return problems


check("a graph with no network-capable crate passes",
      paths(MEMBERS, edges=[("evreos-shell", "evreos-net"),
                            ("evreos-shell", "evreos-engine")]) == [])

problems = paths(MEMBERS, edges=[("evreos-shell", "reqwest")])
check("a direct dependency outside the chokepoint fails", len(problems) == 1)
check("...and is reported as direct",
      problems and "directly" in problems[0] and "evreos-shell -> reqwest" in problems[0])

problems = paths(MEMBERS, edges=[("evreos-engine", "helper"), ("helper", "hyper")])
check("a transitive route around the chokepoint fails", len(problems) == 1)
check("...and the chain that reaches it is named",
      problems and "evreos-engine -> helper -> hyper" in problems[0]
      and "through helper" in problems[0])

# The exemption itself: the chokepoint may hold the transport...
check("evreos-net reaching a network-capable crate passes",
      paths(MEMBERS, edges=[("evreos-net", "reqwest"), ("reqwest", "tokio")]) == [])
# ...and the paths through it are the chokepoint working, not a second path.
check("a path through evreos-net passes",
      paths(MEMBERS, edges=[("evreos-shell", "evreos-net"),
                            ("evreos-net", "reqwest")]) == [])
problems = paths(MEMBERS, edges=[("evreos-shell", "evreos-net"),
                                 ("evreos-net", "tokio"),
                                 ("evreos-shell", "helper"), ("helper", "tokio")])
check("a route around the chokepoint fails even when one through it exists",
      len(problems) == 1 and "evreos-shell -> helper -> tokio" in problems[0])

problems = paths(MEMBERS, edges=[("evreos-shell", "Tokio")])
check("case does not hide a listed name", len(problems) == 1)
problems, _ = egress.second_egress_paths(
    metadata("/w", MEMBERS, edges=[("evreos-shell", "Trust_Dns_Resolver")]),
    DENY + ["trust-dns-resolver"])
check("underscores do not hide a listed name", len(problems) == 1)
check("a name that merely contains a listed one is not listed",
      paths(MEMBERS, edges=[("evreos-shell", "tokio-util")]) == [])

# Without the chokepoint in the workspace nothing is exempt: the same rule
# with an empty exemption, which is what a workspace with no evreos-net means.
problems = paths(["evreos-shell"], edges=[("evreos-shell", "reqwest")])
check("with no evreos-net member the rule binds every crate", len(problems) == 1)

# --- DECLARED -----------------------------------------------------------------

problems = paths(MEMBERS, declared=[("evreos-shell", "tokio")])
check("a declared dependency the resolve did not reach fails on its name",
      len(problems) == 1 and "did not reach" in problems[0])

check("the chokepoint's own declared dependencies are exempt",
      paths(MEMBERS, declared=[("evreos-net", "reqwest")]) == [])

# A registry package's declared-but-unresolved dependency is code Cargo will
# never build for this workspace; judging it would fail the tree over nothing.
check("a registry package's unresolved declaration is not judged",
      paths(MEMBERS, edges=[("evreos-shell", "serde")],
            declared=[("serde", "tokio")]) == [])

# --- the whole check ----------------------------------------------------------

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    write(root, "Cargo.toml", "[workspace]\nmembers = []\n")
    deny = denylist(root)

    try:
        egress.check_workspace(
            root, deny, metadata={"workspace_members": [], "packages": []}
        )
        check("a workspace with no members is a CheckError", False)
    except egress.CheckError as error:
        check("a workspace with no members is a CheckError", "not a pass" in str(error))

    try:
        egress.check_workspace(root / "nowhere", deny, metadata=metadata("/w", MEMBERS))
        check("a root with no Cargo.toml is a CheckError", False)
    except egress.CheckError:
        check("a root with no Cargo.toml is a CheckError", True)

    problems, summary = egress.check_workspace(root, deny, metadata=metadata("/w", MEMBERS))
    check("a clean workspace passes end to end", problems == [])
    check("...and the summary counts the exemption",
          "1 exempt" in summary and "2 crates held" in summary)

# --- exit codes ---------------------------------------------------------------

if shutil.which("cargo"):
    result = run_check()
    check("the repository passes the check", result.returncode == 0)
    check("...and says so in one line",
          result.stdout.startswith("Egress chokepoint check passed:"))
else:
    result = run_check()
    check("with no cargo the check exits 2 rather than passing", result.returncode == 2)

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    write(root, "Cargo.toml", "[workspace]\nmembers = []\n")
    deny = denylist(root)

    broken = write(root, "broken.json", json.dumps(
        metadata(str(root), MEMBERS, edges=[("evreos-shell", "reqwest")])))
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(broken))
    check("a second egress path exits 1", result.returncode == 1)
    check("...naming the crate and the rule",
          "evreos-shell reaches network-capable reqwest directly" in result.stderr)
    check("...with the breaches before one summary line",
          result.stderr.rstrip().splitlines()[-1].startswith(
              "Egress chokepoint check FAILED: 1 breach"))

    clean = write(root, "clean.json", json.dumps(
        metadata(str(root), MEMBERS, edges=[("evreos-net", "reqwest")])))
    result = run_check("--root", str(root), "--denylist", str(deny), "--metadata", str(clean))
    check("a clean synthetic workspace exits 0", result.returncode == 0)

    result = run_check("--root", str(root), "--denylist", str(root / "absent.txt"),
                       "--metadata", str(clean))
    check("a missing deny-list exits 2, not 0", result.returncode == 2)

    empty = write(root, "empty.txt", "# nothing\n")
    result = run_check("--root", str(root), "--denylist", str(empty), "--metadata", str(clean))
    check("an empty deny-list exits 1", result.returncode == 1)

    result = run_check("--root", str(root / "nowhere"), "--denylist", str(deny),
                       "--metadata", str(clean))
    check("a root with no Cargo.toml exits 2", result.returncode == 2)

    result = run_check("--root", str(root), "--denylist", str(deny),
                       "--metadata", str(root / "missing.json"))
    check("an unreadable metadata file exits 2", result.returncode == 2)

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
