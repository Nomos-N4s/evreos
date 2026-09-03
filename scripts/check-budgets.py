#!/usr/bin/env python3
"""Enforce the budgets Principle II requires and FR-043 names.

WHAT THIS GATE IS, and what it deliberately is not.

Principle II: hard budgets "MUST live in one budget file in this repository and
MUST be enforced by CI gates that fail the build on regression." The Success
Criteria preamble defines three gates and says they are defined there and only
there. This script is those three, and it adds none of its own.

  BUDGET FILE  fails when the file does not describe a gateable state: an entry
               the preamble's closed list states is absent, a ratified entry
               naming no recorded founder decision, a baseline above the
               entry's stated figure, an SC-004 entry declaring no cross-check
               margin or one outside its cap, a baseline reset naming no
               recorded decision or leaving a baseline above the figure, a
               runner not pinned, SC-005's wake enumeration absent, a wake in
               it lacking a period, a processor-time bound or a justifying
               requirement, a wake bounded above the 50 ms SC-005 caps a wake
               at, or the enumerated bounds summing above the 500 ms SC-005
               allows in a 60-minute window. Not hardware dependent -- it
               compares numbers in a file -- so it blocks from M0
               unconditionally. It is what bounds the advisory period on the
               absolute gate, rather than leaving that to good intentions. It
               also fails on a file this script cannot read as the schema
               states it -- a figure with no unit, or in a unit its criterion
               does not state; a spike exemption, baseline reset or wake
               lacking a field its schema names -- because a verdict over a
               misread file is a verdict on nothing.

  ABSOLUTE     fails when a measured figure exceeds its stated value. On a
               hardware-dependent entry this is advisory until that tier's
               runner is procured and pinned, because a figure measured on an
               unnamed machine is not reproducible under SC-013 and so cannot be
               what a build fails on.

  REGRESSION   fails when a measured figure is worse than the entry's baseline
               by more than its declared tolerance. It compares one machine
               against itself, so it blocks from M0 on every entry.

THE CLOSED LIST IS READ FROM THIS SCRIPT, not off the file. A file that is
missing an entry cannot report its own omission, and iterating the entries a
file declares is exactly asking it to. So the eighteen entries the preamble
states are written here and the file is compared against them, in both
directions: one it lacks is absent, one it adds is stated by no criterion.
Adding an entry is an amendment to the specification, made in the change that
states the figure, and that change extends this list.

An UNDECLARED TOLERANCE IS ZERO, not unbounded, and so is an undeclared
cross-check margin. That direction matters: the opposite reading lets an entry
disable its own regression gate, or SC-004 its whole-machine cross-check, by
omission. The budget-file gate goes one step further on SC-004 and fails an
entry that declares no margin, because the preamble requires the declaration on
that entry; reading absence as zero is what keeps the cross-check strict rather
than absent on a file this gate has already refused.

A RATIFIED FIGURE NAMES THE DECISION THAT SET IT, in the decision register's
citation form, decisions/NNNN, and so does a baseline reset. A name, a clarify
question or a bare "yes" is not a citation: the gate can read a citation off
the file and can read nothing off a description.

A SPIKE EXEMPTION LIFTS ONE ENTRY'S ABSOLUTE GATE AND NOTHING ELSE. Recorded on
an entry while a spike establishes a figure that does not yet exist, it turns
that entry's absolute breach into an advisory. It never lifts the regression
gate, which is what stops a spike being a route around a baseline; it never
lifts the budget-file gate, which refuses a malformed exemption and one naming
another entry's figure; and it never reaches another entry, because the only
exemption an entry is read against is the one recorded on it. A malformed
exemption is not an exemption, so the absolute gate does not lift on one.
Retiring an exemption is deleting it, in the change that lands the figure;
until then the file records it, and a build produced from such a commit is not
released or tagged. --refuse-exemptions is the release job's refusal: it fails
on any recorded exemption, well formed or not, and runs nothing else, because
the build job the release job depends on has already run the gates.

THE WAKE ENUMERATION IS READ AT ITS STRICTEST. SC-005 bounds every wake at
50 ms of processor time and the enumerated wakes together at 500 ms in any
60-minute window. "Any" window is the worst one: a wake with period p seconds
fires at most floor(3600 / p) + 1 times in a window that opens on one firing
and closes on another, and that count times its bound is what it contributes.
An absent enumeration fails; an empty one, `wake = []`, passes, being the
statement that nothing on the idle path is scheduled.

A FIGURE IS COMPARED ONLY IN ITS CRITERION'S UNIT. Every entry states its unit,
and the unit is the one its criterion states -- MB, ms or percent-of-core -- so
a measurement produced in that unit is compared against a figure in the same
unit and never against one a different criterion states. This script's own
measurement is in MB, for SC-001; the harness figures arrive in the unit their
criterion states.

A MEASUREMENT IS ONE PLATFORM'S, and satisfies that platform's entry alone.
Measurements are keyed on (criterion, name, platform), an entry's whole
identity, because a figure stated per platform is one entry per platform and a
number measured for one platform is no measurement of the other. This script's
own measurement, SC-001's download size, is the size of the installer artefact
the host it runs on builds -- what the entry's condition names, "the installer
artefact CI publishes" -- read from where that platform's packaging build
publishes it and declared for that platform. A host of no tier builds no such
artefact and measures nothing: the hosted Linux runner the build job runs on
is one, Linux being the deferred platform, and the release binary it does
build is a Linux ELF that meets neither entry's condition, so it is not read.
An entry whose platform is not the measuring host's is reported unmeasured
with that reason rather than compared against another platform's artefact.
Neither installer exists yet, so both download-size entries stand unmeasured
with that reason until the installer each entry's condition names is built.

This script measures only what it can measure honestly on the machine it runs
on: SC-001's download size, on the host that builds the artefact. SC-001's
installed footprint is the disk delta after first run completes and needs an
installed artefact, and the hardware-dependent entries -- SC-002, SC-004,
SC-005, SC-006 -- are measured by the benchmark harness on a pinned runner;
this script reports each as unmeasured, with the reason, rather than inventing
a number. An unmeasured entry is not a pass: it blocks unless
--allow-unmeasured defers it, the one exception being a hardware-dependent
entry whose tier has no pinned runner, which the budget-file gate already
reports.
"""
import argparse
import collections
import datetime
import math
import re
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Criteria whose figures depend on the machine they are measured on. Their
# absolute gate waits on a pinned runner; their regression gate does not.
HARDWARE_DEPENDENT = {"SC-002", "SC-004", "SC-005", "SC-006"}

