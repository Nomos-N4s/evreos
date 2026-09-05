#!/usr/bin/env python3
"""Tests for the purpose enum check. The check is CI's authority to fail a
build over an enum edit, so its own behaviour is checked rather than assumed
-- above all that drift fails in BOTH directions: an enum edited without the
specification amendment, and a specification amended without the enum.

Run: python3 scripts/checks/test_check_purpose_enum.py
"""
import subprocess
import sys
import tempfile
from pathlib import Path

import check_purpose_enum as agreement

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / "check_purpose_enum.py"
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

HISTORY = ["PageLoad", "CertificateStatus", "SubmittedSearch", "HandOff"]
NON_HISTORY = [
    "UpdateCheck", "BlockingListRefresh", "SurfaceDelivery", "DiagnosticReport",
    "SignIn", "WalletRead", "ClaimCodeRedemption { code: ClaimCode }",
    "WithdrawalRequest { amount: MinorUnits }", "MerchantCatalogueRead",
    "ClickOut { reference: ClickOutReference }",
]
BULLETS = ["Page load", "Certificate status", "Submitted search", "Hand-off"]


def spec_text(bullets=BULLETS):
    lines = "\n".join(f"  - **{name}**: what it carries and to whom." for name in bullets)
    return (
        "- **FR-007a**: Browsing history is the record of where the member has\n"
        "  been. The transmissions that may carry it are exactly the four\n"
        "  below, and the list is exhaustive:\n"
        f"{lines}\n"
        "\n"
        "  Anything not on that list is forbidden, whether it runs in the\n"
        "  foreground or the background.\n"
    )


def purpose_source(history=HISTORY, non_history=NON_HISTORY,
                   wrapper=("HistoryBearing(HistoryBearing)", "NonHistory(NonHistory)")):
    def body(variants):
        return "\n".join(f"    {variant}," for variant in variants)
    return (
        "pub enum Purpose {\n" + body(wrapper) + "\n}\n\n"
        "pub enum HistoryBearing {\n" + body(history) + "\n}\n\n"
        "pub enum NonHistory {\n" + body(non_history) + "\n}\n"
    )


def run(spec=None, source=None):
    """(problems, summary) for a fixture spec and a fixture purpose.rs."""
    with tempfile.TemporaryDirectory() as tmp:
        spec_path = Path(tmp) / "spec.md"
        spec_path.write_text(spec_text() if spec is None else spec, encoding="utf-8")
        purpose_path = Path(tmp) / "purpose.rs"
        purpose_path.write_text(
            purpose_source() if source is None else source, encoding="utf-8"
        )
        return agreement.check_agreement(spec_path, purpose_path)


def mentions(problems, *fragments):
    return any(all(fragment in problem for fragment in fragments) for problem in problems)


def run_check(*args):
    return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True)


# --- the repository itself ----------------------------------------------------

problems, summary = agreement.check_agreement(REPO / agreement.SPEC, REPO / agreement.PURPOSE)
check("the repository's enum and FR-007a agree", problems == [])
check("...and all fourteen purposes are read",
      "4 history-bearing" in summary and "10 non-history" in summary)

# --- the name mapping ---------------------------------------------------------

for spec_name, variant in (("Page load", "PageLoad"), ("Certificate status", "CertificateStatus"),
                           ("Submitted search", "SubmittedSearch"), ("Hand-off", "HandOff")):
    check(f"{spec_name!r} maps to {variant}", agreement.variant_name(spec_name) == variant)

# --- agreement ----------------------------------------------------------------

problems, _ = run()
check("an agreeing fixture passes", problems == [])

# A fifth history-bearing variant added without the specification amendment.
problems, _ = run(source=purpose_source(history=HISTORY + ["SuggestionFetch"]))
check("a fifth history-bearing variant fails", problems != [])
check("...naming the amendment rule",
      mentions(problems, "SuggestionFetch", "amendment"))

# One of FR-007a's four removed from the enum.
problems, _ = run(source=purpose_source(history=HISTORY[:-1]))
check("a removed history-bearing variant fails", problems != [])
check("...naming the transmission left untyped",
      mentions(problems, "'Hand-off'", "no HandOff variant"))

# A money purpose misfiled into the history-bearing set.
misfiled_non_history = [v for v in NON_HISTORY if v != "WalletRead"]
problems, _ = run(source=purpose_source(history=HISTORY + ["WalletRead"],
                                        non_history=misfiled_non_history))
check("a misfiled money purpose fails", problems != [])
check("...as a money purpose in the history-bearing set",
      mentions(problems, "money purpose WalletRead", "history-bearing set"))
check("...and as a hole in the non-history set",
      mentions(problems, "non-history set has no WalletRead"))

# An infrastructure purpose misfiled the same way is named with its requirement.
problems, _ = run(source=purpose_source(history=HISTORY + ["UpdateCheck"]))
check("a misfiled infrastructure purpose fails",
      mentions(problems, "UpdateCheck", "FR-014", "history-bearing set"))

# The other direction: the specification amended while the enum stands still.
problems, _ = run(spec=spec_text(BULLETS + ["Sync push"]))
check("a spec-side addition the enum lacks fails", problems != [])
check("...naming the missing variant", mentions(problems, "'Sync push'", "no SyncPush"))

