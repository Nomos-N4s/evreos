# Feature Specification: Evreos v1

**Feature Branch**: `feat/evreos-v1-spec`

**Created**: 2026-08-30

**Status**: Draft

**Input**: Block B of the browser programme master prompt, amended where ADR-0001 and verified investigation supersede it.

## Clarifications

### Session 2026-08-30

- Q: How should Evreos measure the 30-day retention target, given telemetry must
  be opt-in? (SC-011) → A: Measure both, reported separately — signed-in member
  retention from existing server-side account activity, and signed-out retention
  from an opt-in diagnostic signal, never combined into a single figure.
- Q: What is the oldest macOS version Evreos will support? → A: macOS 13
  Ventura. This sets the tier-2 engine floor, since the engine is the operating
  system's own.
- Q: Which search engine ships as default, and do you take payment for that
  placement? (Q-E2) → A: A privacy-preserving engine, with no paid placement or
  revenue arrangement in v1. Members can change it freely.
- Q: What happens when a macOS member needs a saved password, given platform
  autofill is unavailable there? (Q-E4) → A: State the limitation before
  install, and offer to hand the site off to the hand-off browser when a password
  field is encountered. No credential storage is built in v1.
- Q: Should the placeholder performance budgets be ratified now, or replaced
  after measurement? (Q-E9) → A: Ratify size, memory, idle and interaction
  budgets now as tighten-only CI gates; hold the cold-start figure (SC-002)
  until the cold-start spike measures engine initialisation on the reference
  hardware.
- Q: Is claim-code redemption in v1, or does it wait for the campaign backend?
  → A: They are two different flows. Member-facing claim-code redemption ships
  in v1. Partner-facing campaign administration is the flow blocked on the
  backend decision, and only that one ships disabled.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Browse privately on an ordinary machine (Priority: P1)

A person installs Evreos on a laptop that struggles with mainstream browsers, and
uses it as an everyday browser without signing in to anything. They open tabs,
search, keep bookmarks, download files, and find that trackers and adverts are
blocked from the first launch without being configured. The machine stays
responsive, and nothing asks them to create an account.

**Why this priority**: The constitution requires that, signed out and ignoring
every Apivo surface, Evreos is a genuinely good private browser — that is the
default experience, not a degraded mode. It is also the only story that is a
viable product on its own: nothing else in this specification is usable without
it, and a browser that cannot browse has no second chance with a reviewer.

**Independent Test**: Install on the reference hardware, complete a full session
— search, ten tabs, bookmark, download, print, find-in-page, close and reopen
— entirely signed out, and confirm every ratified budget in Success Criteria
holds, recording the measured cold start against the provisional SC-002 target.

**Acceptance Scenarios**:

1. **Given** a first launch on a machine with no prior profile, **When** the
   person opens the browser, **Then** an interactive window appears within the
   SC-002 cold-start target, which is provisional, and tracker blocking is
   already active.
2. **Given** ten open tabs, **When** the person leaves the machine idle,
   **Then** background tabs are suspended and processor use falls below the idle
   budget without audible fan activity.
3. **Given** a session with tabs open, **When** the browser is closed and
   reopened, **Then** the session is restored.
4. **Given** a mistyped address, an expired certificate, a captive portal, or a
   site requesting HTTP authentication, **When** navigation fails, **Then** the
   person sees an error page in their language that names the problem and offers
   a next step. Presenting the failure as a successful, blank page is a defect,
   as is a loading indicator that never resolves.
5. **Given** a site whose blocking breaks it, **When** the person opens the
   per-site control, **Then** blocking can be turned off for that site alone and
   the setting persists.
6. **Given** an existing Chrome, Firefox or Edge profile, **When** the person
   chooses to import, **Then** their bookmarks and history are available in
   Evreos.

---

### User Story 2 - Claim and follow cashback (Priority: P2)

A member arrives from a partner shop with a claim code, having been handed a
reason to install by a person they trust. They install Evreos, sign in once,
redeem the claim, browse the merchant catalogue, open an offer, and later see
what they have earned — including what is still pending, and why.

**Why this priority**: This is the revenue mechanism and the pilot's reason to
exist. It is second only because it depends on the shell from Story 1, and
because the constitution requires money surfaces to be opt-in rather than the
default experience.