# The tolerance cap the Success Criteria preamble sets, as a percentage of the
# entry's baseline. SC-004's cross-check margin is "declared and justified
# exactly as a tolerance is": a percentage of the summed per-process figure its
# cross-check compares against, under the same cap. The preamble sets one limit
# and no second one for the margin, and a percentage is the only form in which
# a gate that reads a file and no measurement can compare a margin against it.
MAX_TOLERANCE_PCT = 5.0
MAX_CROSS_CHECK_MARGIN_PCT = MAX_TOLERANCE_PCT

# The units a figure may be stated in, and the unit each criterion states its
# figures in. A criterion absent here states no figure and carries no entry:
# SC-003 is a required experience, verified by acceptance test.
UNITS = {"MB", "ms", "percent-of-core"}
UNIT_OF = {
    "SC-001": "MB",
    "SC-002": "ms",
    "SC-004": "MB",
    "SC-005": "percent-of-core",
    "SC-006": "ms",
}

# The entries the Success Criteria preamble closes over, by criterion: nine per
# platform, eighteen across tier 1 and tier 2. The gate compares the file
# against this list rather than reading the list off the file, because a file
# missing an entry cannot report its own omission. Adding an entry is an
# amendment to the specification, and the change that states the figure adds
# it here.
PLATFORMS = ("windows", "macos")
STATED_ENTRIES = {
    "SC-001": ("download size", "installed footprint"),
    "SC-002": ("warm start", "cold start"),
    "SC-004": ("ten-tab memory",),
    "SC-005": ("60-minute window", "wake-free 1-second sample"),
    "SC-006": ("tab switch", "address-field keystroke"),
}
CLOSED_LIST = frozenset(
    (criterion, name, platform)
    for criterion, names in STATED_ENTRIES.items()
    for name in names
    for platform in PLATFORMS
)

# The decision register's citation form. A ratified figure is one a recorded
# founder decision set, and a baseline is reset upward only by one, so both
# name the decision in the form the register is cited by.
DECISION_CITATION = re.compile(r"^decisions/[0-9]{4}$")

# Field names the schema retired. An entry still written with them was not
# migrated, and is named as such rather than reported as merely incomplete.
RETIRED_FIELDS = ("figure_mb", "baseline_mb")

# The two tiers, closed. Every entry's platform maps to exactly one, and the
# budget file must declare both: the closed list of eighteen entries is checked
# against this constant rather than read off the file, for the reason the entry
# list is -- a file missing a tier cannot report its own omission. Before this
# was closed, deleting a `[runners.*]` table made that tier indistinguishable
# from an unpinned one, so every hardware-dependent entry on it went advisory
# and a measurement thirty times its figure passed the build.
TIER_OF = {"windows": "tier1", "macos": "tier2"}

# The phrase every unpinned-runner failure carries. --allow-unpinned-runners
# defers exactly the failures that carry it and nothing else.
UNPINNED = "is not pinned"
# The tag --allow-unpinned-runners selects on. The phrase above stays for the
# reader; the tag is what the deferral matches, so no text in the budget file
# can decide what a flag defers.
UNPINNED_TAG = "unpinned-runner"

# SC-005's two caps on the wake enumeration, in ms of processor time: what one
# wake completes within, and what the enumerated wakes together consume in a
# 60-minute window. The window is 3600 s, and a wake with period p fires in it
# at most floor(3600 / p) + 1 times -- once at its opening and once every
# period after, the last on its close -- which is the count "any 60-minute
# window" binds, since the criterion binds the worst one.
WAKE_BOUND_CAP_MS = 50
WAKES_WINDOW_CAP_MS = 500
WINDOW_SECONDS = 3600


# TOML specifies an integer as 64-bit signed. A value outside that is not a TOML
# integer, whatever `tomllib` accepts, and is refused as a number this file can
# carry rather than left to overflow somewhere downstream.
TOML_INT_MIN = -(2 ** 63)
TOML_INT_MAX = 2 ** 63 - 1


def is_number(value):
    """A FINITE int or float, which is what every gate here compares.

    A bool is neither: TOML has no way to write one where a number is meant,
    and Python would otherwise accept `true` as 1.

    `nan` and `inf` are neither, and that is the point rather than a detail.
    Every ordering comparison against `nan` is False, so an entry written
    `baseline = nan` passed the clause that refuses a negative baseline, passed
    the clause that refuses a baseline above the figure, and then made its
    regression gate unreachable -- the exact defect a negative baseline was
    fixed for, in a spelling nobody would notice. `inf` does the same to a
    figure. A number that disables the gate it is written into is not a number
    this file can carry.
    """
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        return False
    if isinstance(value, float):
        # Only a float can be nan or inf. Asking `math.isfinite` of an int costs
        # a conversion that RAISES on one too large to convert, so the predicate
        # written to reject a value a gate cannot compare destroyed the verdict
        # on exactly such a value.
        return math.isfinite(value)
    # An int is finite by construction but not therefore usable. TOML specifies
    # an integer as 64-bit signed, so a larger one is outside the format this
    # file is written in -- and admitting it moved the failure downstream twice
    # over: `baseline * (1 + tolerance / 100.0)` converts and raises past 1e308,
    # and rendering a magnitude for a message raises past 4300 digits, which the
    # wake sum reaches by multiplying. Bounding it here is one clause; guarding
    # each arithmetic and formatting site it can reach is an open-ended list, and
    # the last two rounds each found one more member of that list.
    return TOML_INT_MIN <= value <= TOML_INT_MAX


def is_positive_number(value):
    return is_number(value) and value > 0


def is_text(value):
    """A string with something in it. Whitespace is not a value."""
    return isinstance(value, str) and bool(value.strip())


def is_pull_request_number(value):
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def is_date(value):
    """A TOML date, written unquoted. tomllib parses one into datetime.date."""
    return isinstance(value, datetime.date)


