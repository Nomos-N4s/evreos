<!--
Sync Impact Report
Version change: 1.0.0 → 1.1.0

Ratification 1.0.0 (2026-08-29):
  Modified principles: none (scaffold replaced; no prior principles existed)
  Added sections: Core Principles I–X, Permanent Prohibitions, Development Workflow,
    Governance
  Removed sections: none (all template placeholders resolved)

Amendment 1.0.0 → 1.1.0 (2026-08-30), MINOR — guidance materially expanded:
  - Classified MINOR rather than MAJOR deliberately. MAJOR is scoped to a principle being
    removed or redefined in a backward-incompatible way, or to any removal from
    Permanent Prohibitions; this amendment does neither. What changed is the Development
    Workflow section and, in consequence, Governance's Versioning policy. No Core
    Principle and no Permanent Prohibition is touched, and both sections are tightened
    or extended rather than relaxed, so PATCH does not fit either. The Versioning
    policy is extended below to name that case rather than leaving the classification
    to be inferred.
  - Modified sections: Development Workflow — the review bullet gains the review
    dimensions and the recording form; the merge gate is new; the ADR-0001 bullet moves
    to the past tense. Governance — the Versioning policy gains the tightening case.
  - Removed: the "or to the founder" destination for a round's outcome.
  - Development Workflow gains a merge gate: a pull request is not mergeable until a
    review round is recorded green against the exact diff that would merge, with the
    record on the pull request naming head and base SHAs and every finding raised with
    its severity and disposition — not counts.
  - Recording is fixed in time and form: a new comment, before the next push, never an
    edit to an earlier record. A record kept only in the pull request body is a mutable
    field the author can silently rewrite, which would let a blocked round be edited
    green after the fact.
  - The obligation to record every round's outcome is retained and stays on the review
    bullet; what is withdrawn is its "or to the founder" destination, because a record
    only the founder can see makes the gate unverifiable by anyone reading the pull
    request. Every round, green or not, is now recorded on the pull request itself.
  - The override must be stated before the merge, not merely stated. The repository rules
    file said "never an accident of timing"; deferring to this document dropped the phrase,
    and without it a merge over confirmed blockers could be regularised by a comment
    posted afterwards. The issue this amendment closes records the adjacent failure:
    two pull requests merged with no round recorded and no override stated at all.
  - The review dimensions — correctness, security, consistency, rules compliance — move
    here from the repository rules file, which was the only place that stated them.
    Deferring to this document would otherwise have deleted them. "Consistency" is spelled
    out rather than narrowed to "internal": the dominant defect across five rounds has been
    a statement in a change contradicting untouched text elsewhere, which a reviewer
    reading "internal consistency" is licensed to skip. The fourth dimension keeps its
    original scope deliberately: `CLAUDE.md` retains obligations that
    rest on review alone — the co-authorship trailer ban, the branch prefixes, the Spec
    Kit directory rule — so narrowing it to this document would have orphaned them.
  - Reason for amending here rather than in the repository rules file: this document
    supersedes that one, so a stricter rule placed there was void wherever the two
    disagreed. The review and merge-gate rules now live here; the rules file defers to
    this document for them. Its remaining workflow bullets — the prohibition on direct
    pushes to `main`, issue linking, commit format, atomicity — stay there as operational
    reminders, as do the branch prefixes and the Spec Kit directory rule in their own
    sections.
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ reads the constitution at runtime; no change needed
  - .specify/templates/spec-template.md ✅ no change needed
  - .specify/templates/tasks-template.md ✅ no change needed
  - .specify/templates/checklist-template.md ✅ no change needed
