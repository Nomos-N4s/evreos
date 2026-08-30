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
  autofill may be unavailable there? (Q-E4) → A: State the limitation before
  install, and offer to hand the site off to the hand-off browser when a password
  field is encountered. No site-credential storage is built in v1.
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
holds, recording the measured first-run and warm-start times against the two
provisional figures SC-002 states.

**Acceptance Scenarios**:

1. **Given** a first launch on a machine with no prior profile and the system
   web runtime already present, **When** the person opens the browser, **Then**
   an interactive window appears within the first-run figure SC-002 states,
   which is provisional, and tracker blocking is already active. *(FR-008)*
2. **Given** ten open tabs, **When** the person leaves the machine idle,
   **Then** background tabs are suspended and processor use falls below the
   SC-005 idle figure without audible fan activity. *(FR-002)*
3. **Given** a session with tabs open, **When** the browser is closed and
   reopened, **Then** the session is restored. *(FR-001)*
4. **Given** a mistyped address, an expired certificate, a captive portal, or a
   site requesting HTTP authentication, **When** navigation fails, **Then** the
   person sees an error page in their language that names the problem and offers
   a next step. Presenting the failure as a successful, blank page is a defect,
   as is a loading indicator that never resolves. *(FR-015)*
5. **Given** a site whose blocking breaks it, **When** the person opens the
   per-site control, **Then** blocking can be turned off for that site alone and
   the setting persists. *(FR-008)*
6. **Given** an existing Chrome, Firefox or Edge profile, **When** the person
   chooses to import, **Then** their bookmarks and history are available in
   Evreos. *(FR-012)*

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
   member navigating anywhere. *(FR-032)*
2. **Given** a signed-in member, **When** they open an offer from the catalogue,
   **Then** the click-out is tracked and the member is told plainly that it was.
   *(FR-025)*
3. **Given** a completed purchase, **When** the member opens the wallet,
   **Then** pending, confirmed and payable amounts appear exactly as the ledger
   reports them, with an explanation of why pending exists. *(FR-026, FR-027)*
4. **Given** a payable balance, **When** the member requests a withdrawal,
   **Then** the request is recorded and its status is visible until it reaches a
   terminal state. *(FR-028)*
5. **Given** a member who has never signed in, **When** they browse the web,
   **Then** nothing requires an account and no money surface is imposed on them.
   *(FR-016a, FR-022)*
6. **Given** a member browsing a merchant site, **When** an offer applies,
   **Then** any activation requires an explicit action for that occasion, and
   attribution is never attached silently. *(FR-018a, FR-030)*

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
   presents as an app with its own surface rather than a browser tab. *(FR-016)*
2. **Given** a member reading the news, **When** they change language, **Then**
   the interface language changes without changing their place, and vice versa.
   *(FR-035)*
3. **Given** a published app update, **When** a member next opens the app,
   **Then** they receive the update without a browser release. *(FR-019)*
4. **Given** an app requesting a capability beyond its manifest, **When** it
   attempts to use it, **Then** the request is refused; an app can never widen
   its own capabilities from inside. *(FR-017)*
5. **Given** an app surface that requires something page-adjacent, **When** it
   is first used, **Then** the member is asked for a per-app grant. *(FR-018)*
6. **Given** no network, **When** the member opens an app, **Then** a cached
   surface or an honest offline state appears rather than a blank screen.
   *(FR-020)*

---

### Edge Cases

- **The engine runtime is missing.** On a tier-1 machine without the required
  system web runtime, first run must be a designed experience with honest
  progress and a resumable download, not a silent stall. SC-003 states what that
  path must meet, and holds it to neither figure SC-002 states.
- **A site depends on protected media.** On neither tier is the outcome
  established: on tier 1 whether PlayReady reaches the Win32 host Evreos ships,
  and whether it covers any service members use, are both untested (Q-E11); on
  tier 2 whether that platform's own system is reachable to a third-party host
  is untested (Q-E11b). No capability is claimed and no exclusion is asserted on
  either tier until measured. Wherever playback does fail, FR-037 applies and
  the member must be offered a clear hand-off to their hand-off browser rather
  than a failure they must diagnose.
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
- **FR-003**: A single entry field MUST combine search, history and bookmarks;
  the suggestions it offers are produced from local data only, as FR-007a
  requires.
- **FR-003a**: The default search provider MUST be a privacy-preserving engine
  that does not build an advertising profile of the member, and MUST be
  changeable by the member without penalty. No paid placement or revenue-sharing
  arrangement for the default position is entered into for v1; should one ever
  be, it MUST be disclosed in the product rather than only in a policy document.
  Submitting a search is the one point at which what the member typed leaves the
  machine, and it is the **submitted search** entry in FR-007a's list. The
  request MUST carry only the terms the member submitted, and MUST NOT carry an
  address the member navigated to, page content, the member's history or
  bookmarks, or any identifier Evreos assigns or persists across searches.
  Nothing is sent before the member submits. Changing the provider — by the
  member, or by brand configuration under FR-042 — changes which service
  receives the query and MUST NOT change what the query carries.
- **FR-004**: Users MUST be able to create, rename, organise into folders and
  delete bookmarks; to review, search and delete browsing history, both single
  entries and a chosen time range; and to see downloads in progress and
  completed, each with its destination on disk, to cancel one in progress, and
  to remove one from the list. Each of the three stores MUST survive closing and
  reopening the browser, and a deletion MUST NOT reappear afterwards. All three
  are local stores, and FR-007a governs what may leave the machine: none of the
  three is among the transmissions it enumerates, so none of them may be sent
  anywhere.
- **FR-005**: Users MUST be able to find text within a page, adjust page zoom,
  and scale the interface up to 200%.
- **FR-006**: The browser MUST prompt per site for camera, microphone, location
  and notification access, and MUST allow those decisions to be revisited.
- **FR-007**: A private window MUST leave no browsing trace on the machine after
  it is closed.
- **FR-007a**: Browsing history is the record of where the member has been: the
  addresses the member navigated to, when, and in what order, together with any
  store or transmission from which that record could be reconstructed. It MUST
  NOT be transmitted to, or retained by, any server. This binds by the
  transmission and not by who receives it — a server Evreos operates, one it
  contracts for, one a partner operates, and one named only by brand
  configuration under FR-042 are all covered equally. Principle VI states the
  rule as "Browsing history MUST NOT leave the machine"; the Permanent
  Prohibition on server-side collection states it as "transmitted to or retained
  by any server". Fetching a page transmits that page's address to the site
  serving it, which is the visit rather than a record of where the member has
  been, so the unqualified "any server" needs no exception carved out of it.

  The transmissions Evreos may make that carry an address the member navigated
  to, a term the member typed into the FR-003 field, or content of a page the
  member visited are exactly the four below, and the list is exhaustive:
  - **Page load**: the requests that load and use the site the member opened —
    the navigation request, the name resolution it requires, the subresource
    requests the page itself makes, and the form submissions and other requests
    the member's use of the page causes — sent to that site and to the hosts
    that page references.
  - **Certificate status**: the validity check made while loading a site, to the
    authority that site's certificate names, carrying that certificate's
    identifiers.
  - **Submitted search**: the terms the member submits from the FR-003 field, to
    the default search provider, on the boundary FR-003a states.
  - **Hand-off**: the address of the current site, passed under FR-015a or
    FR-037 on the member's action for that occasion, to the hand-off browser —
    a program on the same machine, not a server.

  Anything not on that list is forbidden, whether it runs in the foreground or
  the background, at any volume, under any retention period, and with or without
  the member's consent — a Permanent Prohibition admits no consent exception.
  Named because each is a plausible addition: no address-bar or as-you-type
  suggestion service; no reputation, safe-browsing or malware lookup keyed to the
  address being visited; no prefetch, preconnect or name resolution derived from
  history or from a partly typed address; no synchronisation of history or
  bookmarks; and no diagnostic or crash payload carrying either, which FR-039's
  payload rule and FR-039c's closed list of crash-report contents also exclude.
  Adding an entry to the list is an amendment to this specification, made in the
  pull request that would add the transmission and checked against the Permanent
  Prohibition there; it is never an implementation decision. Brand configuration
  under FR-042 may change which server receives an enumerated transmission, and
  MUST NOT add a transmission the list does not carry, widen what an enumerated
  one carries, or remove the member action an entry requires.

  A transmission the system web runtime makes while serving Evreos is Evreos's
  transmission for this requirement. Where that runtime offers a feature that
  sends visited addresses on its own — a reputation or address-filtering service
  is the case to expect — the feature MUST be turned off, because no entry above
  accounts for it.

  Suggestions in the FR-003 field MUST be produced only from data already on the
  machine: the member's history, bookmarks and open tabs. The field therefore
  transmits nothing as the member types, and no suggestion service exists to be
  consented to.

  Conformance MUST be demonstrated by a network-capture test committed to this
  repository and run in CI, exercising first launch, typing in the FR-003 field
  without submitting, a submitted search, navigation to a site with subresources,
  a private window, and a hand-off; it MUST fail on any outbound request from
  Evreos that no entry above accounts for.