def is_decision_citation(value):
    """A recorded founder decision, cited as the register cites it:
    decisions/NNNN. A name, a clarify question or a bare "yes" is not one; the
    gate can read a citation off the file and can read nothing off a
    description."""
    return isinstance(value, str) and DECISION_CITATION.match(value) is not None


# The sub-tables an entry may carry and the wake table, each as the fields it
# must carry with what each accepts. A sub-table carries exactly these fields:
# a misspelt one is refused rather than ignored, because an ignored field is how
# a reset that "names a decision" comes to name nothing.
SPIKE_EXEMPTION_FIELDS = (
    ("pull_request", is_pull_request_number, "the pull request's number"),
    ("figure", is_text, "the figure the spike measures, as its criterion states it"),
)
BASELINE_RESET_FIELDS = (
    ("date", is_date, "a TOML date, unquoted"),
    ("measured_cost", is_number, "a number in the entry's unit"),
    ("requirement_served", is_text, "the requirement the cost serves"),
    ("founder_decision", is_decision_citation,
     "the recorded founder decision, cited as decisions/NNNN"),
)
WAKE_FIELDS = (
    ("name", is_text, "a name"),
    ("period", is_positive_number, "a period in seconds"),
    ("processor_time_bound", is_positive_number, "a bound in ms of processor time"),
    ("justifying_requirement", is_text, "the requirement that justifies the wake"),
)


class Gate:
    """One gate's verdict over the whole budget file."""

    def __init__(self, name):
        self.name = name
        self.blocking = []
        self.advisory = []
        self.tags = []

    def block(self, message, tag=None):
        """Record a blocking failure, optionally tagged.

        `tag` is what a deferral flag selects on. It was once selected by
        searching the message text for a phrase, which put the choice in the
        hands of the file being judged: an entry whose *name* contained that
        phrase deferred its own unrelated failure, and the closed-list clause --
        the one this script does not read off the file precisely so the file
        cannot weaken it -- was turned off by a string inside the file.
        """
        self.blocking.append(message)
        self.tags.append(tag)

    def advise(self, message):
        self.advisory.append(message)

    @property
    def failed(self):
        return bool(self.blocking)


class Unreadable(Exception):
    """The budget file cannot be read, so no gate has an input."""


def load_budgets(path):
    """The budget file as a dict. Raises Unreadable rather than a traceback.

    `run_gates` already guards a misread ENTRY on the ground that a verdict over
    a misread file is a verdict on nothing. That guard sits three hundred lines
    below this function and cannot help when the file itself does not parse: the
    traceback replaced the message one level up from where it was fixed.

    The two clauses below the named ones are the ones this function was missing.
    `tomllib` does not raise only TOMLDecodeError: an integer literal of more
    than 4300 digits reaches `int()` and comes back as a plain ValueError, from
    the same interpreter limit that makes such a number unrenderable further
    down -- so a file could still put a traceback where this function promises a
    message. And the two OSError subclasses named above are the two that were
    thought of, not the two that exist: a path whose parent is a file, a
    permission, a name too long for the filesystem all arrive as some other
    OSError. The named clauses stay ahead of the general one for their wording.
    """
    try:
        with open(path, "rb") as handle:
            return tomllib.load(handle)
    except FileNotFoundError:
        raise Unreadable(f"{path}: no such file") from None
    except IsADirectoryError:
        raise Unreadable(f"{path}: is a directory, not a budget file") from None
    except UnicodeDecodeError as error:
        raise Unreadable(f"{path}: not valid UTF-8 ({error.reason})") from None
    except tomllib.TOMLDecodeError as error:
        raise Unreadable(f"{path}: {error}") from None
    except ValueError as error:
        raise Unreadable(f"{path}: {error}") from None
    except OSError as error:
        raise Unreadable(f"{path}: {error.strerror or error}") from None


def label_of(criterion, name, platform):
    return f"{criterion} {name} ({platform})"


# The three fields that name an entry. Read by three functions, and each of them
# guarded the read separately -- two in almost the same words and the third not
# at all, which is how a traceback came to replace a whole verdict. One reader
# now, so a fourth cannot be written without the guard.
IDENTITY_FIELDS = ("criterion", "name", "platform")


def entry_identity(entry):
    """The entry's (criterion, name, platform), or None when it has none.

    None means the budget-file gate has a defect to report and every other
    reader has nothing to say: a verdict over an entry that does not name
    itself is a verdict on nothing.
    """
    if any(not is_text(entry.get(field)) for field in IDENTITY_FIELDS):
        return None
    return tuple(entry[field] for field in IDENTITY_FIELDS)


def rendered(value):
    """`value` for a message, whatever its magnitude.

    `:g` converts to float and raises on an int too large to convert. Bounding
    one factor of a product is not enough -- the clamp on the firing count left
    the wake's own bound unbounded, and two large bounds reach the same raise at
    the same line. A message must be printable for every value a gate can hold,
    so the fallback states the magnitude rather than the digits.
    """
    try:
        return f"{value:g}"
    except OverflowError:
        return f"about 1e{len(str(abs(value))) - 1}"


def entry_label(entry):
    return label_of(entry["criterion"], entry["name"], entry["platform"])


def runner_missing(runner):
    """What a runner block still lacks before it is pinned; empty once it is.

    Pinned means five things recorded. FR-043's own sentence names three of
    them -- the budget file MUST record both machines "by model,
    operating-system version, memory configuration and a durable machine
    identifier" -- so an absent `os_version` or `memory` is a requirement
    unmet, not a convenience missing. `os_floor` does not stand in for
    `os_version`: the floor is what the tier admits, the version is what a
    figure was measured on, and an operating-system update moves a figure
    without touching the floor. The other two are what the gate itself needs:
    a runner label, without which no workflow job can resolve the machine, and
    the display refresh, without which SC-006's stated condition is
    unverifiable on it.

    `os_build`, `storage` and `latency_rig` are recorded in the runner block
    for SC-013 reproducibility and are deliberately NOT tested here, because
    FR-043's list does not name them and this gate enforces the requirement
    rather than a preference. The rig in particular is shared across tiers and
    its absence is a stated limitation of the SC-006 figures, not an unpinned
    runner.

    All five are written when the machine is procured, so until then a runner
    fails the budget-file gate for the same reason five times over, reported
    once.
    """
    missing = []
    if not is_text(runner.get("identity", "")):
        missing.append("no durable identity")
    if not is_text(runner.get("runner_label", "")):
        missing.append("no runner_label")
    if not is_text(runner.get("os_version", "")):
        missing.append("no operating-system version, which FR-043 requires")
    if not is_text(runner.get("memory", "")):
        missing.append("no memory configuration, which FR-043 requires")
    if not is_positive_number(runner.get("display_refresh", 0)):
        missing.append("display_refresh not recorded")
    return missing


