#!/usr/bin/env python3
"""Enforce that evreos-net is the only crate with a path to the network.

WHAT THIS CHECKS, and why.

FR-007a is a closed enumeration of what may leave the machine, and SC-014
proves conformance by capturing and classifying every outbound request. Both
depend on there being exactly one place traffic originates: crates/evreos-net,
the egress chokepoint, whose `Purpose` and `Endpoint` types are what make a
transmission a reviewable event. A network-capable dependency anywhere else is
a second egress path -- traffic the chokepoint's types never judged -- and the
point of this check is that the diff which would add one is the diff that
fails the build. This check reads the graph `cargo metadata` resolves and
fails on:

  REACH        a workspace crate other than evreos-net from which a crate
               named in scripts/checks/network-capable-crates.txt is
               reachable in the resolved graph BY ANY ROUTE THAT DOES NOT
               PASS THROUGH evreos-net. The exemption is the crate and the
               paths through it, and both halves are deliberate: the shell
               depends on evreos-net, and evreos-net will one day hold the
               one transport dependency, so every crate above it would reach
               that transport transitively -- which is the chokepoint
               working, not a second path. The walk therefore never expands
               evreos-net's own edges; a listed crate found any other way
               fails, with the chain named. The graph is read with every
               feature enabled and every target's dependencies included, and
               --locked, so the verdict is about the committed Cargo.lock.
               Dev- and build-dependencies are in that graph and are not
               excused: a test that opens a socket is a transmission SC-014's
               capture would have to explain, and an entry excusing one is
               exactly the diff a review has to see.

  DECLARED     a dependency a non-exempt workspace member's own manifest
               declares on a listed crate that the resolved graph does not
               reach -- optional, or behind a target this machine is not.
               The name alone fails: an optional network dependency is one
               feature flag from being an egress path. Only the workspace
               members' own declarations are read this way; a registry
               package's declared-but-unresolved dependencies are ones Cargo
               will never build for this workspace, and judging them would
               fail the tree over code that cannot ship.

THE DENY-LIST is the plain-text form scripts/checks/README.md fixes for a
committed list -- one entry per line, `#` starting a comment, blank lines
ignored -- with the same deliberate narrowing the engine prohibition's list
states: it may not be empty, because a deny-list's empty state is its weakest
and this one is seeded from the day it lands. The list's own header states the
criterion for an entry and that it grows in review.

WHERE evreos-net IS ABSENT from the workspace, nothing is exempt and every
member is held to the rule -- the same rule with an empty exemption, which is
the correct reading of a workspace that has no chokepoint yet: no crate may
reach the network at all.

WHAT THIS DOES NOT CATCH, stated so nothing is assumed of it.

The verdict is only as good as the list. A network-capable crate not yet
named -- a new HTTP client, a raw use of the standard library's own
`std::net`, which no dependency edge records -- passes here and rests on
review and on SC-014's capture, which reads what was actually sent wherever
it came from. And the check binds dependencies, not call sites: a crate above
evreos-net that reaches the transport THROUGH evreos-net's public API but
around `request(Purpose, Endpoint)` is a misuse of the chokepoint's surface,
which the chokepoint's own types and review carry, not this graph walk.

No filesystem name is compared anywhere here -- the check reads `cargo
metadata`'s package names and nothing on disk -- so scripts/checks/casefs.py
has nothing to answer for it; crate names are folded the way crates.io folds
them, in `normalise` below.
"""
import argparse
import json
import re
import subprocess
import sys
from collections import deque
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = Path(__file__).resolve().parents[2]
DENYLIST = HERE / "network-capable-crates.txt"

# The one crate permitted to reach the network, by package name.
CHOKEPOINT = "evreos-net"


class CheckError(Exception):
    """The check cannot reach a verdict. An unrun check is not a pass."""


def normalise(name):
    """crates.io treats names case-insensitively and `-` as `_`; so does this."""
    return name.strip().lower().replace("_", "-")