- **FR-008**: Tracker and advert blocking MUST be active on first launch without
  configuration, and MUST offer a visible per-site control. Blocking MUST also
  collapse the empty space a blocked element leaves, because a page that reserved
  layout for an advert which never arrived reads as broken rather than as
  protected, and the member's likely response to a page full of holes is to turn
  blocking off. That collapsing modifies the page's rendering, which is why
  FR-018a has to rule on whether it is injection.
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
  Evreos stores no site credentials of its own in v1. Whether either supported
  platform provides autofill to an embedder is unverified: it appears in neither
  ADR-0001's verified nor its unverified list. It MUST therefore be established
  by a test on each tier whose result — the platform version tested, the
  reference machine, and presence or absence — is committed to this repository
  and owned by the founder. A tier MUST NOT be released until its result is
  committed. That is a release blocker rather than a deferral: FR-041 forbids the
  distribution page asserting presence or absence until the result exists, so an
  untaken test leaves the member without the statement FR-041 requires before
  installing, rather than merely leaving a question open.

**Super-app platform**

- **FR-016**: The home surface MUST present, whenever the member opens it, every
  installed first-party app the member has not removed from it under FR-016a.
  Removing an app under FR-016a takes it off the home surface and not out of the
  browser: it remains installed, remains reachable from the menu entry FR-016a
  guarantees, and the removal persists until the member reverses it, so a
  presented app and a removed one are the two states this requirement ranges
  over. Hiding the home surface itself under FR-016a removes it from the browsing
  experience — the new-tab page and the chrome — and never from that menu entry.
  Opening it from that entry while it is hidden presents it for that occasion
  only: the hidden state persists, and clearing it MUST be a separate choice the
  member makes deliberately, so that reaching a hidden surface never restores it.
  Without that, a member who hid the home surface and later opened it to reach
  the wallet would find it back in the new-tab page, and FR-016a's dismissal — a
  release criterion under Principle IV — would hold only until the member used
  the reachability the same requirement guarantees.
- **FR-016a**: Apivo surfaces MUST be discoverable, opt-in and dismissible, as
  Principle IV requires — all three. *Discoverable*: a single neutral entry point
  to the home surface MUST be present in the browser's own menus from first run,
  and every dismissible Apivo surface — the home surface, the wallet, the claim
  surface — MUST remain reachable from those menus after dismissal. Neutral is
  not left to self-assessment: that entry point MUST be a static label drawn from
  the interface catalogues under FR-035, rendered in the menu's own typeface and
  colour, and MUST NOT carry a brand colour, a badge, a counter, an amount, a
  promotional string, or any state derived from the wallet, the claim surface or
  any other money surface. It MUST read identically on a fresh profile and on a
  signed-in member's machine, which is the test a build can be held to. That
  entry point is not itself an Apivo surface and is exempt from the opt-in rule
  below; without the exemption, opt-in and discoverability would be circular,
  since opening requires reaching and reaching requires appearing. *Opt-in*: no
  Apivo surface may be rendered — in the new-tab page, in the chrome beyond that
  menu entry, or in page content — until the member has activated it once. On a
  fresh profile that one menu entry is the whole of Apivo's presence; everything
  else the profile presents is a browser. *Dismissible*: a member MUST be able to
  remove any first-party app from the home surface, to hide the home surface
  entirely, and to dismiss the wallet and claim surfaces, and each choice MUST
  persist across restarts and updates. An app update or a browser release MUST
  NOT reverse a dismissal, and every dismissal MUST be reversible by the member
  from the same menu entry. Dismissal removes a surface from the browsing
  experience, never from the member's reach: FR-028 requires a withdrawal to be
  followable to a terminal state, which a wallet with no way back would make
  unsatisfiable.
- **FR-018a**: Nothing may be injected into a web page without an explicit member
  action for that occasion. Principle IV states the rule for "that occasion" and
  leaves an occasion undefined; this requirement fixes it at the narrowest
  available reading, so that no standing permission can supply one. An action
  qualifies only where all three of the following hold:
  - it is taken in the browser's own chrome — a control the shell renders and
    owns, outside the page — and not in page content;
  - it is addressed to the specific thing it authorises: that offer, named to the
    member in that control, rather than injection in general;
  - it is taken on the page load it authorises and authorises that page load
    alone; a later navigation, a reload, and a restored tab are each a new
    occasion.

  Interaction with page content is never such an action. A click, keypress,
  scroll or pointer movement anywhere in the page authorises nothing, at any
  position and on any element, because the member directed it at the site rather
  than at Evreos. An overlay armed by the member's first interaction with a
  merchant's page therefore satisfies no part of the test above and is prohibited
  however it is described. A per-app grant under FR-018 authorises an app to
  respond to a qualifying action; it does not authorise injection in its absence,
  and the standing per-app grant FR-018 carries from Principle IX is a weaker
  thing that does not reach this. In particular a cashback offer MUST NOT alter,
  overlay or annotate a merchant's page until the member has taken a qualifying
  action for that offer on that page load. Principle IV makes a violation of it a
  release blocker rather than a bug.

  This requirement reads *injected* in its ordinary sense, which is the sense
  Principle IV uses: the insertion of content into a page. Removing content from
  a page, collapsing the space a removed element left, and rendering the shell's
  own chrome are not insertions of content, and nothing in this requirement
  reaches them. Where a shell function does place something in a page, it is
  exempt only on a characterisation whose three parts must all hold: the shell
  speaks on the member's behalf, carries no commercial interest, and places no
  third party's content in the page. Content blocking under FR-008 meets it,
  including the collapsing of blocked slots that requirement mandates, as do
  find-in-page under FR-005, error and failure states under FR-015, the
  site-credential hand-off offer under FR-015a, and the capability hand-off under
  FR-037. A later shell function meeting the test is equally exempt; a commercial
  one is not, however it is packaged. FR-018b forecloses the case the test would
  otherwise be argued over, advertising, without needing it.
- **FR-017**: Each app MUST declare its capabilities in a signed, versioned
  manifest, and MUST NOT be able to widen them from inside.
- **FR-018b**: No advertising may be placed in a web page by Evreos, by any app,
  or under any commercial arrangement, with or without a member action. The
  Permanent Prohibitions forbid advert injection "under any commercial
  arrangement" and admit no consent exception, so FR-018a's per-occasion consent
  rule does not reach it: a member tapping "show offers here" does not make an
  injected offer panel permissible. A cashback offer surface MUST therefore be
  rendered in the browser's own chrome, never in the page.
- **FR-018**: Any page-adjacent capability MUST additionally require a per-app
  grant from the member, which MUST be asked for when the capability is first
  used, as Story 3's fifth acceptance scenario requires. A capability is
  page-adjacent when its subject is a web page the member visits or has visited,
  or that page's context, whether the app alters it, reads it, or only observes
  it: reading or altering page content, reading the address or title of the
  current or any open tab, observing navigation or session state, reading or
  writing a page's storage or cookies, and running code in a page are all
  page-adjacent, and that list is illustrative rather than exhaustive. Observing
  without altering is page-adjacent regardless: reading the current tab's URL
  touches no page content and requires the grant. "Touches page content" is
  narrower than Principle IX's "anything page-adjacent" and MUST NOT be
  substituted for it. Every capability an app may declare under FR-017 MUST be
  classified page-adjacent or not in the capability catalogue published with the
  manifest format, and a capability that catalogue does not classify MUST NOT be
  granted, so that an implementer cannot escape the grant by naming a new
  capability.