def record_defects(label, record, fields):
    """Why a sub-table is not what its schema names; empty when it is.

    A sub-table carries exactly the fields its schema names, well typed. The
    defects are returned rather than blocked so that a caller can also ask
    whether a record is well formed at all, which is what the absolute gate
    asks of a spike exemption before lifting on it.
    """
    if not isinstance(record, dict):
        names = ", ".join(name for name, _, _ in fields)
        return [f"{label} must be a table {{ {names} }}"]
    defects = []
    for name, accepts, description in fields:
        if name not in record:
            defects.append(f"{label} lacks {name}")
        elif not accepts(record[name]):
            defects.append(f"{label}: {name} must be {description}")
    known = {name for name, _, _ in fields}
    for name in record:
        if name not in known:
            defects.append(f"{label} carries an unknown field {name}")
    return defects


def check_record(gate, label, record, fields):
    """A sub-table must carry exactly the fields its schema names, well typed."""
    for message in record_defects(label, record, fields):
        gate.block(message)


def spike_exemption_defects(entry):
    """Why the spike exemption this entry records is not one; empty when it is.

    Beyond its schema, the exemption names the figure the spike measures, as
    the criterion states it, and that is this entry's own figure: an exemption
    recorded on one entry and naming another's is an exemption that has
    extended to another entry in its own record, which it never does.
    """
    label = f"{entry_label(entry)}: spike_exemption"
    record = entry["spike_exemption"]
    defects = record_defects(label, record, SPIKE_EXEMPTION_FIELDS)
    if not defects and record["figure"] != entry["name"]:
        defects.append(
            f"{label} names the figure {record['figure']!r}; this entry's figure "
            f"is {entry['name']!r}, and an exemption never extends to another "
            "entry"
        )
    return defects


def spike_exemption_of(entry):
    """The well-formed spike exemption recorded on this entry, or None.

    A malformed record is not an exemption. The budget-file gate refuses the
    file on it, and the absolute gate does not lift on it: a record that fails
    its own schema cannot be read as saying what it would have to say.
    """
    if "spike_exemption" not in entry or spike_exemption_defects(entry):
        return None
    return entry["spike_exemption"]


def firings_in_window(period):
    """The most firings of a wake with this period, in seconds, that one
    60-minute window can hold: one at the window's opening and one every period
    after it, the last landing on its close. "Any 60-minute window" binds the
    worst one, so this is the count a wake's bound is multiplied by."""
    firings = WINDOW_SECONDS // period
    if not math.isfinite(firings) or firings > sys.maxsize:
        # A period this small is not a schedule. The caller does not merely
        # COMPARE what this returns -- it multiplies it by the wake's bound and
        # formats the product with `:g`, and a big enough int raises there
        # rather than converting. Returning an unbounded int moved the failure
        # from this line into that message, which is the same destroyed verdict
        # one step further on; the clamp keeps the product inside float range,
        # and the cap refuses it either way.
        return sys.maxsize
    return int(firings) + 1


def declared_entries(budgets):
    """The `[[entry]]` blocks, or an empty list when the key is not an array.

    `runners` and `wake` are both guarded on their shape and `entry` was not,
    so writing `[entry]` for `[[entry]]` -- the ordinary TOML slip -- replaced
    the verdict with a traceback. That is the failure `load_budgets` exists to
    prevent, one level further in: an unrun gate is not a pass, and a stack
    trace is not a verdict either. The shape is reported by `check_budget_file`;
    the two measuring readers take the empty list and have nothing to compare.
    """
    entries = budgets.get("entry", [])
    if not isinstance(entries, list):
        return []
    return [entry for entry in entries if isinstance(entry, dict)]


