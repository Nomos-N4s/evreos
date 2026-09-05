#!/usr/bin/env python3
"""Hold evreos-net's `Purpose` enum and FR-007a's enumeration in agreement.

WHAT THIS CHECKS, and why.

FR-007a permits exactly four transmissions to carry browsing history and
states that its list is exhaustive; adding an entry "is an amendment to this
specification ... never an implementation decision". crates/evreos-net encodes
that list as the `HistoryBearing` enum inside `Purpose`, and the two copies of
one rule -- the specification's text and the enum's variants -- can drift.
This check fails when they disagree IN EITHER DIRECTION, so an enum edited
alone fails the build rather than shipping, and a specification amended alone
fails until the enum catches up. It reads the tree and fails on:

  OMITTED      a transmission FR-007a enumerates with no variant in the
               `HistoryBearing` enum. One of the four removed from the enum is
               a permitted transmission the type system no longer names, so
               the traffic would be built under some other purpose -- a
               misclassification SC-014's analysis would then repeat.

  ADDED        a `HistoryBearing` variant FR-007a does not enumerate. A fifth
               history-bearing variant is a fifth permitted transmission, which
               only a specification amendment can create; until spec.md's
               enumeration names it, the diff that adds the variant is the
               diff that fails. Where the added variant is one of the known
               NON-HISTORY purposes -- the money purposes included -- the
               failure says so: filing a purpose into the history-bearing set
               is what would let its request path carry an address, a term
               typed into the FR-003 field, or page content.

  MISFILED     a known non-history purpose absent from the `NonHistory` enum.
               Together with ADDED this closes the move in both directions: a
               purpose relocated from one set to the other fails as an
               unenumerated history-bearing variant AND as a hole in the
               non-history set, and a purpose deleted outright still fails
               here, because the transmissions it named do not stop existing
               when their type does.

  SHAPE        a `Purpose` enum whose variants are not exactly
               `HistoryBearing` and `NonHistory`. This is the structural
               convention the whole check parses, and it is also what
               discharges the reachability half of the rule: the workspace's
               one request path is typed on `Purpose`, so a request path typed
               as history-bearing is reachable only through a `HistoryBearing`
               value, and this check has just held that set to FR-007a's
               four -- a non-history purpose cannot reach it without first
               appearing in the enum body this check reads. A third variant
               would be a third category of transmission no requirement
               defines.

THE SPEC SIDE is anchored on FR-007a's own enumeration text in
specs/001-evreos-v1/spec.md, located as: the literal `**FR-007a**`, then after
it the anchor this check keys on, quoted exactly --

    and the list is exhaustive:

-- then one `- **Name**:` bullet per transmission until the closing literal
`Anything not on that list is forbidden`. Each bolded name maps to a variant
by capitalising its words and dropping spaces and hyphens: `Page load` ->
`PageLoad`, `Hand-off` -> `HandOff`. If the FR-007a marker, the anchor, the
terminator or any bullet between them cannot be found, the check exits 2
rather than passing: an enumeration it cannot read is not an enumeration it
has compared.

THE ENUM SIDE is read from crates/evreos-net/src/purpose.rs with comments and
string literals blanked first by scripts/checks/rustlex.py -- the one Rust
scanner, shared because a second weaker copy was a defect twice -- so a
variant name written in a doc comment is prose, not a variant. The variant
names are then read from each enum's brace-balanced body.

THE KNOWN NON-HISTORY PURPOSES are a committed constant below rather than
parsed from anywhere: the four infrastructure purposes carry their requirement
each (FR-014, FR-008, FR-019, FR-039), and the six money purposes are the
Apivo transmissions User Story 2 has no lawful route off the machine without,
which enter the enum as purposes and never as a crate exempted from the
chokepoint. No single specification list enumerates the ten together -- the
committed closed list of non-history DESTINATIONS is gap G16, a specification
amendment -- so the constant is the check's own record of the sets tasks.md
T035 fixed, and growing it is a visible diff to this file reviewed against
that task.

WHAT THIS DOES NOT CATCH: an eleventh NonHistory variant beyond the known
ten. FR-007a's closure governs the history-bearing set; a new non-history
purpose is an ordinary reviewable diff to purpose.rs, and refusing it here
would make this check the specification. What the new purpose may carry is
bound by the `NonHistory` definition and reviewed there.
"""
import argparse
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = Path(__file__).resolve().parents[2]
SPEC = Path("specs") / "001-evreos-v1" / "spec.md"
PURPOSE = Path("crates") / "evreos-net" / "src" / "purpose.rs"

# The one Rust scanner; see its docstring for why there is exactly one.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rustlex import strip_non_code  # noqa: E402