**Independent Test**: With a fixture merchant network, complete the path from
scanning a claim code to seeing a pending entry in the wallet, then follow a
withdrawal request to a terminal state.

**Acceptance Scenarios**:

1. **Given** a printed claim code at the partner counter, **When** the member
   scans it after installing, **Then** the claim flow opens directly without the
   member navigating anywhere.
2. **Given** a signed-in member, **When** they open an offer from the catalogue,
   **Then** the click-out is tracked and the member is told plainly that it was.
3. **Given** a completed purchase, **When** the member opens the wallet,
   **Then** pending, confirmed and payable amounts appear exactly as the ledger
   reports them, with an explanation of why pending exists.
4. **Given** a payable balance, **When** the member requests a withdrawal,
   **Then** the request is recorded and its status is visible until it reaches a
   terminal state.
5. **Given** a member who has never signed in, **When** they browse the web,
   **Then** nothing requires an account and no money surface is imposed on them.
6. **Given** a member browsing a merchant site, **When** an offer applies,
   **Then** any activation requires an explicit action for that occasion, and
   attribution is never attached silently.

---

### User Story 3 - Read the news, and update apps without a browser release (Priority: P3)

A member opens epiloYES from the home surface as a first-class app rather than a
bookmarked page, reads the front page and an article, and switches language and
place independently. Separately, the operator publishes a change to that app and
members receive it without anyone installing a new browser.

**Why this priority**: It proves the app platform, which is what makes Evreos a
super-app shell rather than a browser with links. It ranks third because the
reader is valuable but not the wedge, and because the platform's value is only
demonstrable once at least one app runs on it.

**Independent Test**: Publish an app change server-side and confirm members see
it without a browser update, while the app's declared capabilities remain
unchanged.

**Acceptance Scenarios**:

1. **Given** the home surface, **When** the member opens epiloYES, **Then** it
   presents as an app with its own surface rather than a browser tab.
2. **Given** a member reading the news, **When** they change language, **Then**
   the interface language changes without changing their place, and vice versa.
3. **Given** a published app update, **When** a member next opens the app,
   **Then** they receive the update without a browser release.
4. **Given** an app requesting a capability beyond its manifest, **When** it
   attempts to use it, **Then** the request is refused; an app can never widen
   its own capabilities from inside.
5. **Given** an app surface that requires something page-adjacent, **When** it
   is first used, **Then** the member is asked for a per-app grant.
6. **Given** no network, **When** the member opens an app, **Then** a cached
   surface or an honest offline state appears rather than a blank screen.

---

### Edge Cases

- **The engine runtime is missing.** On a tier-1 machine without the required
  system web runtime, first run must be a designed experience with honest
  progress and a resumable download, not a silent stall. This is a separate
  path from the cold-start budget.
- **A site depends on protected media.** Some streaming that requires content
  protection may not play; which services are affected is unsettled and tracked
  as Q-E11, so no exclusion is described to anyone until it is measured. Where
  playback does fail, the member must be offered a clear hand-off to their
  hand-off browser rather than a failure they must diagnose.
- **A site behaves differently across platforms.** The same page may render
  differently on tier 1 and tier 2, so a reported problem may not reproduce.
  Reports must capture enough context to tell platform-specific faults apart.
- **The ledger and the client disagree.** The client never computes a balance;
  when the service is unreachable the wallet shows a stale-data state with the
  time of last update rather than a number it inferred.
- **A claim code is already redeemed, expired, or belongs to another member.**
  Each has a distinct, plain-language outcome; none may present as a generic
  error.
- **Blocking breaks a bank or government site.** The per-site control must be
  discoverable at the moment of failure, because this cohort will otherwise
  abandon the browser rather than hunt through settings.
- **Assistive technology crosses the boundary between browser chrome and page
  content.** Focus order and reading order must remain coherent across that
  boundary, which is where this class of interface commonly fails.
- **The person changes system language, or uses a layout requiring compose or
  dead keys.** Text entry must remain correct, including German dead keys and
  Greek layouts.

## Requirements *(mandatory)*

### Functional Requirements

**Browsing**

- **FR-001**: Users MUST be able to open, close, reorder and restore tabs, with
  the session restored after the browser is closed and reopened.