def check_budget_file(budgets, gate):
    """The file must describe a state a gate can be run against."""
    runners = budgets.get("runners", {})
    if not isinstance(runners, dict):
        gate.block("runners must be a table of tiers; a verdict over a misread "
                   "file is a verdict on nothing")
        runners = {}
    if not runners:
        gate.block("no runners declared; every measured figure is reported against one")

    # The tier set is closed for the same reason the entry list is: a file
    # missing a tier cannot report its own omission, and an absent tier was
    # indistinguishable from an unpinned one -- so deleting a [runners.*] table
    # turned every hardware-dependent gate on that tier advisory and let a
    # measurement many times its figure pass. This is stated against TIER_OF
    # rather than against whatever the file happens to declare.
    for tier in sorted(set(TIER_OF.values()) - set(runners)):
        gate.block(
            f"runner {tier} is not declared; the two tiers are closed, and an "
            "absent tier is not an unpinned one -- every hardware-dependent "
            "entry on it would go unreported rather than blocked"
        )

    for tier, runner in runners.items():
        if not isinstance(runner, dict):
            gate.block(f"runner {tier}: must be a table, not "
                       f"{type(runner).__name__}")
            continue
        model = runner.get("model", "unnamed")
        if not isinstance(runner.get("runner_label", ""), str):
            gate.block(f"runner {tier} ({model}): runner_label must be a string")
        hz = runner.get("display_refresh", 0)
        if not is_number(hz) or hz < 0:
            gate.block(
                f"runner {tier} ({model}): display_refresh must be a number of Hz, "
                "0 until recorded"
            )
        missing = runner_missing(runner)
        if missing:
            gate.block(
                f"runner {tier} ({model}) {UNPINNED}: {', '.join(missing)}; until "
                "it is procured and pinned no hardware-dependent figure is "
                "reproducible and no workflow job can resolve it",
                tag=UNPINNED_TAG,
            )

    declared = budgets.get("entry", [])
    if not isinstance(declared, list):
        gate.block("entry must be an array of tables, written [[entry]] rather than "
                   "[entry]; a verdict over a misread file is a verdict on nothing")
    elif any(not isinstance(entry, dict) for entry in declared):
        gate.block("every entry must be a table; a verdict over a misread file is a "
                   "verdict on nothing")
    entries = declared_entries(budgets)
    if not entries:
        gate.block("no entries declared")

    seen = set()
    for entry in entries:
        key = entry_identity(entry)
        if key is None:
            gate.block(f"an entry without a criterion, a name and a platform: {entry}")
            continue

        label = entry_label(entry)
        if key in seen:
            gate.block(f"{label}: declared twice")
        seen.add(key)

        if not is_text(entry.get("status")) or entry["status"] not in {"ratified", "provisional"}:
            gate.block(f"{label}: status must be ratified or provisional")

        for old in RETIRED_FIELDS:
            if old in entry:
                gate.block(
                    f"{label}: {old} is a retired field; the schema is figure and "
                    "baseline in a stated unit"
                )

        # The unit is required and is the criterion's. Without one a figure is
        # not a number a measurement can be compared against; in another
        # criterion's unit it is not the entry this criterion states.
        criterion = entry["criterion"]
        stated = UNIT_OF.get(criterion)
        unit = entry.get("unit")
        units = ", ".join(sorted(UNITS))
        if stated is None:
            gate.block(f"{label}: {criterion} states no budget entry")
        elif key not in CLOSED_LIST:
            gate.block(
                f"{label}: no criterion states this entry; the list of eighteen is "
                "closed, and adding one is an amendment to the specification"
            )
        if unit is None:
            gate.block(f"{label}: no unit; one of {units} is required")
        elif not is_text(unit) or unit not in UNITS:
            gate.block(f"{label}: unit {unit!r} is not one of {units}")
        elif stated is not None and unit != stated:
            gate.block(
                f"{label}: unit {unit}; {criterion} states its figures in {stated}"
            )
        unit_text = unit if is_text(unit) and unit in UNITS else "(no unit)"

        # A ratified figure is one a recorded founder decision set, so a
        # ratified entry names that decision; a provisional entry names none,
        # because no decision has set its figure, and gains one in the change
        # that ratifies it. Either way what is named is a citation, not a
        # description.
        decision = entry.get("founder_decision")
        if decision is None:
            if entry.get("status") == "ratified":
                gate.block(
                    f"{label}: ratified and names no founder decision; a ratified "
                    "figure is one a recorded decision set, cited as decisions/NNNN"
                )
        elif not is_decision_citation(decision):
            gate.block(
                f"{label}: founder_decision {decision!r} does not cite a recorded "
                "decision; the register's citation form is decisions/NNNN"
            )

        # SC-004 declares its cross-check margin on every entry, a percentage
        # under the tolerance cap. An undeclared margin is zero rather than
        # unbounded, so omitting it does not switch the cross-check off; it is
        # refused here because the preamble requires the declaration.
        margin = entry.get("cross_check_margin")
        if criterion == "SC-004":
            if margin is None:
                gate.block(
                    f"{label}: no cross_check_margin; SC-004 declares one for its "
                    "whole-machine cross-check, 0.0 until a measurement writes it, "
                    "and an undeclared margin is zero rather than unbounded"
                )
            elif not is_number(margin):
                gate.block(f"{label}: cross_check_margin must be a number")
            elif margin < 0:
                gate.block(f"{label}: negative cross_check_margin")
            elif margin > MAX_CROSS_CHECK_MARGIN_PCT:
                gate.block(
                    f"{label}: cross_check_margin {margin}% exceeds the "
                    f"{MAX_CROSS_CHECK_MARGIN_PCT}% cap"
                )
        elif margin is not None:
            gate.block(
                f"{label}: cross_check_margin is declared only on SC-004, whose "
                "whole-machine cross-check it bounds"
            )

        if "spike_exemption" in entry:
            for message in spike_exemption_defects(entry):
                gate.block(message)
        if "baseline_reset" in entry:
            check_record(
                gate, f"{label}: baseline_reset", entry["baseline_reset"],
                BASELINE_RESET_FIELDS,
            )

        figure = entry.get("figure")
        baseline = entry.get("baseline")
        if figure is None or baseline is None:
            gate.block(f"{label}: missing figure or baseline")
            continue
        if not is_number(figure) or not is_number(baseline):
            gate.block(f"{label}: figure and baseline must be numbers")
            continue
        # A negative baseline is not a smaller baseline; it is a disabled
        # regression gate. `run_gates` compares only when `baseline > 0`,
        # because 0.0 means "not yet measured" and is documented as inert. A
        # negative value is not a documented state and was accepted in silence,
        # which on a hardware-dependent entry -- whose absolute gate is already
        # advisory while the runner is unpinned -- left the entry with no
        # blocking gate at all. This is the same reading the tolerance clause
        # takes: the opposite one lets an entry disable its own gate by writing
        # a number nobody would notice.
        if baseline < 0:
            gate.block(
                f"{label}: baseline {baseline} is negative; a baseline is a "
                "measured figure, and a negative one disables the regression "
                "gate rather than tightening it"
            )
            continue
        if figure <= 0:
            gate.block(
                f"{label}: figure {figure} is not positive; a budget that is "
                "zero or negative is met by nothing and refuses everything"
            )
            continue

        # A provisional figure binds a baseline exactly as a ratified one does.
        # A provisional figure is a ceiling for as long as it stands, which is
        # the whole of its function. Where a reset is recorded, the reset is
        # what placed the baseline there, and the failure says so.
        if baseline > figure:
            if "baseline_reset" in entry:
                gate.block(
                    f"{label}: baseline_reset leaves the baseline at {baseline} "
                    f"{unit_text}, above the stated figure {figure} {unit_text}; a "
                    "reset may never place a baseline above it"
                )
            else:
                gate.block(
                    f"{label}: baseline {baseline} {unit_text} is above the stated "
                    f"figure {figure} {unit_text}; a reset may never place a "
                    "baseline above it"
                )

        tolerance = entry.get("tolerance_pct", 0.0)
        if not is_number(tolerance):
            gate.block(f"{label}: tolerance_pct must be a number")
            continue
        if tolerance > MAX_TOLERANCE_PCT:
            gate.block(
                f"{label}: tolerance {tolerance}% exceeds the {MAX_TOLERANCE_PCT}% cap"
            )
        if tolerance < 0:
            gate.block(f"{label}: negative tolerance")

    # The closed list, compared against the preamble's statement of it rather
    # than read off the file: a file missing an entry cannot report its own
    # omission, and iterating the entries declared is exactly asking it to.
    for criterion, name, platform in sorted(CLOSED_LIST - seen):
        gate.block(
            f"{label_of(criterion, name, platform)}: stated by the preamble and "
            "absent from the file"
        )

    # SC-005's wake enumeration. Absent, it fails: every scheduled wake on the
    # idle path is enumerated here, and a file with none says so with
    # `wake = []`, a statement rather than an omission. Each wake declared
    # carries every field its schema names and a bound inside the per-wake
    # cap, and the bounds together stay inside the window cap at the count the
    # worst 60-minute window can hold of each.
    wakes = budgets.get("wake")
    if wakes is None:
        gate.block(
            "no wake enumeration; SC-005 requires every scheduled wake on the idle "
            "path enumerated in this file, and a file with none writes "
            "`wake = []`, a statement rather than an omission"
        )
    elif not isinstance(wakes, list):
        gate.block("wake must be an array of tables, `wake = []` when empty")
    else:
        names = set()
        contributions = []
        for position, wake in enumerate(wakes, start=1):
            name = wake.get("name") if isinstance(wake, dict) else None
            label = f"wake {name!r}" if is_text(name) else f"wake {position}"
            defects = record_defects(label, wake, WAKE_FIELDS)
            for message in defects:
                gate.block(message)
            if is_text(name):
                if name in names:
                    gate.block(f"{label}: enumerated twice")
                names.add(name)
            if defects:
                # A wake the schema cannot read has no bound to sum; the
                # failures above report it.
                continue
            bound = wake["processor_time_bound"]
            if bound > WAKE_BOUND_CAP_MS:
                gate.block(
                    f"{label}: processor_time_bound {rendered(bound)} ms is above the "
                    f"{WAKE_BOUND_CAP_MS} ms SC-005 caps a wake at"
                )
            contributions.append((label, bound, firings_in_window(wake["period"])))
        total = sum(bound * firings for _, bound, firings in contributions)
        if total > WAKES_WINDOW_CAP_MS:
            breakdown = ", ".join(
                f"{label} {rendered(bound)} ms x {firings}"
                for label, bound, firings in contributions
            )
            gate.block(
                f"the enumerated wakes' bounds sum to {rendered(total)} ms of processor "
                f"time in a 60-minute window ({breakdown}), above the "
                f"{WAKES_WINDOW_CAP_MS} ms SC-005 allows; work that needs more is "
                "a change to this file stating its cost, not an exception"
            )