# Where FR-007a's enumeration starts and stops in spec.md. The anchor is the
# closing words of the sentence that declares the list exhaustive; the
# terminator opens the paragraph that forbids everything else.
FR_MARKER = "**FR-007a**"
ANCHOR = "and the list is exhaustive:"
TERMINATOR = "Anything not on that list is forbidden"
BULLET = re.compile(r"^\s*-\s+\*\*([^*]+?)\*\*\s*:", re.MULTILINE)

# The two sets' names in purpose.rs, and the required shape of the wrapper.
HISTORY_ENUM = "HistoryBearing"
NON_HISTORY_ENUM = "NonHistory"
WRAPPER_ENUM = "Purpose"

# The known non-history purposes: the four infrastructure transmissions, each
# with its requirement, and the six Apivo money transmissions. The money set
# is named separately because MISFILED reports a money purpose as one.
INFRASTRUCTURE_PURPOSES = {
    "UpdateCheck": "FR-014",
    "BlockingListRefresh": "FR-008",
    "SurfaceDelivery": "FR-019",
    "DiagnosticReport": "FR-039",
}
MONEY_PURPOSES = {
    "SignIn",
    "WalletRead",
    "ClaimCodeRedemption",
    "WithdrawalRequest",
    "MerchantCatalogueRead",
    "ClickOut",
}
NON_HISTORY_PURPOSES = set(INFRASTRUCTURE_PURPOSES) | MONEY_PURPOSES


class CheckError(Exception):
    """The check cannot reach a verdict. An unrun check is not a pass."""


def variant_name(spec_name):
    """FR-007a's bolded name as the enum spells it: words capitalised, spaces
    and hyphens dropped. `Page load` -> `PageLoad`, `Hand-off` -> `HandOff`."""
    return "".join(
        part[:1].upper() + part[1:]
        for part in re.split(r"[\s-]+", spec_name.strip())
        if part
    )


def spec_transmissions(spec_path):
    """FR-007a's enumerated transmissions, as (spec name, variant name) pairs.

    Raises CheckError when the enumeration cannot be located, because a
    comparison against text this parser did not find is a comparison against
    nothing.
    """
    spec_path = Path(spec_path)
    if not spec_path.is_file():
        raise CheckError(f"{spec_path}: missing; the spec side of the comparison")
    text = spec_path.read_text(encoding="utf-8")
    marker = text.find(FR_MARKER)
    if marker == -1:
        raise CheckError(f"{spec_path.name}: {FR_MARKER!r} not found")
    anchor = text.find(ANCHOR, marker)
    if anchor == -1:
        raise CheckError(
            f"{spec_path.name}: the enumeration anchor {ANCHOR!r} not found after "
            f"{FR_MARKER!r}; if the sentence was reworded, this check's ANCHOR moves "
            "with it in the same pull request"
        )
    end = text.find(TERMINATOR, anchor)
    if end == -1:
        raise CheckError(
            f"{spec_path.name}: the closing sentence {TERMINATOR!r} not found after "
            "the anchor"
        )
    names = [match.group(1).strip() for match in BULLET.finditer(text[anchor:end])]
    if not names:
        raise CheckError(
            f"{spec_path.name}: no `- **Name**:` bullet between the anchor and the "
            "closing sentence; an enumeration this check cannot read is not one it "
            "has compared"
        )
    return [(name, variant_name(name)) for name in names]


def enum_body(source, enum_name):
    """The brace-balanced body of `enum <enum_name> { ... }`, or None.

    `source` must already have comments and strings blanked, so the braces
    counted are the ones rustc reads.
    """
    match = re.search(rf"\benum\s+{re.escape(enum_name)}\b[^{{]*\{{", source)
    if match is None:
        return None
    depth, i = 1, match.end()
    while i < len(source) and depth:
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
        i += 1
    return source[match.end():i - 1]


def variant_names(body):
    """The variant names of one enum body: the first identifier of each
    top-level comma-separated segment, attributes skipped."""
    names = []
    segments, depth, start = [], 0, 0
    for i, ch in enumerate(body):
        if ch in "{([":
            depth += 1
        elif ch in "})]":
            depth -= 1
        elif ch == "," and depth == 0:
            segments.append(body[start:i])
            start = i + 1
    segments.append(body[start:])
    for segment in segments:
        # An attribute on a variant -- `#[deprecated] Name` -- is not its name.
        segment = re.sub(r"#\s*\[[^\]]*\]", " ", segment)
        match = re.match(r"\s*([A-Za-z_]\w*)", segment)
        if match:
            names.append(match.group(1))
    return names