- **FR-002**: Background tabs MUST be suspended according to a stated policy,
  and suspension MUST be reversible without losing the page's state visible to
  the user.
- **FR-003**: A single entry field MUST combine search, history and bookmarks.
- **FR-003a**: The default search provider MUST be a privacy-preserving engine
  that does not build an advertising profile of the member, and MUST be
  changeable by the member without penalty. No paid placement or revenue-sharing
  arrangement for the default position is entered into for v1; should one ever
  be, it MUST be disclosed in the product rather than only in a policy document.
- **FR-004**: Users MUST be able to keep bookmarks, review history, and manage
  downloads.
- **FR-005**: Users MUST be able to find text within a page, adjust page zoom,
  and scale the interface up to 200%.
- **FR-006**: The browser MUST prompt per site for camera, microphone, location
  and notification access, and MUST allow those decisions to be revisited.
- **FR-007**: A private window MUST leave no browsing trace on the machine after
  it is closed.
- **FR-008**: Tracker and advert blocking MUST be active on first launch without
  configuration, and MUST offer a visible per-site control.
- **FR-009**: Users MUST be able to view documents in a portable document format
  consistently across supported platforms.
- **FR-010**: The browser MUST offer light and dark presentation and follow the
  system preference by default.
- **FR-011**: Every action reachable by pointer MUST be reachable by keyboard.
- **FR-012**: Users MUST be able to import bookmarks and history from the
  browsers named in Assumptions.
- **FR-013**: Users MUST be able to make Evreos their default browser from
  within it.
- **FR-014**: The browser MUST update itself, verifying the update's
  authenticity before applying it, and MUST support releasing to a proportion of
  users at a time.
- **FR-015**: When navigation fails — an unresolvable address, an untrusted or
  expired certificate, an intercepting network, or a request for authentication
  — the browser MUST distinguish that failure from a successful load, and MUST
  present an error state naming the cause and offering a next step. Treating a
  failed load as a successful empty page is a defect, as is a loading indicator
  that never resolves.
- **FR-015a**: Where the platform provides site-credential autofill through the
  engine, the browser MUST use it. Where it does not, the browser MUST state
  that limitation before installation rather than after, and MUST offer to open
  the site in the hand-off browser when a site-credential field is detected.
  Detection MUST be local to the device, MUST inspect only whether a
  password-type input is present, and MUST NOT transmit or retain page content.
  Evreos stores no site credentials of its own in v1. Which platforms provide
  autofill to an embedder is unverified — it appears in neither ADR-0001's
  verified nor its unverified list — so it MUST be tested on both tiers before
  release, and any assumption of absence recorded as an assumption.

**Super-app platform**

- **FR-016**: A home surface MUST present the installed first-party apps.
- **FR-017**: Each app MUST declare its capabilities in a signed, versioned
  manifest, and MUST NOT be able to widen them from inside.
- **FR-018**: Any capability that touches page content MUST additionally require
  a per-app grant from the user.
- **FR-019**: App surfaces MUST be updatable without releasing a new browser
  version.
- **FR-020**: App surfaces MUST be cached so that a stated offline state is
  presented rather than a blank surface.

**Identity**

- **FR-021**: One account MUST serve every app.
- **FR-022**: Signing in MUST be required for money surfaces and MUST NOT be
  required for browsing.
- **FR-023**: Account credentials MUST be held in the operating system's secure
  store.

**Cashback**

- **FR-024**: Users MUST be able to browse the merchant catalogue, with language
  and place as independent parameters.
- **FR-025**: Opening an offer MUST route through a click-out URL issued by the
  service for that occasion, and the user MUST be told that tracking is taking
  place. The client MUST NOT construct, template, or modify an affiliate link or
  any of its parameters. Principle V prohibits the client building an affiliate
  deeplink, and building the redirect client-side is the obvious implementation,
  so the prohibition is stated where it will be read.
- **FR-026**: The wallet MUST present pending, confirmed and payable amounts
  exactly as reported by the service, and MUST NOT compute or estimate any
  amount.
- **FR-027**: The wallet MUST explain in plain language why an amount is
  pending.
- **FR-028**: Users MUST be able to request a withdrawal and follow its status
  to a terminal state.