def defer_unpinned_runners(gate):
    """Move the unpinned-runner failures to advisory, and only those.

    This is --allow-unpinned-runners: a stated deferral until the machines
    Q-E9a names are procured. Every other budget-file failure keeps blocking,
    which is what makes it a deferral rather than a way to turn the gate off.
    """
    kept, kept_tags = [], []
    for message, tag in zip(gate.blocking, gate.tags):
        if tag == UNPINNED_TAG:
            gate.advisory.append(message)
        else:
            kept.append(message)
            kept_tags.append(tag)
    gate.blocking, gate.tags = kept, kept_tags


# Where each tier's packaging build publishes its installer artefact, relative
# to the repository root, and the kind of artefact it is: the .msi WiX builds
# from packaging/windows/evreos.wxs, and the .pkg productbuild assembles from
# packaging/macos/Distribution.xml. The directory is this gate's contract with
# those builds -- under cargo's target/, beside the release binary and cleaned
# with it -- so that "the installer artefact CI publishes", which SC-001's
# condition names, is read from one place the build and the gate agree on.
# Exactly one artefact is served to everyone: the packaging decision research
# §10.3 records, which rests on FR-033 -- attribution for a partner referral
# comes only from a code the member scans or types and is never inferred from
# the installation, so there is no per-partner and no per-campaign build. So
# exactly one is expected: none is the state before that platform's installer
# is built.
INSTALLER_ARTEFACT = {
    "windows": ("target/packaging/windows", ".msi"),
    "macos": ("target/packaging/macos", ".pkg"),
}

# What measure_download_size() reports: the platform the artefact was built
# for, its size in MB, and -- where nothing was measured, megabytes being None
# -- the reason.
DownloadSize = collections.namedtuple("DownloadSize", "platform megabytes reason")


def host_platform(system=sys.platform):
    """The tier this host builds an installer artefact for: windows on a
    Windows host, macos on a macOS host, None on any other. The hosted Linux
    runner the build job runs on is a host of no tier, Linux being the
    deferred platform: it builds a release binary, but that is a Linux ELF that
    meets neither download-size entry's condition, and it is not read."""
    return {"win32": "windows", "darwin": "macos"}.get(system)


def installer_artefacts(platform, repo=REPO):
    """Every file of the platform's artefact kind where its build publishes."""
    directory, suffix = INSTALLER_ARTEFACT[platform]
    return sorted((repo / directory).glob(f"*{suffix}"))


def measure_download_size(host, repo=REPO):
    """The size of the installer artefact this host builds, in MB, SC-001's
    unit, declared for the platform it was built for.

    The declaration is what keeps one platform's artefact from satisfying
    another platform's entry: the figure is keyed by the platform returned
    here, and run_gates compares it against that entry alone. A host of no
    tier measures nothing, rather than measuring what it can build and calling
    it a figure for a platform it is not. On a tier's host with no artefact
    the installer is not built yet, and the reason says so; with more than one
    the gate does not pick, since exactly one artefact is served to everyone
    -- FR-033's consequence, recorded at research §10.3 -- and two is a state
    to report rather than resolve.
    """
    if host is None:
        return DownloadSize(None, None, "this host builds no tier's installer artefact")
    directory, suffix = INSTALLER_ARTEFACT[host]
    artefacts = installer_artefacts(host, repo)
    if not artefacts:
        return DownloadSize(
            host, None,
            f"no {suffix} artefact under {directory}; the {host} installer is not "
            "built yet",
        )
    if len(artefacts) > 1:
        names = ", ".join(path.name for path in artefacts)
        return DownloadSize(
            host, None,
            f"{len(artefacts)} {suffix} artefacts under {directory} ({names}) where "
            "exactly one is served to everyone",
        )
    return DownloadSize(host, artefacts[0].stat().st_size / (1024 * 1024), None)