- **FR-019**: App surfaces MUST be updatable without releasing a new browser
  version.
- **FR-019a**: Every delivered app surface MUST be signed, and the client MUST
  verify that signature before rendering the surface or writing it to the FR-020
  cache; an unverifiable surface MUST be refused, the cached copy retained, and
  the refusal stated rather than shown as a blank surface, which is what FR-020
  already requires of the offline case. Principle IX requires apps to ship as
  signed surfaces; FR-017 signs the manifest, which covers the capability
  declaration and not the surface, so a compromised delivery host or an
  intercepting network could otherwise render modified content inside the shell
  that holds the wallet. Three properties are required of that verification, each
  because without it the signature check passes on content the member should
  never have been shown:
  - **A pinned trust root.** The root of trust MUST be pinned in the shipped
    shell, and MUST NOT be fetched, replaced or updated from the host that serves
    the surface or from any host under the same control. A change of root reaches
    the member only in a browser release under FR-019b. Otherwise a compromised
    host serves its own root alongside its own modified surface and the client
    verifies both.
  - **One signature over the whole binding.** A single signature MUST cover the
    surface bytes, the app's identity, the digest of that app's FR-017 manifest,
    and the surface version together, and the client MUST refuse a surface whose
    signed app identity is not the app it is about to render, or whose signed
    manifest digest is not the manifest whose capabilities it would run under.
    Otherwise one app's signed surface can be served with another app's signed
    manifest and run with that app's capabilities.
  - **No downgrade.** The delivered surface's version MUST be greater than or
    equal to the version of the cached copy it would replace; a lower version
    MUST be refused, the cached copy retained, and the refusal stated. Otherwise
    a correctly signed older surface with a known defect can be replayed at the
    member by whoever controls delivery.
- **FR-019b**: A browser release MUST contain only the shell and its engine
  integration, as Principle IX requires. No app surface, and no cached copy of
  one, may ship in an installer or in a browser update, and the FR-020 cache MUST
  be populated only from surfaces the service delivers after installation, each
  verified under FR-019a. A pre-cached surface shipped in an installer would
  carry a valid signature and so would satisfy FR-019a, which is why that
  requirement does not carry this one; and app content travelling inside a
  release puts app updates back on the release cycle Principle IX exists to keep
  them off.
- **FR-020**: App surfaces MUST be cached so that a stated offline state is
  presented rather than a blank surface.

**Identity**

- **FR-021**: One account MUST serve every app: a member who signs in once on any
  Apivo surface is signed in on all of them, no app presents a sign-in or an
  account of its own, and signing out on one surface ends the session on all of
  them. An observer verifies this by signing in once, opening each installed
  first-party app in turn and finding the same member identity with no second
  sign-in prompt, then signing out on one surface and finding the others signed
  out.
- **FR-022**: Signing in MUST be required for money surfaces and MUST NOT be
  required for browsing.
- **FR-023**: Account credentials MUST be held in the operating system's secure
  credential store on every supported platform, and nowhere else: no Evreos
  profile file, database, preference store, cache or log may hold the credential,
  a token derived from it, or any value from which either can be reconstructed.
  Where that store is unavailable, or the member declines access to it, the
  member stays signed out; the credential MUST NOT fall back to storage Evreos
  writes itself. An observer verifies this by signing in, searching the profile
  directory and the logs for the credential and finding no match, and by deleting
  the entry from the platform store and finding the member signed out.

**Cashback**

- **FR-024**: Users MUST be able to browse the merchant catalogue, with language
  and place as independent parameters.
- **FR-025**: Opening an offer MUST route through a click-out URL issued by the
  service for that occasion, and the user MUST be told that tracking is taking
  place. The client MUST NOT construct, template, or modify an affiliate link or
  any of its parameters, because Principle V forbids the client building an
  affiliate deeplink and assembling the redirect in the client is the
  implementation that would breach it.
- **FR-026**: The wallet MUST present every amount the service reports, in the
  state the service reports for it — pending, confirmed, declined and reversed,
  and payable where the service reports a payable amount — exactly as reported,
  and MUST NOT compute, estimate, aggregate or omit any of them. Principle V
  requires all four states to be displayed as the ledger reports them: a wallet
  that shows pending and confirmed but drops declined and reversed states a
  larger entitlement than the ledger holds, and the member believes the wallet.
- **FR-026a**: The cashback invariants Principle V names — double entry,
  evidence, approver-gated payouts, exactly-once — live behind the Apivo API, and
  the client MUST NOT re-implement, approximate or cache as truth any of the
  four. In particular the client MUST NOT post, pair or balance ledger entries;
  MUST NOT generate, hold or infer the evidence for an entry; MUST NOT approve,
  pre-approve or predict the outcome of a payout; and MUST NOT deduplicate money
  actions or treat a retry of its own as having settled one. A wallet value held
  on the device is a cached copy: it MUST be presented as stale, carrying the
  time it was last received from the service, and never as a current balance. On
  reconnection the service's value replaces the cached one outright — the client
  MUST NOT reconcile, merge or diff the two, because a client that resolves a
  disagreement with the ledger has computed a balance.
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
  FR-029a and both ship disabled. Q-E11a records that dependency and SC-010 rests
  on it; if it resolves negatively this requirement is renegotiated rather than
  silently disabled.
- **FR-029a**: Partner-facing campaign administration, by which a partner
  business creates or funds a campaign, MUST be present in the interface and
  disabled until its backing service exists. The disabled control MUST state in
  plain language that the flow is not yet available, MUST NOT be presented as a
  control that failed, and MUST make no request to any service when the member
  activates it. An observer verifies this on a build with no backing service by
  finding the control present and reachable, reading the stated explanation, and
  confirming that activating it produces no outbound request and no error state.
- **FR-030**: Attribution MUST never be attached without an explicit member
  action for that occasion, and MUST never be claimed for a purchase the member's
  click did not lead to. Principle IV and the Permanent Prohibition on silent
  affiliate attribution both set the connection at the member's click, and the
  Non-Goals section names this requirement as what enforces that prohibition, so
  no weaker connection may carry a claim — not a visit to the merchant, not a
  search, not a session already in progress, and not a click the member made
  elsewhere.
- **FR-031**: The wallet MUST be delivered as part of the shell. It MUST NOT be
  delivered as an extension the browser hosts, and MUST NOT be installed,
  enabled, updated or removed through any extension mechanism. An observer
  verifies this by finding the wallet present and usable in a build with no
  extension host loaded and no extension installed, and by finding no extension
  package or manifest for the wallet anywhere in the distribution.

**Onboarding**

- **FR-032**: Scanning or entering a claim code MUST open the claim flow
  directly after installation.
- **FR-033**: Attribution for a partner referral MUST come from a code the
  member deliberately scans or types, and MUST NOT be inferred from the
  installation.

**Accessibility and language**

- **FR-034**: Every shell surface MUST meet WCAG 2.1 AA.
- **FR-044**: Rendering MUST go through an interface the shell defines as the
  consumer, with the system web runtime as the default implementation and a
  headless implementation kept working from day one, as Principle III requires —
  which in this specification means from milestone M0, the first point at which
  the shell builds and runs in CI and therefore the earliest point at which a
  second implementation can be kept working at all. The seam is proved by that
  second implementation rather than asserted. The shell MUST be stable Rust with
  no nightly features on the release path. Electron, CEF and any bundled
  Chromium are permanently rejected, exactly as Principle III rejects them. A
  web engine Evreos itself fetches, unpacks or installs — at first run, on
  update, or on demand — is bundled for the purposes of this requirement and of
  the SC-001 count, whether or not it is present in the build output; the only
  web engine that is not bundled in that sense is one the operating system
  provides and shares with other applications. Principle III leaves room for a
  pure Rust engine as an experimental third backend when one becomes
  daily-drivable, and this requirement MUST NOT be read to foreclose it; such a
  backend ships inside the same budgets and states its cost under FR-043 like
  any other change.