- **FR-029**: Member-facing claim-code redemption — scanning or entering a code,
  binding the member to an existing campaign, and showing the resulting entry in
  the wallet — MUST work in v1, against campaigns already held by the existing
  service. It is a distinct flow from FR-029a and is not blocked by it. It does
  depend on the existing service holding campaign records and accepting a
  redemption, because Principle V forbids the client producing either; if that
  service does not serve redemption, FR-029 is blocked by the same decision as
  FR-029a and both ship disabled. Confirm before v1 scope is treated as settled
  (Q-E11a), since SC-010 rests on the answer.
- **FR-029a**: Partner-facing campaign administration, by which a partner
  business creates or funds a campaign, MUST be present in the interface and
  disabled until its backing service exists, with an honest explanation rather
  than a broken control.
- **FR-030**: Attribution MUST never be attached without an explicit user action
  for that occasion, and MUST never be claimed for a purchase the user's action
  did not lead to.
- **FR-031**: The wallet MUST be delivered as part of the shell. It MUST NOT be
  delivered as an extension the browser hosts.

**Onboarding**

- **FR-032**: Scanning or entering a claim code MUST open the claim flow
  directly after installation.
- **FR-033**: Attribution for a partner referral MUST come from a code the
  member deliberately scans or types, and MUST NOT be inferred from the
  installation.

**Accessibility and language**

- **FR-034**: Every shell surface MUST meet WCAG 2.1 AA.
- **FR-042**: No brand name, colour, endpoint or support address may be
  hardcoded outside a single brand configuration, and a fixture brand MUST build
  in CI on every change. Principle VIII requires both; the spec previously
  carried neither.
- **FR-041**: Before download, the distribution page MUST state the minimum
  operating-system version for each platform and the tier-2 site-credential
  limitation of FR-015a. These are product surfaces, not marketing: they MUST
  meet WCAG 2.1 AA as FR-034 requires and MUST be available in German, Greek and
  English as FR-035 requires. The tier-2 installer MUST additionally refuse to
  install below the floor with a plain-language reason, rather than failing at
  first launch.
- **FR-035**: Interface text MUST be available in German, Greek and English,
  keyed by language alone, with place never fused into the language value.
- **FR-036**: Text entry MUST be correct for German dead keys and Greek
  layouts.

**Diagnostics**

Principle VI requires telemetry to be opt-in, aggregate AND EU-hosted — three
conditions, not two. A payload keyed to a stable per-install identifier is
pseudonymous rather than aggregate however well that identifier is bounded, so
FR-039a computes retention on the device rather than shipping a key that would
satisfy the fingerprinting prohibition while failing the aggregate requirement.

- **FR-039**: The browser MUST offer an opt-in diagnostic signal, off until the
  member turns it on. It MUST NOT carry browsing history, URLs or search terms,
  and MUST be reportable to a member in plain language before they consent.
- **FR-039a**: The signal MUST support 30-day retention without carrying any
  per-install identifier. Retention is a cohort measure, so the client MUST
  evaluate its own retention locally and emit at most two reports per install:
  an enrolment report on the first day diagnostics are enabled, carrying only
  the install week; and a retention report on a day from 24 to 30 after install,
  carrying only that same install week. Neither report may carry an identifier,
  and the service MUST NOT be able to link them. Signed-out retention is the
  ratio of retention reports to enrolment reports for an install week. No other
  per-install state may be transmitted.
- **FR-039b**: Crash reporting MUST be opt-in and off by default, MUST NOT
  include page contents, URLs or browsing history, and is subject to FR-039c.
- **FR-039c**: The diagnostic signal, crash reports, and any computation over
  them MUST be received, processed and retained only on infrastructure hosted in
  the European Union, and MUST be deleted no later than 40 days after receipt —
  the shortest period that supports a 30-day measure over weekly cohorts. No
  diagnostic or crash payload may be transmitted to or stored on infrastructure
  outside the European Union.
- **FR-040**: Signed-in retention MUST be derived from existing account and
  wallet activity that the service records as originating from an Evreos client,
  rather than from the diagnostic signal, so that members who decline
  diagnostics are still counted in the figure that matters most. The origin
  marker MUST be a client-type field on requests the member's own action
  initiates; it MUST NOT be a device fingerprint. Without it, a member who
  uninstalls Evreos and keeps using the existing web wallet produces the same
  account activity as a retained member and would be counted as retained. The
  retention computation MUST run on EU-hosted infrastructure.