def enum_sets(purpose_path):
    """The three enums' variant lists from purpose.rs, comments and strings
    blanked first. Raises CheckError when a set cannot be found."""
    purpose_path = Path(purpose_path)
    if not purpose_path.is_file():
        raise CheckError(f"{purpose_path}: missing; the enum side of the comparison")
    # The BOM is stripped for the reason the other checks strip it: an editor
    # that writes one is not a way past a check.
    source = strip_non_code(purpose_path.read_text(encoding="utf-8").lstrip("\ufeff"))
    sets = {}
    for name in (WRAPPER_ENUM, HISTORY_ENUM, NON_HISTORY_ENUM):
        body = enum_body(source, name)
        if body is None:
            raise CheckError(
                f"{purpose_path.name}: no `enum {name}`; the structural convention "
                "this check parses is the two sets nested inside Purpose"
            )
        sets[name] = variant_names(body)
    return sets


def compare(spec, sets, purpose_name):
    """Every disagreement between FR-007a's enumeration and the enums."""
    problems = []
    spec_variants = [variant for _, variant in spec]
    history = sets[HISTORY_ENUM]
    non_history = sets[NON_HISTORY_ENUM]

    # SHAPE first: everything else reasons from the two-set structure.
    if sets[WRAPPER_ENUM] != [HISTORY_ENUM, NON_HISTORY_ENUM]:
        problems.append(
            f"{purpose_name}: Purpose's variants are {sets[WRAPPER_ENUM]!r}, not exactly "
            f"[{HISTORY_ENUM!r}, {NON_HISTORY_ENUM!r}]; the request path is typed on this "
            "wrapper, so its shape is what keeps the two sets the only two categories"
        )

    for spec_name, variant in spec:
        if variant not in history:
            problems.append(
                f"{purpose_name}: FR-007a enumerates {spec_name!r} and the "
                f"history-bearing set has no {variant} variant; a permitted "
                "transmission the enum does not name would be built under some "
                "other purpose"
            )
    for variant in history:
        if variant in spec_variants:
            continue
        if variant in MONEY_PURPOSES:
            problems.append(
                f"{purpose_name}: money purpose {variant} is declared in the "
                "history-bearing set; money traffic is a different category and its "
                "request path must not be typed to carry an address, a search term "
                "or page content"
            )
        elif variant in INFRASTRUCTURE_PURPOSES:
            problems.append(
                f"{purpose_name}: non-history purpose {variant} "
                f"({INFRASTRUCTURE_PURPOSES[variant]}) is declared in the "
                "history-bearing set; its transmission carries no browsing history "
                "and may not be typed as if it did"
            )
        else:
            problems.append(
                f"{purpose_name}: history-bearing variant {variant} is not among "
                "FR-007a's enumerated transmissions; adding one is an amendment to "
                "the specification, never an implementation decision"
            )
    for variant in NON_HISTORY_PURPOSES:
        if variant not in non_history:
            problems.append(
                f"{purpose_name}: the non-history set has no {variant} variant; a "
                "known non-history purpose that leaves the enum does not stop being "
                "a transmission, it stops being a typed one"
            )
    return problems


def check_agreement(spec_path, purpose_path):
    """Run the comparison. Returns (problems, summary); no problems is a pass.

    Raises CheckError when a verdict cannot be reached.
    """
    spec = spec_transmissions(spec_path)
    sets = enum_sets(purpose_path)
    problems = compare(spec, sets, Path(purpose_path).name)
    summary = (
        f"{len(spec)} transmissions enumerated by FR-007a, "
        f"{len(sets[HISTORY_ENUM])} history-bearing and "
        f"{len(sets[NON_HISTORY_ENUM])} non-history variants read"
    )
    return problems, summary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root", default=str(REPO), help="repository root; the checkout by default"
    )
    parser.add_argument("--spec", help=f"the specification to read; {SPEC} under --root by default")
    parser.add_argument(
        "--purpose", help=f"the enum source to read; {PURPOSE} under --root by default"
    )
    args = parser.parse_args()
    root = Path(args.root)
    spec_path = Path(args.spec) if args.spec else root / SPEC
    purpose_path = Path(args.purpose) if args.purpose else root / PURPOSE

    try:
        problems, summary = check_agreement(spec_path, purpose_path)
    except CheckError as error:
        print(f"Purpose enum check could not run: {error}", file=sys.stderr)
        return 2

    if problems:
        for problem in problems:
            print(f"  FAIL  {problem}", file=sys.stderr)
        print(
            f"\nPurpose enum check FAILED: {len(problems)} disagreement(s). FR-007a in "
            "specs/001-evreos-v1/spec.md is the enumeration; changing it is a "
            "specification amendment made in the same pull request as the enum edit. "
            "See the docstring of scripts/checks/check_purpose_enum.py for the four "
            "clauses.",
            file=sys.stderr,
        )
        return 1

    print(f"Purpose enum check passed: {summary}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