- **FR-043**: Every budget Principle II names — download size, installed size,
  cold start, shell memory overhead, idle CPU, chrome input latency — MUST live
  in one budget file in this repository, and MUST be enforced by the CI gates the
  Success Criteria preamble defines, from milestone M0. Every pull request that
  adds or changes a feature, and every pull request that changes any quantity
  those budgets measure — a new dependency, a bundled asset, a build or packaging
  change — MUST state its byte and millisecond cost against that file, whether or
  not it changes behaviour a member can observe. A change that adds bytes or
  milliseconds without adding behaviour is the case this covers, and it is the
  case a rule scoped to shipped behaviour lets through. A stated cost is not by
  itself a justification: Principle II requires that "a feature that cannot
  justify its cost is not added", so a change whose cost the pull request cannot
  justify MUST NOT be merged on the strength of a green gate, and refusing it on
  that ground is a founder decision recorded on the pull request. The gate rule
  itself is stated once in the Success Criteria preamble and is not restated
  here.
- **FR-042**: No brand name, colour, endpoint or support address may be
  hardcoded outside a single brand configuration, and a fixture brand MUST build
  in CI on every change, proving the seam rather than asserting it, as Principle
  VIII requires.
- **FR-041**: Before download, the distribution page MUST state the minimum
  operating-system version for each platform that declares one, and, for any
  platform where the test FR-015a requires has found site-credential autofill
  absent, that limitation for that platform. Until that test has run the page
  MUST NOT assert either presence or absence. The distribution page is neither a
  shell surface under FR-034 nor interface text under FR-035, so this requirement
  is the one that carries both obligations to it: the page MUST meet WCAG 2.1 AA,
  MUST be available in German, Greek and English keyed by the primary language
  subtag alone as FR-035 keys interface text, and MUST keep language and place as
  two separate values in its text, its download links and their parameters. Those
  two obligations are verified on the published page, before each release that
  page advertises, by an automated WCAG 2.1 AA check, a keyboard-only pass over
  the whole download path, and a rendering of each of `de`, `el` and `en` showing
  no untranslated string and no fused language-and-place value; the results are
  published with the release, and a failure blocks that release. The tier-2
  installer MUST additionally refuse to install below the floor with a
  plain-language reason, rather than failing at first launch.
- **FR-035**: Interface text MUST be available in German, Greek and English,
  keyed by the BCP-47 primary language subtag alone — `de`, `el`, `en` — with no
  region subtag in the key and place never fused into the language value.
  Language and place MUST be represented as two separate values wherever either
  appears — in stored preferences, in interface state, and in every request
  Evreos makes to an Apivo service, not only in the merchant catalogue.
  Principle VII says "everywhere they appear, including in requests to Apivo
  surfaces": the Apivo case is its example, not its scope. "Keyed by language
  alone" would on its own be satisfied by `de-DE`, which re-fuses the two. The
  distribution page is not interface text and is not governed here; FR-041
  carries this requirement's language obligation, and FR-034's accessibility
  obligation, to that page and states how both are verified.
- **FR-036**: Text entry MUST be correct for German dead keys and Greek
  layouts.
- **FR-036a**: Neither the shell nor any app it hosts may derive, store or
  transmit a stable identifier for a device or a member from device, display,
  font, network or timing characteristics, or from any combination of them.
  Principle VI prohibits fingerprinting outright, alongside the install-referrer
  tricks FR-033 carries, and the prohibition admits no consent exception and no
  diagnostic exception. It binds every channel Evreos has, not only the
  diagnostic signal FR-039 governs and the origin marker FR-040 constrains. A
  manifest under FR-017 MUST NOT declare a capability that requires such a
  derivation, and a per-app grant under FR-018 authorises page-adjacent access
  but never this.

**Diagnostics**

Principle VI sets three conditions on telemetry and crash reporting — opt-in,
aggregate, and EU-hosted — and prohibits fingerprinting separately. All three
conditions bind independently: a payload keyed to a stable per-install identifier
is pseudonymous rather than aggregate however well that identifier is bounded, so
satisfying the fingerprinting prohibition does not satisfy the aggregate one.
These requirements are written from those conditions rather than from a
measurement wish-list: what may be transmitted and retained is bounded by the
conditions first, and the measure is defined inside what remains.

- **FR-039**: The browser MUST offer an opt-in diagnostic signal, off until the
  member turns it on and turnable off again at any time, taking effect before the
  next report. Before consent the member MUST be shown, in plain language, every
  transmission the signal causes, and on which occasion each one happens —
  including that turning the signal off transmits the withdrawal report FR-039a
  requires only when no retention report has yet been sent for that enrolment and
  the enrolment's window has not closed, and that turning it off at any other
  time, or turning it back on afterwards, transmits nothing. The signal MUST NOT
  carry browsing history, URLs, page content or search terms.
- **FR-039a**: Signed-out 30-day retention MUST be measured without any
  per-install identifier. The client evaluates its own retention locally, whenever
  it runs, and emits at most two reports per enrolment and at most one enrolment
  per install:
  - an **enrolment report** on the first day diagnostics are enabled after
    installation, carrying only the enrolment week;
  - then exactly one of a **retention report**, on the first day the browser runs
    in the window 24 to 30 days after that enrolment, or a **withdrawal report**,
    if diagnostics are disabled before that window closes and no retention report
    has been emitted for that enrolment. Both carry only the same enrolment week.
    Re-enabling after a withdrawal emits nothing further.

  Both are keyed to enrolment, never to install: keying them to different events
  makes the measure meaningless, and nothing about the install date is
  transmitted. **Signed-out retention for an enrolment week is that week's
  retention-report count divided by its enrolment-report count less its
  withdrawal-report count.** The withdrawal count MUST be published beside the
  rate, or every opt-out reads as churn. Where that denominator is not positive —
  withdrawals equalling or exceeding enrolments for the week — no rate is reported
  for that week: the counts are reported and the rate is stated as not computable.
  Every publication of a count or a rate under this requirement is subject to
  FR-039e.

  The report endpoint accepts unauthenticated reports by design, because FR-039b
  admits no credential that distinguishes one client from another, so nothing at
  the endpoint can tell a client's report from a fabricated one. The signed-out
  figure therefore carries no integrity guarantee against a forged report stream.
  It MUST be labelled as unverified wherever it is reported, MUST be used only for
  direction inside the project, and MUST NOT be quoted outside the project as a
  measurement.
- **FR-039b**: No report may carry an identifier, and identifier-free MUST hold at
  the network layer too, where two reports from one address in a small cohort are
  otherwise trivially linkable. Reports MUST reach the service through a relay,
  and the relay MUST be structurally unable to read what it forwards: the client
  MUST encrypt the payload to the receiving service's public key, pinned in the
  build and rotated only by a release, and the relay MUST see only the ciphertext,
  its length and the destination. Separation of parties alone does not achieve
  this, because a terminating proxy run on the receiving service's own account is
  a different party that nevertheless sees both the source address and the
  payload, which is the arrangement this requirement exists to prevent.

  The relay MUST be operated by a legal entity distinct from the entity operating
  the receiving service, and that entity and its jurisdiction MUST be named in the
  pre-consent disclosure FR-039 requires. The relay's obligations below MUST be
  terms of a written contract with that operator, whose existence and effective
  date are stated in the same disclosure. Where no operator is named or no such
  contract is in force, the diagnostic signal MUST NOT be offered and no report
  may be transmitted; that is the consequence, since the obligations fall on a
  party this project does not otherwise control. The obligations are: the relay
  MUST NOT retain source addresses, transport metadata, or any log correlating an
  inbound and an outbound request; and it MUST forward the service's delivery
  acknowledgement on the same connection without retaining it.

  The receiving service MUST NOT retain the source address, transport metadata, or
  a receipt timestamp finer than the day. A client MUST NOT retransmit an
  unacknowledged report — an unacknowledged enrolment is abandoned and emits
  nothing further — because FR-039d counts on receipt with no identifier, so a
  retransmission cannot be deduplicated and would inflate the denominator. These
  constraints apply identically to crash reports; the enrolment-week and
  report-count constraints above do not, since crash reporting is separately
  consented and may exist where no enrolment does.