**Honesty**

- **FR-037**: Where a capability is unavailable — protected media in
  particular — the browser MUST say so and offer a hand-off, rather than
  failing silently.
- **FR-038**: Benchmark methodology and scripts MUST be published so that a
  third party rerunning them obtains the same figures.

### Key Entities

- **Member**: a person with one account across apps. Browsing requires no
  member; money surfaces do.
- **App**: a signed, versioned surface with declared capabilities, updatable
  independently of the browser.
- **Merchant offer**: a catalogue entry, resolved by language and place as
  separate parameters.
- **Click-out**: a recorded intent to shop, carrying the reference that later
  attributes a purchase.
- **Wallet entry**: a ledger-derived amount in a stated state — pending,
  confirmed, declined or reversed — never computed by the client.
- **Withdrawal request**: a member-initiated request with a status that
  progresses to a terminal state.
- **Claim code**: a deliberately presented token binding a member to a partner
  campaign.
- **Site credential**: a username and password belonging to a third-party
  website the member visits.
- **Account credential**: the member's Apivo sign-in secret or token.
- **Campaign**: a partner-funded offer held by the Apivo service, to which a
  claim code binds a member.
- **Hand-off browser**: the browser Evreos opens a site in when it cannot serve
  it — the OS default browser, or, where Evreos is itself the default, the
  browser the member nominates once in settings, defaulting to the platform's
  own. Evreos MUST NOT nominate itself.
- **Session**: the set of open tabs and their state, restored across restarts.

## Success Criteria *(mandatory)*

### Measurable Outcomes

**Footprint and responsiveness.** Principle II requires every budget it names —
download size, installed size, cold start, shell memory, idle CPU, chrome input
latency — to live in one budget file and be enforced by a CI gate that fails the
build on regression. No budget named there may be un-gated at any point on the
release path, so every figure below is a gate from milestone M0, at the value
stated.

Ratified and provisional describe the *figure*, never whether the gate exists. A
ratified figure may afterwards only be tightened, never relaxed, except by
recorded founder decision. A provisional figure may be replaced once, by
recorded founder decision on spike evidence, and is tighten-only thereafter.
SC-003 states a required experience rather than a figure; it is verified by
acceptance test, not by a budget gate.

Reference machines are named in Assumptions as a class rather than by model.
SC-002 already says a figure is meaningless without them; that is equally true
of SC-004, SC-005 and SC-006, which the founder has ratified. Ratification is
not reopened here — see Q-E9a for the naming that reproducibility still needs.

- **SC-001** *(ratified)*: The download is 20 MB or less and the installed
  footprint 60 MB or less per platform, counting only the bytes Evreos ships and
  excluding any system-provided web runtime.
- **SC-002** *(figure provisional; gate mandatory from M0)*: With the system web
  runtime already present, an interactive window appears within 800 ms on a warm
  start and 2 s on first run, on each reference machine listed in Assumptions.
  Held open deliberately: a large share of this figure is the engine's own
  initialisation rather than Evreos's code, so the figure is ratified only after
  the cold-start spike measures that floor on the reference machines. The shell
  architecture is expected to be shaped by what that spike finds. Assumptions
  currently describes a class of machine rather than exact models; until models
  are recorded there this criterion cannot be measured reproducibly and MUST NOT
  be reported as met.
- **SC-003**: Where the system web runtime is absent, first run presents
  continuous, honest progress and completes without user intervention beyond
  consent. This path is a designed experience and is deliberately not held to
  SC-002.
- **SC-004** *(ratified)*: With ten tabs open,
  proportional set size attributable to Evreos processes, excluding the system
  web runtime's own processes, stays at or below 150 MB, measured 60 s after the
  tenth tab finishes loading on each named reference machine. USS and PSS give
  different answers for the same state, so the metric has to be named; PSS is
  used because it charges shared pages proportionally rather than discarding
  them, which is the conservative reading of a shell-overhead budget.
- **SC-005** *(ratified)*: When idle, processor use stays below 0.5% of one core
  at every 1-second sample across a 10-minute window, with no periodic wake
  activity, and background tabs are suspended.