Follow-up TODOs (status as of 1.1.0):
  - DONE — the commit-msg hook and server-side commit-hygiene CI job required by
    Principle I landed in `.githooks/commit-msg` and
    `.github/workflows/commit-hygiene.yml`.
  - DONE — ADR-0001, the engine decision the Development Workflow section requires, is
    recorded at `docs/adr/0001-rendering-engine.md`.
  - CLOSED by `decisions/0002` — nine merge commits on `main` are authored with the
    founder's GitHub account display name, `Carlos Pinto`, rather than
    `xcoder-es <capintobe@gmail.com>`. The founder has accepted that name as an author
    and a committer alongside the canonical one: the two spellings denote one person and
    are bound by the one address Principle I names, so sole authorship is unaffected.
    The account is not renamed, and the remedy this entry previously proposed is not
    carried out. `scripts/check-commit-hygiene.py` cites that record. What remains open
    is unrelated to the name: the check runs over `origin/<base>..HEAD`, which never
    contains the merge commit being created, so only branch protection gates what lands.
  - OPEN — the merge gate above is only partly mechanical. A check can verify that a
    record exists, names the current head and merge base, and reports no confirmed
    finding; it cannot verify that the round was adversarial, that the findings list is
    complete, or that a dismissal came from someone other than the author. The
    preamble's CI requirement is scoped to principles and Development Workflow is not
    one, so this is a gap worth closing rather than one the preamble compels. Tracked as
    issue #27, which also notes the record has no defined syntax to parse.
  - OPEN — the budget file and CI gates required by Principle II are not yet present.
    They are ratified here as a requirement, not described as current state, and land
    with milestone M0.
-->

# Evreos Constitution

Evreos is a featherweight, privacy-first browser for Windows, macOS and Linux that doubles
as the shell for the Apivo super-app. This constitution makes two things structural rather
than habitual: the performance discipline that is the product's identity, and the trust
posture that is its licence to carry money surfaces. Every principle below is enforceable,
and where a principle can be measured, it MUST be gated in CI rather than left to
intent.

## Core Principles

### I. Sole Authorship and Signed Commits (NON-NEGOTIABLE)

Every commit MUST be authored by the founder, `xcoder-es <capintobe@gmail.com>`, and
MUST be signed. Nothing anywhere in this repository may attribute authorship or
assistance to an AI or generator tool: no attribution trailers, and no such attribution in
commit messages, pull request titles or bodies, issues, comments, code comments,
documentation, or generated files. Naming a tool the project integrates with, or recording
that a review ran, is description rather than attribution and is permitted. Commit messages
MUST follow Conventional Commits and reference the issue they serve. One pull request per
issue; nothing lands directly on `main`. Authorship and the absence of AI attribution MUST
be enforced mechanically by the same commit-msg hook and server-side commit-hygiene CI
job the Apivo repository uses.

**Rationale**: Authorship is a legal and reputational fact about the product, not a
formatting preference. A rule enforced only by discipline is a rule that fails silently
under time pressure, so the enforcement is mechanical.

### II. Featherweight Is Law

Hard budgets — download size, installed size, cold start, shell memory overhead, idle CPU
and chrome input latency — MUST live in one budget file in this repository and MUST be
enforced by CI gates that fail the build on regression. Budgets move only by recorded
founder decision, and the default direction is tighter. Every feature MUST state its byte
and millisecond cost in its pull request; a feature that cannot justify its cost is not
added. Preferring deletion is a legitimate merge argument.

**Rationale**: "Fast" decays into a slogan the moment it stops being measured. Budgets in
version control with CI teeth are the difference between a performance claim and a
performance product.

### III. Rust Core, No Bundled Engine

The shell MUST be stable Rust, with no nightly features on the release path. Electron, CEF
and any bundled Chromium are permanently rejected — they are the thing Evreos exists not to
be. Rendering MUST go through an `Engine` interface defined by the shell as the consumer,
with the system-webview implementation as the default and a headless test implementation
kept working from day one, so the seam is proved by a second implementation rather than
asserted. This leaves room for a pure Rust engine as an experimental third backend when one
becomes daily-drivable.

**Rationale**: Bundling an engine forfeits the size, memory and startup budgets in a single
decision, and inherits a patch treadmill the operating system would otherwise carry. A seam
with only one implementation is an assumption; a second implementation makes it a fact.

### IV. Browser First, Super-App Second