- **FR-039c**: Crash reporting, if it ships in v1 at all (Q-E16), MUST be opt-in,
  off by default, and separately consented from FR-039's signal, which Principle
  VI treats as a distinct thing. Separate consent does not narrow FR-039's content
  ban: a crash report MUST NOT carry browsing history, URLs, page content or
  search terms either.

  A crash report MUST carry only a symbolised stack trace, the browser and
  operating-system version, and a crash-reason code. The reason code MUST be drawn
  from a closed enumeration committed to this repository; a report carrying a code
  outside that enumeration MUST be discarded on receipt rather than counted, and
  adding a code is a change to that file. A free-form reason string is forbidden,
  because a field that accepts arbitrary text accepts a URL.

  Each stack frame MUST carry only the module name, the symbol name, and the
  source file and line drawn from Evreos's own debug information. Frame arguments,
  register contents, and strings read from the heap or the stack MUST NOT be
  captured or transmitted: an argument or a heap string is where a URL or a page's
  text otherwise reaches a crash report. Full process memory MUST NOT be captured
  or transmitted: it necessarily contains URLs and page content, and forbidding
  those inside a memory image is not implementable, so the capture is bounded
  instead. Before consent the member MUST be shown in plain language exactly what
  a crash report contains, and the per-install daily cap FR-039e requires.
- **FR-039d**: Counters, not reports, are the retained artefact — this is what
  Principle VI's *aggregate* condition requires. A diagnostic report MUST be added
  to its (report type, enrolment week) counter on receipt and discarded by the end
  of the following calendar day, which is the finest deadline a day-granularity
  receipt timestamp can audit. A crash report MUST be added to its (symbolised
  stack, release, operating-system version, reason code) counter on receipt and
  discarded on that same deadline; the counter's key is the symbol list itself
  rather than a hash of it, because a retained hash is undiagnosable and a list of
  Evreos's own symbol names carries no member content and no per-install content.
  No report of either kind may be retained individually.

  Those two keys are the only counter keys permitted. No counter may be keyed on
  anything else — machine or processor model, screen geometry, installed fonts,
  timezone, language, or any combination of them — and discarding the reports on
  schedule does not license one, because a count of one under a key few installs
  share is a per-install record whatever it is called. Adding or widening a
  counter key is an amendment to this specification, not an implementation choice.

  A counter held below FR-039e's threshold carries the same risk and MUST also be
  discarded on a stated deadline rather than kept until it might one day qualify.
  A (report type, enrolment week) counter is discarded 30 days after that
  enrolment week ends, which is the last day a report bearing that week can arrive
  under FR-039a. A crash counter is discarded 90 days after its first increment: a
  stack that has not drawn 50 reports in 90 days is not a crash this project will
  act on, and holding it longer preserves a near-unique record of the machines
  that hit it for no diagnostic return.

  If symbol-keyed counters prove insufficient to diagnose crashes, crash reporting
  does not ship in v1 (Q-E16) unless Principle VI is amended through its own
  procedure; this specification grants no exception to it.
- **FR-039e**: No counter of either kind may be published, exported, or used in
  any derived figure until it covers at least 50 reports. Below that threshold the
  counter is held and nothing drawn from it is released, in any form — not the
  count, not a rate computed from it, not a range, and not a confidence interval,
  since an interval around a small count discloses the same thing more politely. A
  crash counter of one is one member's crash on one code path. Where a figure this
  specification names would be drawn from a counter below the threshold — the
  signed-out retention rate FR-039a computes among them — the figure is withheld
  and its absence stated, never reported with a wider interval instead. At or
  above the threshold this requirement does not settle how a figure is expressed;
  below it, no expression is permitted.

  The threshold counts reports, and it is a lower bound on distinct installs only
  if no install can contribute twice. A client MUST therefore emit at most one
  crash report per (symbolised stack, release, operating-system version, reason
  code) key per calendar day, enforced on the device and stated in the pre-consent
  disclosure FR-039c requires. Without that cap one machine in a crash loop clears
  the threshold on its own, which is exactly the single member on a single code
  path the threshold exists to suppress. FR-039a already admits at most one report
  of each type per enrolment, so the diagnostic counters need no further cap.
  Because FR-039b admits no client credential, this cap binds the client and is
  not a guarantee against a fabricated stream; FR-039a states what that leaves the
  published figure worth.
- **FR-039f**: The diagnostic signal, crash reports, and every counter and figure
  derived from them MUST be received, processed and retained only on
  infrastructure hosted in the European Union. No payload or derivative may be
  transmitted to or stored on infrastructure outside it. Counters and the figures
  drawn from them carry no per-install content and are retained; the reports
  themselves are not, on the deadlines FR-039d sets.
- **FR-040**: Signed-in retention MUST be derived from existing account and wallet
  activity that the service records as originating from an Evreos client, rather
  than from the diagnostic signal, so that members who decline diagnostics are
  still counted in the figure that matters most. The origin marker MUST be a
  client-type field carried on requests that a deliberate member act on an Apivo
  surface initiates, and on no others. Those acts are a closed enumeration:
  signing in, opening the wallet, redeeming a claim code, and following a
  click-out to a merchant. A request the client makes without such an act — a
  wallet or catalogue refresh at launch, a background token renewal, an update
  check, a retry — MUST NOT carry the marker and MUST NOT count towards retention,
  or the criterion measures browser launches rather than members returning. Adding
  an act to that enumeration is an amendment to this specification. The marker
  MUST NOT be a device fingerprint. Without it, a member who uninstalls Evreos and
  keeps using the existing web wallet produces the same account activity as a
  retained member. Both that field and this computation's hosting are changes to a
  service outside this repository, recorded as Q-E14.

**Honesty**

- **FR-037**: Where a capability proves unavailable — protected-media playback is
  the case to plan for, though its availability is unestablished (Q-E11, Q-E11b)
  — the browser MUST say so and offer a hand-off, rather than failing silently.
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
download size, installed size, cold start, shell memory overhead, idle CPU,
chrome input latency — to live in one budget file and be enforced by a CI gate
that fails the build on regression. No budget it names may be un-gated at any
point on the release path. The gates are defined in this preamble and only
here; the criteria below state figures and how each is measured, and none of
them states when a gate blocks.

A **budget entry** is one number, for one criterion, on one platform, under one
stated measurement condition. Every rule in this preamble applies to the entry
rather than to the criterion that states it, because a criterion may state
several: SC-004 states a single number under two entries, tier 1 and tier 2,
whose statuses differ, and SC-002 states two numbers, warm start and cold start,
which are two entries each carrying its own status. Every entry MUST appear in
the budget file FR-043 requires, carrying its figure, its platform, its
measurement condition, its status, its baseline, its declared tolerance and, on
SC-004, its declared cross-check margin.

Each entry's **status** is *ratified*, set by a recorded founder decision, or
*provisional*, standing until such a decision replaces it. Status describes the
figure alone and never whether a gate exists.

Every entry is gated in CI from M0 by three gates, each of which fails the
build:

- The **budget-file gate** compares numbers already in the repository. It needs
  no hardware, no runner and no measurement, so it is unconditional from M0. It
  fails when an entry a criterion below states is missing from the budget file;
  when an entry's recorded baseline is above that entry's stated figure; when a
  status is absent, or an entry recorded as ratified names no founder decision;
  when a declared tolerance or cross-check margin exceeds the limit this
  preamble sets for it; when an upward baseline reset names no recorded founder
  decision; when the pinned runner's identity is absent; or when SC-005's wake
  enumeration is absent or a wake in it lacks a period, a processor-time bound
  or a justifying requirement.
- The **absolute gate** fails the build when the figure measured on the pinned
  benchmark runner exceeds the entry's stated figure.
- The **regression gate** fails the build when the figure measured on that
  runner is worse than the entry's recorded baseline by more than that entry's
  declared tolerance.