def read_denylist(path):
    """One crate name per line; `#` starts a comment; blank lines are ignored.

    Returns (names, problems). A missing file is a CheckError: the list is a
    committed input read on every run, and a missing input is not an empty
    one. An empty list is a problem rather than a pass -- the narrowing the
    docstring states -- because it makes the REACH clause a check over
    nothing. The chokepoint's own name may not be listed: an entry naming
    evreos-net would make the exemption and the denial one rule fighting
    itself, and whichever won would be an accident of ordering.
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
        if name == CHOKEPOINT:
            problems.append(
                f"{path.name}:{number}: {entry} names the chokepoint itself; the crate "
                "the rule exempts cannot also be a crate the rule denies"
            )
            continue
        if name in names:
            problems.append(f"{path.name}:{number}: {entry} is listed twice")
        names.append(name)
    if not names:
        problems.append(
            f"{path.name}: names no crate; an empty deny-list is a reach clause over "
            "nothing, and this list is seeded, so an empty file means every name was deleted"
        )
    return names, problems


def cargo_metadata(root):
    """The graph Cargo resolves for the workspace at `root`.

    Every feature, so a network stack behind an optional dependency is seen;
    every target, which is `cargo metadata`'s default; and --locked, so the
    verdict is about the committed Cargo.lock rather than about a graph this
    run resolved and nobody reviewed.
    """
    command = ["cargo", "metadata", "--format-version", "1", "--all-features", "--locked"]
    try:
        completed = subprocess.run(command, cwd=root, capture_output=True, text=True)
    except FileNotFoundError:
        raise CheckError("cargo not found; the REACH clause reads `cargo metadata`") from None
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


def second_egress_paths(metadata, denylist):
    """REACH and DECLARED: every way a non-exempt member gets to a listed crate.

    Walks the resolved graph outward from each member other than the
    chokepoint, never expanding the chokepoint's own node -- a path through
    evreos-net is the chokepoint working -- and reports the first chain that
    reaches each listed crate. Then reads each such member's declared
    dependencies, so a listed name the resolve did not reach fails too.
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
    checked = 0
    for member in member_packages(metadata):
        if normalise(member["name"]) == CHOKEPOINT:
            continue
        checked += 1
        parent = {member["id"]: None}
        queue = deque([member["id"]])
        while queue:
            current = queue.popleft()
            if normalise(name_of(current)) == CHOKEPOINT:
                continue  # the chokepoint's own edges are its licence, not a path
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
                        f"{member['name']} reaches network-capable {name_of(dependency)} {how} "
                        "without passing through evreos-net: " + " -> ".join(chain)
                        + "; the chokepoint is the only permitted egress path"
                    )
                queue.append(dependency)
        for declared in member.get("dependencies", []):
            name = normalise(declared.get("name", ""))
            if name in denied and name not in {
                normalise(name_of(identifier)) for identifier in parent
            }:
                problems.append(
                    f"{member['name']} declares a dependency on {declared['name']} that the "
                    "resolved walk did not reach (optional, or behind a target this machine "
                    "is not); a network-capable name outside evreos-net is a second egress "
                    "path either way"
                )
    return problems, checked


def check_workspace(root, denylist_path, metadata=None):
    """Run the check. Returns (problems, summary); no problems is a pass.

    Raises CheckError when a verdict cannot be reached.
    """
    root = Path(root).resolve()
    if not (root / "Cargo.toml").is_file():
        raise CheckError(f"{root}: no Cargo.toml; there is no workspace to check")
    denylist, problems = read_denylist(denylist_path)
    if metadata is None:
        metadata = cargo_metadata(root)
    members = member_packages(metadata)
    if not members:
        raise CheckError(
            "the metadata names no workspace member; a check over nothing is not a pass"
        )
    exempt = sum(1 for member in members if normalise(member["name"]) == CHOKEPOINT)

    found, checked = second_egress_paths(metadata, denylist)
    problems += found

    summary = (
        f"{checked} crates held to the chokepoint, {exempt} exempt as evreos-net, "
        f"{len(denylist)} network-capable names, "
        f"{len(metadata.get('packages', []))} packages in the graph"
    )
    return problems, summary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="workspace root; the repository by default"
    )
    parser.add_argument(
        "--denylist", default=str(DENYLIST), help="the network-capable-crates list to read"
    )
    parser.add_argument(
        "--metadata",
        help="a saved `cargo metadata --format-version 1` JSON to read instead of running cargo",
    )
    args = parser.parse_args()

    try:
        metadata = load_metadata_file(args.metadata) if args.metadata else None
        problems, summary = check_workspace(args.root, args.denylist, metadata)
    except CheckError as error:
        print(f"Egress chokepoint check could not run: {error}", file=sys.stderr)
        return 2

    if problems:
        for problem in problems:
            print(f"  FAIL  {problem}", file=sys.stderr)
        print(
            f"\nEgress chokepoint check FAILED: {len(problems)} breach(es). See FR-007a and "
            "SC-014 in specs/001-evreos-v1/spec.md, and the docstring of "
            "scripts/checks/check_egress_chokepoint.py for the two clauses and the one "
            "exemption.",
            file=sys.stderr,
        )
        return 1

    print(f"Egress chokepoint check passed: {summary}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