def run_gates(budgets, measurements, host=None):
    """The absolute and regression gates over every entry, and what is unmeasured.

    `measurements` is keyed on (criterion, name, platform), an entry's whole
    identity, so a figure for one platform is compared against that platform's
    entry and no other. `host` is the tier this run measures for, as
    host_platform() reports it, None on a host of no tier; it is what an entry
    no measurement covers is explained against, since SC-001's figures are
    measured on a host of the entry's platform -- the download size where the
    artefact is built, the installed footprint where it is installed. The
    unmeasured list carries (label, reason, blocking).
    """
    absolute = Gate("absolute")
    regression = Gate("regression")
    unmeasured = []

    runners = budgets.get("runners", {})
    if not isinstance(runners, dict):
        runners = {}
    # The same guard check_budget_file carries. Without it a misread file died
    # here before that gate's verdict was ever printed, so the guard's whole
    # stated purpose -- that a verdict over a misread file is a verdict on
    # nothing -- was not delivered: the traceback replaced the message.
    pinned = {tier: not runner_missing(runner)
              for tier, runner in runners.items() if isinstance(runner, dict)}

    for entry in declared_entries(budgets):
        identity = entry_identity(entry)
        if identity is None:
            # The budget-file gate reports it. Reading past it here replaced
            # the whole run's output with a traceback -- including the message
            # that gate had already produced about this very entry -- which is
            # the failure the comment above this loop describes, in the loop
            # below it.
            continue
        label = entry_label(entry)
        figure = entry.get("figure")
        baseline = entry.get("baseline")
        unit = entry.get("unit")
        if not is_number(figure) or not is_number(baseline):
            # No figure to compare. The budget-file gate reports the entry;
            # a verdict here would be a verdict on nothing.
            continue
        if unit != UNIT_OF.get(entry["criterion"]):
            # A measurement arrives in the unit the criterion states. Against a
            # figure in any other unit the comparison is between two different
            # quantities, so none is made; the budget-file gate reports it.
            continue

        platform = identity[2]
        measured = measurements.get(identity)
        if measured is None:
            # An unmeasured entry is not a pass. The one honest exception is a
            # hardware-dependent entry whose tier has no pinned runner: there is
            # no machine to measure it on, which the budget-file gate already
            # reports. Anything else unmeasured means a measurement that should
            # exist does not, and a gate that passes it is a gate that certifies
            # a number nobody produced.
            hardware = entry["criterion"] in HARDWARE_DEPENDENT
            tier = TIER_OF.get(platform)
            if hardware and tier is None:
                # A platform TIER_OF does not map. check_budget_file refuses
                # such an entry already, so this is only ever a second verdict
                # on an already-refused file -- but it must not name a tier
                # called "None".
                unmeasured.append(
                    (f"{label}: platform {platform!r} maps to no tier",
                     "no tier for this platform", True)
                )
            elif hardware and tier not in pinned:
                # Not "unpinned": the file declares no usable runner for this
                # tier at all, so there is nothing to defer to. The measured
                # branch below draws the same distinction; drawing it in one
                # place and not the other is how an absent tier stayed
                # non-blocking here after it began blocking there.
                unmeasured.append(
                    (f"{label}: {tier} is not declared as a usable runner in "
                     "this file", "no runner declared for this tier", True)
                )
            elif hardware and not pinned.get(tier, False):
                unmeasured.append((label, "no pinned runner for this tier", False))
            elif not hardware and platform != host:
                # SC-001's figures are measured on a host of the entry's
                # platform -- the download size where the artefact is built,
                # the installed footprint where it is installed -- and this
                # host is another platform's, or no tier's. The entry is
                # unmeasured here for that reason, and it still blocks:
                # nothing this run can see says it was measured anywhere, and
                # another platform's artefact is not a measurement of it.
                if host is None:
                    this = "this host builds no tier's artefact"
                else:
                    this = f"this is the {host} host"
                unmeasured.append((
                    label,
                    f"a {platform} figure is measured on a {platform} host; {this}",
                    True,
                ))
            else:
                unmeasured.append((label, "no measurement was produced", True))
            continue

        tolerance = entry.get("tolerance_pct", 0.0)

        hardware = entry["criterion"] in HARDWARE_DEPENDENT
        tier = TIER_OF.get(entry["platform"])
        # An ABSENT tier is not an unpinned one. Unpinned is a stated,
        # deferrable state -- the machine is not bought yet -- and a breach on
        # it is advisory by design. A tier the file does not declare at all is
        # a file defect, and treating the two alike let a measurement many
        # times its figure pass as advisory. The file gate refuses such a file,
        # but a gate that also reports the breach correctly is one that does
        # not depend on another gate running first.
        # `tier in pinned`, not `tier in runners`: a tier declared as
        # something other than a table is kept out of `pinned` by the guard
        # above, and treating the bare key as a declaration let its breach fall
        # to the unpinned branch and read as advisory.
        declared = tier in pinned
        runner_pinned = pinned.get(tier, False)
        exemption = spike_exemption_of(entry)

        if measured > figure:
            message = f"{label}: measured {measured:.3f} {unit} exceeds {figure} {unit}"
            if exemption is not None:
                # The one thing a spike exemption lifts: this entry's absolute
                # gate, read off this entry. The breach is still reported, and
                # the release job refuses the build for as long as the
                # exemption is recorded.
                absolute.advise(
                    f"{message} (exempt: the spike in pull request "
                    f"#{exemption['pull_request']} is establishing this figure, "
                    "and a build carrying the exemption is not released or tagged)"
                )
            elif hardware and tier is None:
                # The mirror of the unmeasured branch. Naming a tier called
                # "None" was fixed there and not here, which is the very
                # asymmetry the comment beside that fix warns against.
                absolute.block(
                    f"{message}; platform {entry['platform']!r} maps to no tier"
                )
            elif hardware and not declared:
                absolute.block(
                    f"{message}; {tier} is not declared in this file, so there "
                    "is no runner to defer to -- an absent tier is a file "
                    "defect, not a machine waiting to be bought"
                )
            elif hardware and not runner_pinned:
                absolute.advise(f"{message} (advisory: {tier} runner not pinned)")
            else:
                absolute.block(message)

        # The regression gate compares one machine against itself, so it blocks
        # regardless of whether the runner has been named.
        if not is_number(tolerance):
            # The budget-file gate reports it, at the clause that guards this
            # same field. Reading past it here destroyed the whole run's output,
            # including that verdict -- the failure the identity guard above was
            # written to end, on the next field down. The guard sits HERE and
            # not at the top of the loop, so the absolute gate above still
            # reports its breach: a tolerance says nothing about a ceiling.
            continue
        allowed = baseline * (1 + tolerance / 100.0)
        if baseline > 0 and measured > allowed:
            regression.block(
                f"{label}: measured {measured:.3f} {unit} is worse than baseline "
                f"{baseline} {unit} by more than the declared {tolerance}%"
            )

    return absolute, regression, unmeasured