The **pinned benchmark runner** is a single machine of stable identity, recorded
in the budget file from M0 by model, operating-system version and a durable
machine identifier. A fungible hosted machine is not one: neighbour noise on
shared cloud machines moves memory and latency by more than a real regression
does. Both measuring gates run on it and block from M0, and every measured
figure is reported against the runner named in the file. Naming the reference
machine models is a separate release prerequisite (Q-E9a): a figure may be
reported as *met on reference hardware*, or published under SC-013, only against
a machine Q-E9a names, because no unnamed machine gives the reproducibility
SC-013 requires. The runner may be one of those machines, and MUST be one once
they are named.

The tolerance is declared per entry in the budget file, is justified by measured
run-to-run variation on the pinned runner, and may not exceed 5% of that entry's
recorded baseline — the number the regression gate compares against, not the
budget's stated figure, so that a baseline well inside its budget does not
inherit a proportionally larger allowance. An entry whose variation exceeds that
is not gateable on the runner it was measured on, and the runner is replaced
rather than the tolerance widened; an undeclared tolerance is zero, not
unbounded.

A baseline may be reset upward only by a recorded founder decision, carried on
that entry in the budget file with its date, the measured byte or millisecond
cost Principle II requires, and the requirement that cost serves. The reset
lands as its own commit and MUST NOT be made in the commit that consumes the
headroom it creates: a baseline an author moves in the change that needs it is
an edit, not a decision. A reset may never place a baseline above the entry's
stated figure, and the budget-file gate fails on one that does. A provisional
figure binds a reset exactly as a ratified one does — a provisional figure is a
ceiling for as long as it stands, which is the whole of its function. Without
upward resets the effective bar would be the best figure ever recorded and no
feature could cost anything, which is not what Principle II says.

A change whose purpose is to establish a figure that does not yet exist — a
spike — is exempt from that one entry's absolute gate and from nothing else. It
never lifts the regression gate, which is what stops a spike being a route
around a baseline, it never lifts the budget-file gate, and it never extends to
another entry. The exemption is recorded on that entry in the budget file,
naming the pull request and the figure it measures, and stays recorded until
that figure lands. A build produced while an exemption is unretired MUST NOT be
released or tagged: the release job refuses an artefact built from a commit
whose budget file records an unretired spike exemption. The exemption is
available only to a change that ships no behaviour, and code reachable in a
shipped binary is behaviour whatever flag guards it — a spike behind a disabled
feature flag is not exempt.

A ratified figure may afterwards only be tightened. Relaxing one requires an
amendment to this specification recording the founder decision, the measured
evidence for it, and what discipline replaces the budget it removes — the
standard the constitution's amendment procedure sets for relaxing a principle.
Principle II permits budgets to move by recorded founder decision and sets
tighter as the default direction; the founder's clarify answer narrowed that to
tighten-only, and this specification keeps the narrower rule. A provisional
figure may be replaced once, by recorded founder decision on spike evidence; it
is ratified from the moment that decision lands, and tighten-only from the same
moment.

SC-003 states a required experience rather than a figure. It is verified by
acceptance test and carries no budget entry and no budget gate, and it names no
budget Principle II names.

- **SC-001** *(both entries ratified)*: The download is 20 MB or less per
  platform — the size of the installer artefact CI publishes — and the installed
  footprint is 60 MB or less per platform. **Installed footprint** is the
  difference in occupied disk space between a clean machine image and that same
  image after installation and after first run has completed, so that anything
  fetched during installation or during first run is counted wherever it lands.
  The boundary is drawn after first run because an installer that measures small
  and fetches its engine on first launch otherwise passes a gate on bytes it
  only postponed. The single exclusion is a web runtime the operating system
  itself provides, which is not shipped and is shared with other applications; a
  runtime Evreos downloads, installs or carries is Evreos's bytes. Member data
  the first run creates — profile, cache and downloads — is excluded, and the
  measurement script published under SC-013 states how each is identified.
- **SC-002** *(both entries provisional)*: With the system web runtime already
  present, an interactive window appears within 800 ms on a **warm start** — a
  launch on an existing profile, after the browser has already run on that
  machine since boot — and within 2 s on a **cold start** — the first launch
  after installation, on a fresh profile, with no Evreos process running and no
  cached profile state on the machine. These are the two names used for these
  two entries everywhere in this specification and in the budget file, and the
  cold-start entry is the cold-start budget Principle II names. The
  runtime-absent path is SC-003's and is not a cold start. Both entries are held
  open deliberately: a large share of each is the engine's own initialisation
  rather than Evreos's code, so each is ratified only after the cold-start spike
  measures that floor on the reference machines. The shell architecture is
  expected to be shaped by what that spike finds.
- **SC-003**: Where the system web runtime is absent, first run presents
  continuous, honest progress and completes without user intervention beyond
  consent. This path is a designed experience and is deliberately not held to
  SC-002.
- **SC-004** *(ratified on tier 1; provisional on tier 2)*: With ten tabs open,
  memory attributable to Evreos stays at or below 150 MB at every 5-second
  sample, from the first tab opening until the session is closed.

  **Attributable to Evreos** is every process Evreos launches or causes to be
  launched on its behalf: the shell, and the web runtime's host, content, network
  and GPU processes started for its windows, and any runtime host it spawns
  itself. The only processes excluded are system daemons that were already
  running before Evreos started and are shared with other applications. The
  boundary is drawn at what Evreos causes to exist rather than at its own
  executable, because a boundary drawn at the executable is passed by moving the
  blocking matcher or the history index into a runtime process the count then
  ignores.

  The gate also records a **whole-machine cross-check**: the machine's total
  committed memory immediately before launch, subtracted from the same total at
  each sample. It fails when that delta exceeds the summed per-process figure by
  more than the cross-check margin declared for this entry in the budget file.
  The margin is declared and justified exactly as a tolerance is, and an
  undeclared margin is zero.

  The measurement runs over a soak of at least 8 hours with the ten tabs left
  open, and the gate reports the maximum sample. A ten-tab leak over hours is
  the failure this budget exists to catch, on a cohort that leaves the browser
  open all day, so the window is not bounded at load time. A shortened run may
  be used for the per-change regression gate and MUST cover at least 60 minutes
  with the ten tabs open. The full 8-hour soak MUST pass on the exact commit a
  release artefact is built from before that artefact is published; a shortened
  run, or a soak of an earlier commit, does not release an artefact.

  The metric is `phys_footprint` (from `task_vm_info`) on macOS and Private
  Bytes — the process's private commit charge,
  `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage` — on Windows, summed over the
  processes above. Both count private memory the process has charged whether or
  not it is resident, so neither is reduced by the operating system reclaiming
  idle pages; a resident-set counter such as Working Set — Private would report
  the eight-hour leak this criterion exists to catch as a reduction. Memory the
  shell places in sections shared between its own processes MUST be counted once
  rather than dropped, or the budget is passed by relocating bytes; the sampling
  script published under SC-013 states how. The two counters are different
  quantities, so figures MUST NOT be compared across platforms. Proportional
  apportionment of shared pages is not used: it is a Linux `smaps` construct, and
  the nearest Windows equivalent caps its share count at 7. The exact counters
  and the sampling script are published under SC-013. The tier-2 entry is
  provisional because ADR-0001 records that what governs macOS memory at ten tabs
  is unestablished and belongs to the spikes.