Signed out, with every Apivo surface ignored, Evreos MUST be a genuinely good private
browser — that is the default experience, not a degraded mode. Apivo surfaces MUST be
discoverable, opt-in and dismissible. Nothing is ever injected into a web page without an
explicit user action for that occasion. Affiliate attribution MUST never be attached
silently, and MUST never be claimed for a purchase the member's click did not lead to; the
failure that ended the Honey extension's trust is the canonical counter-example. A
violation of this principle is a release blocker, not a bug.

**Rationale**: The consumer proposition has to stand on its own or the distribution
strategy is a funnel wearing a browser costume. Trust lost this way is not recoverable by
apology, and the counter-example is recent enough to be instructive.

### V. All Money Is Server-Side

The browser MUST render ledger-derived state and request actions; it MUST NOT compute a
balance, MUST NOT build an affiliate deeplink, and MUST NOT hold money logic. The cashback
invariants — double entry, evidence, approver-gated payouts, exactly-once — live behind the
Apivo API and MUST NOT be re-implemented, approximated, or cached-as-truth in the client.
Amounts are displayed exactly as the ledger reports them, including pending, confirmed,
declined and reversed states.

**Rationale**: A client that computes money will eventually disagree with the ledger, and
the member will believe the client. One source of truth is the only arrangement that stays
correct across versions, offline states and partial updates.

### VI. Privacy by Default, GDPR by Construction

Browsing MUST work fully signed out. Telemetry and crash reporting MUST be opt-in,
aggregate and EU-hosted. Browsing history MUST NOT leave the machine. Fingerprinting and
install-referrer tricks are prohibited: partner attribution MUST be a claim code the member
deliberately scans or types.

**Rationale**: Privacy asserted in marketing and privacy enforced in architecture are
different products. Deliberate attribution costs a little conversion and buys the ability
to describe the mechanism honestly to a regulator or a journalist.

### VII. Language and Place Are Independent Axes

UI strings MUST live in catalogues keyed by BCP-47 primary language subtags — German, Greek
and English at launch. Place MUST NOT be fused into a locale value; language and place are
separate parameters everywhere they appear, including in requests to Apivo surfaces.

**Rationale**: Fusing the two produces a combinatorial explosion of near-duplicate
catalogues and makes a combination such as "German language, Greek place"
unrepresentable. The Apivo constitution carries the same principle.

### VIII. Rebrandable Shell

No brand name, colour, endpoint or support address may be hardcoded outside one brand
configuration. A fixture brand MUST build in CI, proving the seam on every change.

**Rationale**: Partner-branded distributions stay possible without being promised to
anyone. A rebrandability claim that is not exercised by CI is discovered to be false at the
worst possible moment — while negotiating with the partner who asked for it.

### IX. Apps Are Content, Not Releases

First-party apps MUST ship as signed, versioned surfaces delivered server-side. A browser
release is only ever for the shell and its engine integration. An app MUST declare its
capabilities in its signed manifest and MUST NOT be able to widen them from inside;
anything page-adjacent additionally requires the user's per-app grant.

**Rationale**: Tying app content to browser releases means every copy change waits on an
update cycle and a staged rollout. Capability declaration in a signed manifest is what
keeps "an app can do more than it said" from being a code review question.

### X. Accessibility Is Not Optional

WCAG 2.1 AA on every shell surface, full keyboard operation, UI scaling to 200%, and
correct international text input — German and Greek at minimum — are release criteria, not
polish. A surface that fails them is not shippable.

**Rationale**: The first real cohort is 40+ and arrives through a partner counter rather
than through technical enthusiasm. Accessibility here is the difference between a product
this cohort adopts and one they abandon at the first dialog they cannot read.

## Permanent Prohibitions

The following are excluded permanently, not merely out of scope for a release. Removing any
of them requires a MAJOR amendment to this constitution:

- **Ad injection.** Evreos MUST NOT inject advertising into any web page, under any
  commercial arrangement.
- **Silent affiliate attribution.** Attribution MUST NOT be attached without an explicit
  user action for that occasion, and MUST NOT be claimed for a purchase the member's click
  did not lead to.
- **Server-side collection of browsing history.** Browsing history MUST NOT be transmitted
  to or retained by any server.

## Development Workflow

- Every change reaches `main` through a pull request linked to a GitHub issue. Direct
  pushes to `main` are prohibited.