def unretired_exemptions(budgets):
    """Every spike exemption the file records, as (entry label, record).

    A recorded exemption is an unretired one: retiring it is deleting it, in
    the change that lands the figure it was establishing. A malformed record
    counts, because it is still a recorded exemption and refusing is the safe
    direction -- the budget-file gate has refused the file on it already, and
    a release that read past it would be a release on a technicality.
    """
    found = []
    for entry in declared_entries(budgets):
        if "spike_exemption" not in entry:
            continue
        if entry_identity(entry) is not None:
            label = entry_label(entry)
        else:
            label = f"an entry without a criterion, a name and a platform: {entry}"
        found.append((label, entry["spike_exemption"]))
    return found


def refuse_exemptions(budgets):
    """The release job's refusal: non-zero on any recorded spike exemption.

    The preamble: a build produced while an exemption is unretired MUST NOT be
    released or tagged, and the release job refuses an artefact built from a
    commit whose budget file records one. This runs nothing else, because the
    build job the release job depends on has already run the gates.
    """
    exemptions = unretired_exemptions(budgets)
    for label, record in exemptions:
        number = record.get("pull_request") if isinstance(record, dict) else None
        if is_pull_request_number(number):
            carrier = f"pull request #{number}"
        else:
            carrier = "a malformed record, which is still a recorded one"
        print(
            f"  FAIL     [release] {label}: unretired spike exemption ({carrier}); "
            "a build carrying it is not released or tagged",
            file=sys.stderr,
        )
    if exemptions:
        count = len(exemptions)
        noun = "exemption" if count == 1 else "exemptions"
        print(
            f"\nRelease REFUSED: the budget file records {count} unretired spike "
            f"{noun}.",
            file=sys.stderr,
        )
        return 1

    print("The budget file records no unretired spike exemption; the release is not "
          "refused on that count.")
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--budgets", default=str(REPO / "budgets.toml"))
    parser.add_argument(
        "--allow-unmeasured",
        action="store_true",
        help="do not fail on an entry whose measurement does not exist yet; for "
        "use before the harness that produces it is built",
    )
    parser.add_argument(
        "--allow-unpinned-runners",
        action="store_true",
        help="do not fail the budget-file gate on a runner that is not pinned; "
        "for use before the reference machines are procured",
    )
    parser.add_argument(
        "--refuse-exemptions",
        action="store_true",
        help="fail when the budget file records an unretired spike exemption, and "
        "run nothing else; the release job's refusal, since a build produced "
        "while one stands is not released or tagged",
    )
    args = parser.parse_args()

    try:
        budgets = load_budgets(args.budgets)
    except Unreadable as error:
        # 2, not 1: the check reached no verdict rather than finding a breach.
        # An unrun gate is not a pass either way, and the workflow fails on both.
        print(f"Cannot read the budget file: {error}", file=sys.stderr)
        return 2

    if args.refuse_exemptions:
        return refuse_exemptions(budgets)

    file_gate = Gate("budget file")
    check_budget_file(budgets, file_gate)

    if args.allow_unpinned_runners:
        defer_unpinned_runners(file_gate)

    # The one measurement this script makes: the installer artefact this host
    # builds, keyed for the platform it declares, so that it is compared against
    # that platform's entry and no other.
    host = host_platform()
    measurements = {}
    download = measure_download_size(host)
    if download.megabytes is not None:
        key = ("SC-001", "download size", download.platform)
        measurements[key] = download.megabytes

    absolute, regression, unmeasured = run_gates(budgets, measurements, host)

    for gate in (file_gate, regression, absolute):
        for message in gate.advisory:
            print(f"  advisory [{gate.name}] {message}")
        for message in gate.blocking:
            print(f"  FAIL     [{gate.name}] {message}", file=sys.stderr)

    blocking_unmeasured = [
        (label, reason) for label, reason, blocking in unmeasured if blocking
    ]
    if unmeasured:
        print(f"  unmeasured on this machine: {len(unmeasured)} entries")
        for label, reason, blocking in unmeasured:
            deferred = ""
            if blocking and args.allow_unmeasured:
                deferred = "; deferred by --allow-unmeasured"
            print(f"    - {label}  ({reason}{deferred})")

    if blocking_unmeasured and not args.allow_unmeasured:
        for label, reason in blocking_unmeasured:
            print(
                f"  FAIL     [budget file] {label}: {reason}; an unmeasured entry "
                "is not a pass",
                file=sys.stderr,
            )
        # Through Gate.block, never by appending to `blocking` directly: the
        # tags list must stay the same length, or defer_unpinned_runners' zip
        # truncates to the shorter one and the surplus failures land in neither
        # blocking nor advisory. Nothing was lost live -- this ran after the
        # only deferral -- but it was one reordering away from silent loss, in
        # the function rewritten to be trustworthy.
        for label, _ in blocking_unmeasured:
            file_gate.block(label)

    if download.megabytes is not None:
        print(
            f"  measured: download size ({download.platform}) "
            f"{download.megabytes:.3f} MB"
        )
    else:
        print(f"  measured: nothing on this {sys.platform} host; {download.reason}")

    failed = [g.name for g in (file_gate, regression, absolute) if g.failed]
    if failed:
        print(f"\nBudget gates FAILED: {', '.join(failed)}", file=sys.stderr)
        return 1

    print("\nBudget gates passed.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