- **SC-005** *(ratified)*: When idle, with background tabs suspended, processor
  use attributable to Evreos — the same processes SC-004 counts — stays below
  0.5% of one core across a window of at least 60 minutes, which at that scale is
  18 s of processor time, and below 0.5% of one core in every 1-second sample
  that contains no enumerated wake, which at that scale is 5 ms of processor
  time. The gate reports the window figure and the highest wake-free sample.

  Only an enumerated wake may exceed the 1-second bound. Each wake MUST be
  coalesced with the platform's own scheduler, MUST NOT wake the machine from
  sleep, and MUST complete within 50 ms of processor time; the enumerated wakes
  together MUST NOT consume more than 500 ms of processor time in any 60-minute
  window, which is 0.014% of one core across that window and allows at most ten
  50 ms wakes in an hour. Both allowances sit inside the 18 s window figure,
  which no wake is exempt from. Work that needs more than this is a change to the
  budget file, not an exception to this criterion.

  Every scheduled wake on the idle path MUST be enumerated in the budget file
  with its period, its processor-time bound and its justifying requirement;
  adding one is a change to that file and states its cost under FR-043. At the
  time of writing the enumeration is the update check under FR-014, the
  blocking-list refresh FR-008 depends on, and — only where the member has
  enabled diagnostics — the retention evaluation under FR-039a. None of the three
  is required to BE a scheduled wake: FR-039a evaluates whenever the browser
  runs, so an implementation with no timer for it enumerates none. The
  enumeration lives in the budget file rather than in this criterion so that a
  wake is added only by a change that states its cost, and so that a requirement
  whose feature is off by default is not forced to carry a timer it does not
  have. No periodic timer outside the enumeration may exist on the idle path,
  verified by design review and by instrumentation of scheduled work rather than
  by observation, since no finite window can falsify a timer with a longer
  period.
- **SC-006** *(ratified)*: Switching tabs and typing in the address field produce
  a visible response within 16 ms, measured on a display driven at 60 Hz on each
  named reference machine, at the 99th percentile of at least 1000 trials per
  interaction, and no trial may exceed 16 ms at all — a single trial over 16 ms
  fails the gate, since the base criterion admitted none and dropping a frame
  once every hundred interactions is perceptible on tab switching. A whole gate
  invocation — the full set of trials for one interaction — MUST be discarded and
  re-run only for a recorded, externally observable cause the harness detects: a
  competing process, thermal throttling, a failed instrumentation check.
  Individual trials MUST NOT be discarded for any reason, since discarding the two
  worst of a thousand is a 99.8th-percentile bar and reinstates the dropped frame
  this criterion forbids. Never discard for an outlier judged environmental after
  the fact, which with unlimited retries passes any hard maximum with probability
  one. Every discard and its cause MUST be published under SC-013 against the
  commit it was taken on. The discard budget is two invocation-level discards per
  head commit, counted cumulatively across every run of the gate on that commit:
  re-running the gate does not reset the count, and the discards of each re-run
  are added to it. The third discard on a commit fails the gate, and only a new
  head commit — measured again from zero — carries a new budget. The bar stays
  16 ms where a machine's native refresh is higher: this is a human-perception
  budget, not a hardware-relative one, and a hardware-relative bar is not
  reproducible under SC-013 — on a 30 Hz panel one frame is 33 ms, twice the
  budget.

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

- **SC-010**: At least 25% of the people offered Evreos at the pilot counter
  install it and complete a claim. The denominator is the count of serialised
  claim codes the service records as issued to that counter, less the codes
  returned unredeemed at the end of the pilot and reconciled against that
  issuance record by the founder rather than by the counter. The numerator is
  the count of distinct issued codes that reach a completed redemption under
  FR-029. A tally of conversations kept by the counter is not the denominator:
  the party this criterion judges may not also supply the number it is judged
  on. Both counts are published beside the figure. Whether FR-029 redemption
  exists to be counted at all rests on Q-E11a.
- **SC-011**: Thirty-day retention is measured two ways and reported separately,
  never as one blended figure.

  **Signed-in retention** is the share of members whose first Evreos-originated
  sign-in falls in a given ISO cohort week and who record, on any day from 24 to
  30 after that first sign-in, at least one further account request of a kind
  FR-040 qualifies as member-initiated. A request the client issues on its own —
  a wallet refresh at launch among them — does not qualify, whatever it carries,
  or the figure counts the client's own timer rather than the member. The clock
  is the sign-in, not the install: the service has no install date, FR-040
  authorises no such datum, and Principle VI closes the substitutes. The figure
  is **provisional at 40%**, replaced once by recorded founder decision on the
  first two full cohorts and tighten-only thereafter — the same instrument
  SC-002's entries use. 40% is a placeholder rather than a measurement, and
  Q-E15 records the ratified bar as a founder decision. It is set against
  members who sign in rather than against everyone who installs, because those
  are different populations: members who sign in are a self-selected minority
  whose retention is expected to be materially higher, and 25 retained out of
  100 sign-ins drawn from 1,000 installs clears a 20% bar on sign-ins while
  missing the same bar on installs by a factor of eight. Stating no threshold at
  all would leave the pilot's stated job — month-two retention — with no
  criterion the pilot could fail. It is derived from server-side account
  activity, which is transactional rather than diagnostic and needs no opt-in,
  and it is measured after release rather than at acceptance, so it gates no
  build.

  **Signed-out retention** is reported alongside it over weekly enrolment
  cohorts (FR-039a), which are not the same cohorts as the signed-in figure and
  MUST NOT be compared item to item. It is a self-selected sample and MUST be
  labelled as such wherever it is reported, and it carries no bar, being reported
  for direction. Its opt-in rate is estimated against an aggregate count of
  active installs derived from the update channel, **once a requirement governs
  what that channel may send and retain** — issue #28 records that gap and this
  criterion must not outrun it. Until then the rate is unknown and MUST be stated
  as unknown beside the figure. Note the count is of distinct installs, not of
  requests: an unidentified request stream cannot distinguish one install
  checking twenty-four times from twenty-four installs checking once, so the
  mechanism is not a free property of FR-014 and must be specified where the
  channel is.

  **Small cohorts.** The signed-out figure is governed by FR-039e, which holds
  anything derived from a counter below 50 reports in any form: such a cohort is
  withheld, not published with an uncertainty band around it. The signed-in
  figure is drawn from account activity rather than from a diagnostic counter, so
  FR-039e does not reach it; there, a cohort of fewer than 200 first sign-ins is
  published with its cohort size and labelled direction-only, and the 40% bar is
  judged only on cohorts of at least 200.
- **SC-012**: Active members average at least one cashback activation per
  calendar month. The denominator is the members with at least one
  Evreos-originated account request in that month of a kind FR-040 qualifies as
  member-initiated, counted whether or not they activated anything; the numerator
  is the click-outs FR-025 records for those members in the same month. Members
  with no activation stay in the denominator, because a population defined by
  having activated makes the criterion true whatever the product does. Both
  counts are published beside the figure.

**Trust**

- **SC-013**: The benchmark methodology, the scripts, the figures and the gate's
  run record are published, and a third party rerunning them on a machine of the
  reference class Q-E9a names obtains the same figures. **The same figures** means,
  for each entry, a result within that entry's declared tolerance of the published
  figure and reaching the same pass-or-fail verdict against the entry's stated
  figure; a reproduction outside that band fails this criterion and is recorded as
  a failure of it rather than as variation. The run record carries the pinned
  runner's identity, the measurement conditions, SC-004's sampling script and
  cross-check margin, and every discarded gate invocation with its cause and its
  commit (SC-006). The traffic capture SC-014 requires is published here too.
- **SC-014**: No browsing history leaves the machine, and diagnostic reporting is
  off until the member turns it on. This is established by a published capture
  rather than by inspection: a scripted session on a fresh profile — first run, a
  search, ten navigations across sites, a download, a private window, then close
  and reopen — is run with all outbound traffic from the machine captured and with
  transport encryption terminated at the harness so payloads are readable, and the
  capture, the script and the analysis are published under SC-013. The criterion
  is met only where every URL-bearing payload in that capture is one FR-007a
  permits — inherent in a function the member invoked on that occasion, carrying
  no more than that function needs — and each is listed in the published analysis
  with the invoking function named. Any URL-bearing payload the analysis does not
  list, and any diagnostic report at all on a profile where diagnostics were never
  enabled, fails the criterion. The capture is rerun on the exact commit a release
  artefact is built from.
## Non-Goals

Carried from the master prompt: compatibility with existing browser extension
ecosystems; a built-in password manager; iOS; synchronisation across devices; a
virtual private network; crypto or web3 surfaces; and assistant sidebars.

Permanently excluded by the constitution, each with the requirement that
enforces it. The three Permanent Prohibitions: advert injection (FR-018b);
silent affiliate attribution (FR-030, FR-033); and server-side collection of
browsing history (FR-007a). Principle III separately and permanently rejects
Electron, CEF and any bundled Chromium, in any release on any platform
(FR-044), while leaving room for a pure Rust engine as an experimental third
backend once one becomes daily-drivable — so what that principle rejects is
that family of bundled engines, not a future engine as such. Listing them here
bounds scope; the requirements are what a build can be held to.