- Commits are atomic: exactly one logical change per commit, each standing, building and
  reverting on its own.
- Every pull request that adds or changes a feature states the byte and millisecond cost
  of its change against the budgets in Principle II, as that principle requires. A red
  budget gate fails the merge.
- After every push of commits to an open pull request, an adversarial review of the pull
  request's full current diff MUST run: independent reviewers instructed to refute the
  changes across correctness, security, consistency — within the change and against the
  rest of the repository, including text the change does not touch — and compliance with
  this constitution and with the repository rules in `CLAUDE.md`, with findings verified
  before they count. Confirmed findings are fixed and pushed — which triggers a new review round
  — before the pull request is ready to merge. The outcome of every round, green or not,
  MUST be recorded on the pull request as a new comment, before the next push and in any
  case before the pull request is merged or closed or an override is stated. A record MUST
  NOT be amended or deleted; one edited or deleted after the fact is void, and the round
  it recorded stops counting until a fresh record is posted. Correction is done by posting
  a superseding record that quotes and corrects the original, never by editing it. That is
  what keeps a round which found something traceable to the fix that followed it.
- A pull request is NOT mergeable until the MOST RECENT review round on it has been
  recorded green — no confirmed findings — against the exact diff that would merge, which
  is `git diff <base>...<head>`, three dots, against the merge base. A later round
  supersedes an earlier one: a red round invalidates any green record at the same head, so
  a green record is never made durable by declining to run another round. The record is a
  comment on the pull request, as the bullet above requires, and states the head SHA, the
  base branch and the merge-base SHA it covers, and every finding raised in that round
  with its severity and its disposition — confirmed, or dismissed with the reason. A green
  round is one in which nothing was confirmed. A round that confirmed findings is answered
  by the next round's record, which names the commits that fixed them; the earlier record
  is never edited. Naming the SHAs is what lets a reader decide whether the round is still
  current; listing the findings is what makes "green" checkable rather than asserted,
  since a bare count lets two confirmed blockers be written up as "0 confirmed" with
  nothing a reader could check. A finding may be dismissed only by someone other than the
  author of the commits it concerns; a finding the author dismisses over the reviewer's
  disagreement counts as confirmed. A round covers exactly one (merge base, head) pair on
  one base branch, and is invalidated whenever the pull request's head SHA, base branch or
  merge base differs from what the record names, or when the merge would conflict. Green
  automated checks are not a substitute: they test what someone thought to automate, and
  the review exists for what nobody did. The founder may override, but the override is
  stated on the pull request before the merge, and names what it overrides and the
  confirmed findings it merges over. Stated afterwards it is not a decision, only an
  account of an accident.
- Feature work follows the Spec Kit flow: constitution → specify → plan → tasks →
  implement, with clarify, checklist and analyze as optional quality gates.
- Architectural decisions MUST be recorded as ADRs in this repository. The engine
  decision is recorded as ADR-0001.

## Governance

This constitution supersedes all other development practices in this repository. Where any
other document, template, skill or habit conflicts with it, this document governs.

**Amendment procedure**: Amendments require a recorded founder decision, land through a
pull request linked to an issue like any other change, and MUST update the version and the
Sync Impact Report in this file. An amendment that relaxes a principle MUST state what
replaces the discipline it removes.

**Versioning policy**: This constitution is versioned semantically.

- **MAJOR** — a principle is removed or redefined in a backward-incompatible way, including
  any removal from Permanent Prohibitions.
- **MINOR** — a principle or section is added, existing guidance is materially expanded,
  or guidance outside the Core Principles is tightened by withdrawing an option it
  previously allowed.
- **PATCH** — clarifications, wording and typo fixes that do not change meaning.

**Compliance review**: Every pull request review verifies compliance with these principles.
Violations of Principle I, Principle IV, Principle X or the Permanent Prohibitions are
release blockers, as is any client-side money logic prohibited by Principle V.
Complexity that appears to conflict with Principle II or Principle III MUST be justified in
the pull request that introduces it, or removed.

**Version**: 1.1.0 | **Ratified**: 2026-08-29 | **Last Amended**: 2026-08-30