- **SC-006** *(ratified)*: Switching tabs and typing in the address field produce
  a visible response within one frame at the display's native refresh rate, at
  the 99th percentile over 100 trials and with no trial exceeding two frames.

**Experience**

- **SC-007**: A person who has never used Evreos can install it, make it their
  default browser and import their bookmarks without assistance.
- **SC-008**: Every shell surface passes WCAG 2.1 AA, is fully operable by
  keyboard, remains usable at 200% scaling, and accepts German dead-key and
  Greek text entry correctly.
- **SC-009a**: The tier-2 build installs and launches on macOS 13.0, on the
  current macOS release, and on every major version between them. On a machine
  below the floor the pre-download statement required by FR-041 is present and
  the installer refuses with a named reason; zero cases of the floor being
  discovered only after installation.
- **SC-009**: Each of the four navigation failures FR-015 enumerates — an
  unresolvable address, an untrusted or expired certificate, an intercepting
  network, and a request for authentication — is exercised on every supported
  platform and produces an error state naming the cause and offering a next
  step. Zero failures presented as successful blank pages, and zero loading
  indicators that do not resolve within 30 s.

**Business**

- **SC-010**: At least 25% of people pitched at the pilot counter install
  Evreos and complete a claim.
- **SC-011**: Thirty-day retention is measured two ways and reported separately,
  never as one blended figure. **Signed-in retention** is at least 20%: the share
  of members who first signed in during a given calendar week and who record at
  least one Evreos-originated account request (FR-040) on any day from 24 to 30
  after install. It is derived from server-side account activity, which is
  transactional rather than diagnostic and needs no opt-in. **Signed-out
  retention** is reported alongside it over the same weekly cohorts, computed
  from the FR-039 signal; it is a self-selected sample, MUST be labelled as such
  and stated with its opt-in rate wherever it is reported, and carries no bar —
  it is reported for direction. The 20% bar applies to the signed-in figure
  only. Cohorts below 200 installs are reported with confidence intervals rather
  than as a point figure.

- **SC-012**: Active members average at least one cashback activation per month.

**Trust**

- **SC-013**: Benchmark methodology and scripts are published, and a third party
  rerunning them obtains the same figures.
- **SC-014**: No browsing history leaves the machine, and any diagnostic
  reporting is off until the member turns it on.

## Non-Goals

Carried from the master prompt: compatibility with existing browser extension
ecosystems; a built-in password manager; iOS; synchronisation across devices; a
virtual private network; crypto or web3 surfaces; and assistant sidebars.

Permanently excluded by the constitution: advert injection; silent affiliate
attribution; and server-side collection of browsing history.

Added on the evidence recorded in ADR-0001, and stated here so they never reach
a landing page:

- **Hosting third-party browser extensions inside Evreos.** Available only on
  the tier-1 platform, without interface surfaces and by manual installation;
  restricted by operating-system version on tier 2; unavailable on the deferred
  platform. It cannot be offered consistently and so is not offered.
- **Playback of content-protected streaming media.** Excluded from v1. The
  content-protection system most streaming services require is unavailable in
  every engine option considered, and is not obtainable by adopting a different
  engine strategy — it is a licensing wall rather than a technical one. Whether
  a different content-protection system available on the tier-1 platform covers
  any of these services is untested; see Q-E11. Until that is settled, FR-037's
  hand-off applies to all such media.

## Platform Scope

- **Tier 1 — Windows.** Release criteria apply in full.
- **Tier 2 — macOS 13 and later.** Because the engine is the operating system's
  own, this floor sets the rendering engine version, the web features available
  on this platform, and the security patch source. It excludes hosting
  third-party extensions, which is a non-goal regardless. It also excludes the
  macOS-14-and-later proxy configuration ADR-0001 records as one of only two
  blocking routes on this platform, leaving top-level navigation gating — which
  gates page navigations rather than subresources, and is nowhere claimed to be
  sufficient for tracker and advert blocking. FR-008 is a P1 acceptance
  scenario, so this consequence of the floor is tracked as Q-E12 rather than
  left to be discovered.
- **Deferred — Linux.** Not part of v1 cross-platform scope. It carries its own
  budget and its own go/no-go decision, gated on the spike results recorded in
  ADR-0001.

This supersedes the master prompt's equal treatment of three platforms.

## Open Decisions