Added on the evidence recorded in ADR-0001, and stated here so it never reaches
a landing page:

- **Hosting third-party browser extensions inside Evreos.** Available only on
  the tier-1 platform, without interface surfaces and by manual installation;
  absent at the tier-2 floor — the platform's extension API arrives only in
  macOS 15.4, two major versions above the macOS 13 floor, so there is no
  extension hosting there to restrict; unavailable on the deferred platform. It
  cannot be offered consistently and so is not offered.

## Unestablished scope (neither claimed nor excluded)

Content-protected playback is unestablished on both tiers: not claimed and not
excluded, pending ADR-0001 risk 8 (Q-E11, Q-E11b). The FR-037 hand-off is built
regardless, as cheap insurance. ADR-0001 forbids the exclusion being described
to anyone until that risk is retired, so nothing in this section is a non-goal,
and nothing in it may be stated as a limitation on the distribution page FR-041
governs.

- **Playback of content-protected streaming media.** The system most streaming
  services require is not in open-source Chromium and is licensed per vendor, so
  a commercial wall stands whichever engine strategy is chosen; whether this
  architecture adds a technical wall on top of it is unestablished, and ADR-0001
  forbids asserting that it does until the mechanism is measured. PlayReady is
  reported reachable through EME at the software security level in a WinUI2/UWP
  WebView2 host. Whether it reaches the Win32 host Evreos ships is untested, as
  is whether it covers the services members use and at which security level
  (Q-E11). Tier-1 content protection is therefore unestablished: no capability
  and no exclusion is asserted. On tier 2, whether the platform's own system is
  reachable to a third-party host is equally unestablished (Q-E11b), and no
  service is yet identified as depending on it. The German public broadcasters
  need no content protection on the paths independent clients use. Wherever
  playback does fail, FR-037's hand-off applies.

## Platform Scope

- **Tier 1 — Windows.** Release criteria apply in full.
- **Tier 2 — macOS 13 and later.** Because the engine is the operating system's
  own, this floor sets the rendering engine version, the web features available
  on this platform, and the security patch source. The platform's extension API
  does not exist at this floor — it arrives in macOS 15.4 — so extension
  hosting is absent rather than limited; hosting third-party extensions is a
  non-goal regardless. The floor puts the macOS-14-and-later proxy route
  ADR-0001 records out of reach as a baseline; it does not put compiled rule
  lists out of reach — Apple documents `WKContentRuleList` as introduced in
  macOS 10.13, five major releases below the floor. What is unbound is `wry`,
  not the platform: `wry` binds neither `WKContentRuleList` nor
  `WebKitUserContentFilterStore`, so at this floor blocking parity depends on
  reaching WebKit's compiled rule lists outside `wry`, alongside top-level
  navigation gating. Whether parity is achievable there, and what that binding
  costs, is unmeasured. FR-008 is a P1 acceptance scenario, so this consequence
  of the floor is tracked as Q-E12 rather than left to be discovered.
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
- **Q-E8** *Superseded by Q-E13.*
- **Q-E9** *Settled 2026-08-30, in part*: SC-001, SC-005, SC-006 and SC-004 on
  tier 1 are ratified as tighten-only figures. Two figures stay provisional and
  open — SC-002's, pending the cold-start spike, and SC-004's on tier 2,
  pending the spike ADR-0001 requires into what governs macOS memory at ten
  tabs. The gate structure is defined in the Success Criteria preamble, not
  here.
- **Q-E9a** Which exact machine models are the reference hardware. This is not a
  presentational question. The Success Criteria preamble sets the conditions
  under which an absolute budget gate on a hardware-dependent figure blocks the
  build, and naming the machines is one of them; that preamble governs, and this
  entry does not restate it. A figure measured on an unnamed machine is not
  reproducible by a third party, which SC-013 requires, and a budget whose
  absolute gate never blocks is un-gated, which Principle II does not admit — so
  answering this is a release prerequisite rather than a preference. The
  Assumptions entry on reference hardware states what the answer must record and
  by when.
- **Q-E10** Whether affiliate attribution survives tracking prevention on the
  tier-2 platform. Recorded in ADR-0001 as unverified and as the only
  identified risk that can invalidate the business rather than the
  architecture.
- **Q-E11** Whether PlayReady reaches the Win32 WebView2 host Evreos ships —
  the one located positive report comes from a WinUI2/UWP host — and, if it
  does, whether it covers any streaming service members actually use and at
  which security level, since commercial streamers commonly gate higher
  resolutions behind a hardware tier. ADR-0001 risk 8 carries the measurement.
- **Q-E11a** Whether the existing service serves claim-code redemption in v1 —
  holding campaign records and accepting a redemption. FR-029 ships on the
  assumption that it does, Principle V forbids the client producing either, and
  SC-010 rests on the answer.
- **Q-E11b** Whether a third-party host of the tier-2 platform's web view
  reaches that platform's content-protection system through EME at all, and
  which services members use depend on it. ADR-0001 records both as
  unestablished and identifies no service as depending on that system, so the
  spike must establish the demand as well as the capability.
- **Q-E12** Whether FR-008 can be met on macOS 13 without the macOS-14 proxy
  route ADR-0001 records, by binding WebKit's compiled rule lists outside `wry`,
  or whether the tier-2 floor must move to macOS 14. Blocking parity on tier 2
  MUST be measured before the floor is treated as settled.
- **Q-E13** Whether a partner-branded distribution ships in v1. The rebrandable
  seam itself is not open: Principle VIII requires it and requires a fixture
  brand to build in CI on every change, regardless of whether any partner build
  is promised (FR-042).
- **Q-E16** Whether crash reporting ships in v1 at all. FR-039d requires
  signature-level aggregation with no individual retention; if that is
  insufficient to diagnose crashes, the alternative is cutting crash reporting
  from v1, because this specification grants no exception to Principle VI.
- **Q-E15** What threshold signed-in 30-day retention must clear. The base
  criterion set 20% over everyone who installs; that population is unmeasurable
  (SC-011), and the measurable population — members who sign in — is a
  self-selected minority whose retention is expected to be materially higher.
  The number does not carry across.
- **Q-E14** Whether the existing service will record a client-type field on
  member-initiated requests, and run the retention computation on EU-hosted
  infrastructure, as FR-040 requires. Both are changes to a service outside this
  repository, and SC-011's signed-in figure rests on them.

## Assumptions

- **Milestone M0** is the first milestone at which the shell builds and runs in
  CI; it is the point from which Principle II's budget gates are required.
- **Cohort week** is an ISO-8601 week, Monday to Sunday, in UTC, wherever a week
  is named.
- **Reference hardware** for SC-002, SC-004, SC-005 and SC-006 is a 2020
  mid-range x86 laptop and an M1-class portable. Q-E9a is answered by recording,
  in the budget file FR-043 names, one machine per tier by model,
  operating-system version and configuration, on a recorded founder decision.
  That record MUST land before any hardware-dependent figure is ratified, and in
  any case before the first release. Until it does, no hardware-dependent figure
  is reproducible under SC-013.
- **Import sources** are Chrome, Firefox and Edge, covering bookmarks and
  history. Site credentials are out of scope; Q-E5 records that decision and
  what reopens it.
- **The pilot launches on existing web surfaces**, not on Evreos. Evreos's job
  for the pilot cohort is month-two retention, not day-one delivery.
- **Partner-facing campaign administration ships disabled** (FR-029a), because
  its backing service is a decision not yet taken outside this repository.
  Member-facing claim-code redemption (FR-029) is a distinct flow, is unaffected
  by that decision, and ships in v1.
- **Money state originates entirely from the existing service.** This
  specification adds no money logic and assumes the ledger's vocabulary of
  pending, confirmed, declined and reversed.
- **The reader app consumes the existing publication**, rather than
  reimplementing it.
- **The SC-001 measurement boundary** excludes any system-provided web runtime,
  since it is not shipped and is shared with other applications.