# An eleventh non-history purpose is an ordinary reviewable diff, not a breach:
# FR-007a's closure governs the history-bearing set, and the docstring states
# this non-catch.
problems, _ = run(source=purpose_source(non_history=NON_HISTORY + ["TokenRenewal"]))
check("a new non-history purpose beyond the known ten passes", problems == [])

# SHAPE: the wrapper must be exactly the two sets, in the structural form the
# reachability argument rests on.
problems, _ = run(source=purpose_source(
    wrapper=("HistoryBearing(HistoryBearing)", "NonHistory(NonHistory)", "Other(u8)")))
check("a third Purpose variant fails", mentions(problems, "Purpose's variants"))
problems, _ = run(source=purpose_source(
    wrapper=("NonHistory(NonHistory)", "HistoryBearing(HistoryBearing)")))
check("a reordered wrapper fails too; the convention is the exact shape",
      mentions(problems, "Purpose's variants"))

# --- what is read as a variant ------------------------------------------------

# A variant named only in a comment is prose; the real removal still fails.
commented = purpose_source(history=HISTORY[:-1]).replace(
    "    SubmittedSearch,", "    SubmittedSearch,\n    // HandOff stays out until amended,")
problems, _ = run(source=commented)
check("a variant named in a comment is not a variant",
      mentions(problems, "no HandOff variant"))

# A decoy enum inside a string literal does not shadow the real one.
decoy = 'const DOC: &str = "enum HistoryBearing { Decoy }";\n' + purpose_source()
problems, _ = run(source=decoy)
check("an enum spelled inside a string is not read", problems == [])

# An attribute on a variant is not its name.
problems, _ = run(source=purpose_source(
    history=["#[deprecated]\n    PageLoad"] + HISTORY[1:]))
check("an attribute on a variant is skipped", problems == [])

# --- the verdict the check cannot reach ---------------------------------------

with tempfile.TemporaryDirectory() as tmp:
    spec_path = Path(tmp) / "spec.md"
    purpose_path = Path(tmp) / "purpose.rs"
    purpose_path.write_text(purpose_source(), encoding="utf-8")

    for label, text, expected in (
        ("a spec with no FR-007a marker", "nothing here\n", "not found"),
        ("a spec with no anchor", "- **FR-007a**: no enumeration follows.\n", "anchor"),
        ("a spec with no closing sentence",
         "- **FR-007a**: are exactly the four below, and the list is exhaustive:\n"
         "  - **Page load**: x.\n", "closing sentence"),
        ("a spec with no bullet",
         "- **FR-007a**: and the list is exhaustive:\n\n"
         "  Anything not on that list is forbidden.\n", "bullet"),
    ):
        spec_path.write_text(text, encoding="utf-8")
        try:
            agreement.check_agreement(spec_path, purpose_path)
            check(f"{label} is a CheckError", False)
        except agreement.CheckError as error:
            check(f"{label} is a CheckError", expected in str(error))

    spec_path.write_text(spec_text(), encoding="utf-8")
    purpose_path.write_text("pub enum Purpose {}\n", encoding="utf-8")
    try:
        agreement.check_agreement(spec_path, purpose_path)
        check("a purpose.rs without the two sets is a CheckError", False)
    except agreement.CheckError as error:
        check("a purpose.rs without the two sets is a CheckError",
              "HistoryBearing" in str(error))

    try:
        agreement.check_agreement(Path(tmp) / "absent.md", purpose_path)
        check("a missing spec is a CheckError", False)
    except agreement.CheckError:
        check("a missing spec is a CheckError", True)

    try:
        agreement.check_agreement(spec_path, Path(tmp) / "absent.rs")
        check("a missing purpose.rs is a CheckError", False)
    except agreement.CheckError:
        check("a missing purpose.rs is a CheckError", True)

# --- exit codes ---------------------------------------------------------------

result = run_check()
check("the repository passes the check", result.returncode == 0)
check("...and says so in one line", result.stdout.startswith("Purpose enum check passed:"))

with tempfile.TemporaryDirectory() as tmp:
    spec_path = Path(tmp) / "spec.md"
    spec_path.write_text(spec_text(), encoding="utf-8")
    purpose_path = Path(tmp) / "purpose.rs"
    purpose_path.write_text(
        purpose_source(history=HISTORY + ["SuggestionFetch"]), encoding="utf-8"
    )
    result = run_check("--spec", str(spec_path), "--purpose", str(purpose_path))
    check("a disagreement exits 1", result.returncode == 1)
    check("...naming the variant", "SuggestionFetch" in result.stderr)
    check("...with the disagreements before one summary line",
          result.stderr.rstrip().splitlines()[-1].startswith(
              "Purpose enum check FAILED: 1 disagreement"))

    result = run_check("--spec", str(Path(tmp) / "absent.md"), "--purpose", str(purpose_path))
    check("a missing spec exits 2, not 0", result.returncode == 2)

print(f"\n{PASSED}/{PASSED + FAILED} passed")
sys.exit(1 if FAILED else 0)
