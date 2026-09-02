Closes #N

<!--
One pull request per issue, and the issue is opened first if none exists
(Principle I; the repository rules file, Workflow section). The commit-hygiene
job reads this body for attribution only and never for this link, so the link
rests on review.
-->

## What changes, and why

<!--
The reviewer reads the diff; this says what the diff is for. State the reason,
not only the change.
-->

## Cost against `budgets.toml`

<!--
Every pull request that adds or changes a feature, and every pull request that
changes any quantity a budget measures — a new dependency, a bundled asset, a
build or packaging change — states its byte and millisecond cost against
budgets.toml, whether or not it changes behaviour a member can observe
(Principle II; FR-043). Keep exactly one of the two blocks below and delete the
other. A stated cost is not a justification: under FR-043 a change whose cost
this pull request cannot justify is refused over a green gate, by a founder
decision recorded here.
-->

### The change moves these entries

One row per affected entry, named as `scripts/check-budgets.py` names it —
criterion, name, platform — and stated in the unit the budget file records for
that entry, because the absolute and regression gates compare nothing else:

- megabytes for SC-001's download size and installed footprint, and for
  SC-004's ten-tab memory;
- milliseconds for SC-002's warm start and cold start, and for SC-006's tab
  switch and address-field keystroke;
- percent of one core for SC-005's 60-minute window and its wake-free 1-second
  sample. SC-005 states both figures as 0.5% of one core; the 18 s and 5 ms it
  also names are that percentage at each window's scale, not the figure. The
  processor-time equivalent belongs in `Measured how` if it helps the reader,
  never in the figure columns, where a millisecond number is one the gates
  never compare.

Principle II's "byte and millisecond cost" is met in whichever of these units
the entry carries. `Before` is measured without the change, at the merge base;
`After` with it, at the head; `Cost` is the difference, signed. A
hardware-dependent entry (SC-002, SC-004, SC-005, SC-006) is measured on the
pinned runner for its platform and on no other machine. SC-001's entries are
not hardware-dependent and are measured from build output: download size from
the artefact CI publishes, installed footprint from the disk delta after first
run completes, as `budgets.toml` states. Where the tier's runner is not yet
pinned, write `unmeasured` and say why: a figure measured on an unnamed machine
is not reproducible under SC-013, and the gate reports such an entry as
unmeasured rather than inventing a number. This table does the same.

| Entry (criterion, name, platform) | Before | After | Cost | Measured how |
| --- | --- | --- | --- | --- |
| SC-001 download size (windows) | | | | |

The requirement this cost serves:

### The change moves no quantity a budget measures

Because:

<!--
A reason names what the change touches and why none of it reaches a shipped
artefact or its run time — for example: documentation and CI configuration
only; a check under scripts/ that runs in CI and ships in no binary. "No
behaviour change" is not a reason, since FR-043 covers exactly the change that
adds bytes or milliseconds without adding behaviour.
-->

## Review-round record

<!--
The Development Workflow section of the constitution makes this pull request
mergeable only when its most recent review round is recorded green, as a
comment on this pull request, against the exact diff that would merge —
`git diff <base>...<head>`, three dots, against the merge base. This section
points at that comment and is not the record: the body can be edited, which is
why the record lives in a comment and why nothing written here counts as a
round. A reader follows the link and checks that the three identifiers below
match the pull request as it stands; a pointer naming a superseded head, base
branch or merge base is stale, and the round it points at no longer counts.
Update this section after every round. Until the first round is recorded,
write `none yet` in place of the link. `main` is prefilled because every change
reaches `main` through a pull request; replace it if this one targets another
branch.
-->

- Record: _link to the comment that records the round_
- Head SHA: _full SHA_
- Base branch: `main`
- Merge-base SHA: _full SHA_

<!--
Taken on the head commit that comment covers: `git rev-parse HEAD` and
`git merge-base origin/main HEAD`.

The commit-hygiene job checks this body for attribution under Principle I. Keep
it free of everything the Authorship section of the repository rules file
forbids: no session, run or conversation identifier, and no co-authorship
trailer of any kind.
-->