Routed to `/speckit-clarify`. These are founder decisions and MUST NOT be
resolved silently by this specification.

- **Q-E1** Platform order, and whether a mobile platform precedes desktop
  polish.
- **Q-E2** *Settled 2026-08-30*: a privacy-preserving default with no paid
  placement in v1 (FR-003a). Which specific engine, and whether to revisit
  payment once traffic volume makes a deal realistic, remain open.
- **Q-E3** Distribution channels at beta.
- **Q-E4** *Settled 2026-08-30*: platform autofill where available, an honest
  limitation and a system-browser hand-off where it is not (FR-015a). Whether to
  integrate the platform keychain on tier 2 later remains open.
- **Q-E5** *Settled as a consequence of Q-E4*: site-credential import is out of
  scope for v1, because FR-015a forbids Evreos holding site credentials — what
  may not be held cannot be imported. Whether a later version imports them
  reopens with the tier-2 keychain question in Q-E4.
- **Q-E6** The minimum opt-in diagnostic set. Partly settled by the session
  2026-08-30 clarification: the set must at minimum support signed-out retention
  (FR-039). What else it carries, if anything, remains open.
- **Q-E7** Brand and trademark clearance, and standalone versus endorsed
  branding.
- **Q-E8** *Narrowed 2026-08-30*: superseded by Q-E13. Principle VIII settles the
  seam; only whether a partner build ships is open.
- **Q-E9** *Settled 2026-08-30*: SC-001, SC-004, SC-005 and SC-006 are ratified
  as tighten-only CI gates. The SC-002 figure is held provisional pending the
  cold-start spike; the gate itself exists from M0 regardless, because Principle
  II admits no un-gated budget.
- **Q-E9a** Which exact machine models are the reference hardware. SC-002 says a
  hardware-dependent figure is meaningless without them, which applies equally to
  SC-004, SC-005 and SC-006. Those three are ratified and stay ratified; until
  models are named, none of the four is reproducible by a third party, which
  SC-013 requires.
- **Q-E10** Whether affiliate attribution survives tracking prevention on the
  tier-2 platform. Recorded in ADR-0001 as unverified and as the only
  identified risk that can invalidate the business rather than the
  architecture.
- **Q-E11** Whether the content-protection system native to the tier-1
  platform covers any streaming services members actually use. The non-goal
  above is currently argued from a different system only, so the exclusion may
  be broader than the evidence supports.
- **Q-E11a** Whether the existing service serves claim-code redemption in v1 —
  holding campaign records and accepting a redemption. FR-029 ships on the
  assumption that it does, Principle V forbids the client producing either, and
  SC-010 rests on the answer.
- **Q-E12** Whether FR-008 can be met on macOS 13 without the macOS-14 proxy
  route ADR-0001 records, or whether the tier-2 floor must move to macOS 14.
  Blocking parity on tier 2 MUST be measured before the floor is treated as
  settled.
- **Q-E13** Whether a partner-branded distribution ships in v1. The rebrandable
  seam itself is not open: Principle VIII requires it and requires a fixture
  brand to build in CI on every change, regardless of whether any partner build
  is promised (FR-042).

## Assumptions

- **Reference hardware** for SC-002, SC-004, SC-005 and SC-006 is a 2020
  mid-range x86 laptop and an M1-class portable. Exact models remain to be named
  (Q-E9a). Until they are, no hardware-dependent figure is reproducible.
- **Import sources** are Chrome, Firefox and Edge, covering bookmarks and
  history. Credentials are out of scope pending Q-E5.
- **The pilot launches on existing web surfaces**, not on Evreos. Evreos's job
  for the pilot cohort is month-two retention, not day-one delivery.
- **Partner-facing campaign administration ships disabled** (FR-029a), because
  its backing service is a decision not yet taken outside this repository.
  Member-facing claim-code redemption (FR-029) is unaffected and ships in v1;
  the two were previously conflated under one name.
- **Money state originates entirely from the existing service.** This
  specification adds no money logic and assumes the ledger's vocabulary of
  pending, confirmed, declined and reversed.
- **The reader app consumes the existing publication**, rather than
  reimplementing it.
- **The SC-001 measurement boundary** excludes any system-provided web runtime,
  since it is not shipped and is shared with other applications.
