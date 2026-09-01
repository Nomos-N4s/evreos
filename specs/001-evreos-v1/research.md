# Research: Evreos v1 (Phase 0)

**Feature** `specs/001-evreos-v1` — **Date** 2026-08-31 — **Status** Phase 0
output

**Inputs, in precedence order**: the constitution (v1.1.0), which supersedes
everything; the specification (61 FRs, 15 SCs); ADR-0001 (Accepted), cited
rather than restated. **Already built, and planned on rather than re-proposed**:
the `Engine` seam and its headless second implementation (`feat/engine-seam`);
`budgets.toml` and `scripts/check-budgets.py` (`feat/budget-gate`).

## How to read this

Entries are grouped by subject, not by who found what. Each gives a
**Decision**, its rationale, the **alternatives rejected**, and a closing
*confidence · source*.

**Established** — verified against a named primary source: pinned-version source
code, official documentation, or a file in this repository. **Indicative** —
reasoned from established facts; may be acted on, may not be published as a
finding. **Unestablished** — carried in §12 with the measurement or decision
that would settle it. Nothing here resolves an unestablished thing by assertion.

Repository facts were read directly. Third-party code and platform documentation
are cited at the versions and paths each entry names; where a source could not
be reached the entry says so and the claim is a question, not a finding. Where
ADR-0001 settles something it is cited and not re-evidenced; where it records
something as unestablished it stays so. Two entries (§1.4, §2.2) *narrow*
ADR-0001 against source read at a pinned version; both say so, and neither
changes its conclusion.

**[GAP]** marks something this design needs that the specification does not
require. Gaps are proposals, collected in §11; none may be cited as though the
specification carried it.

The specification carries four identified spikes — Q-E10, Q-E11, Q-E11b, Q-E12 —
plus two named where their figures are: the cold-start spike (SC-002) and the
macOS-memory-at-ten-tabs spike (SC-004 tier 2 / ADR-0001 risk 9). ADR-0001
additionally names **spike S4**, the windowing and chrome-rendering decision; it
names S4 twice and enumerates no S1–S3 anywhere in that record, so S4 is carried
here by its content rather than by a list it can be resolved against. New
measurements this research opens are **N1–N12** (§12.2) and are stated as new.

---

## 1. The rendering seam

Everything in §1 points one way: **the merged trait must change before the first
real backend is written.** Each change is cheap now against the headless
implementation and expensive later against unsafe COM and objc2 code. FR-044
requires the second implementation kept working from M0, so every seam change
and its headless counterpart are one commit.

**1.1 Replace synchronous `load` with an asynchronous start plus an event
stream.** Neither shipping backend can produce a navigation outcome
synchronously, and both are UI-thread-affine: `wry::WebView::load_url` returns
as soon as navigation has *begun* (wry 0.56.1 `src/lib.rs:2213`); webview2-com
0.38.2 states the UI-thread model in its own doc comment (`src/lib.rs:52-54`);
wry's macOS delegate classes are `#[thread_kind = MainThreadOnly]`. A
synchronous `load` therefore has exactly one implementation route — block the UI
thread in a nested message loop. webview2-com ships one, `wait_with_pump`
(`src/lib.rs:60-79`), whose own documentation scopes it to waiting *before
starting the main message loop*; used in steady state it dispatches input and
paint callbacks re-entrantly for the length of a page load while the shell's
state machine sits suspended mid-call. SC-006 admits no trial over 16 ms at all,
so that breaches it by construction, not by bad luck. *Rejected*: keep sync and
pump (SC-006, re-entrancy); run the engine on a worker thread and block the
shell (WKWebView is main-thread-only and WebView2 UI-thread-affine — there is no
thread to move it to); a future plus `block_on` (identical, with more
machinery); return a synthetic `Page` at navigation start (FR-015's named
defect, reporting a load that produced no page as success); let each backend
paper over it (it makes the headless implementation the only one that can honour
the contract, inverting the purpose Principle III gives it). *Established · wry
0.56.1; webview2-com 0.38.2; SC-006, FR-015.*

**1.2 A navigation id plus a closed event enum — Started, Committed, Redirected,
Succeeded, Failed, TitleChanged, NavigatedAway — with `LoadError` unchanged
inside `Failed`.** Four things `Result<Page, LoadError>` cannot express, each
load-bearing on a merged requirement. Engine-initiated navigation (links,
script, form posts, redirects) produces no `load` call, so `current()` never
updates where the shell observes it — and `Page::address`'s own doc comment in
this repository says "showing the request while displaying the response is how
an address bar lies", which the trait as written makes unavoidable. Title
arrives on its own event: wry registers `add_DocumentTitleChanged` and
`add_NavigationCompleted` as separate handlers with no ordering between them
(`src/webview2/mod.rs:688`, `:724`), so one returned `Page{address,title}`
conflates two events and will routinely carry a stale or empty title. There is
no in-flight state, so SC-009's second clause — zero loading indicators
unresolved within 30 s — is not testable against this trait at all, and the
headless engine cannot even script a load that never resolves. And there is no
request-to-outcome correlation, so an outcome for a navigation the member
abandoned cannot be told from the current one. *Rejected*: add `poll()` and keep
`load` synchronous (two paths to one state, and the synchronous one is still
unimplementable); widen `LoadError` with `Timeout`/`Stalled` (a stalled load is
an absence of an event, not a cause, and modelling it as an error moves a
timeout policy into the engine that FR-015 and SC-009 place with the shell); a
construction-time callback rather than a drained queue (viable, and closer to
both backends, which are already callback-driven — a queue is easier for the
headless implementation and for tests; either is acceptable, the closed event
enum is the part that matters). *Established ·
`feat/engine-seam:crates/evreos-engine/src/lib.rs`; wry 0.56.1; SC-009, FR-015.*

**1.3 `Intercepted` is produced by shell-level classification and never
synthesised by a backend; the other three causes map from platform signals.**
Three of FR-015's four causes are distinguishable from each platform's own API
on both shipping tiers; the fourth on neither. Windows, exhaustively:
`COREWEBVIEW2_WEB_ERROR_STATUS` has 19 values (webview2-com-sys 0.38.2
`src/bindings.rs:886-923`, count verified) — `HOST_NAME_NOT_RESOLVED`(13) gives
`Unresolvable`; values 1–5 (common name incorrect, expired, client-certificate
errors, revoked, invalid) give `Certificate` and populate its `detail`; 17 and
18 (credentials required, proxy authentication required) give
`AuthenticationRequired`, corroborated by `add_BasicAuthenticationRequested`
(`:39380`) and `HttpStatusCode` (`:23418`); that is eight of the nineteen, and
none of the remaining eleven denotes interception. macOS:
`NSURLErrorCannotFindHost`(-1003) and `DNSLookupFailed`(-1006);
`SecureConnectionFailed`(-1200) through `ClientCertificateRejected`(-1205);
`UserAuthenticationRequired`(-1013) with
`webView:didReceiveAuthenticationChallenge:` — again nothing denoting
interception. A captive portal is not an error on either platform: it answers,
so the navigation succeeds. Detecting it is a shell-level inference, and any
probe-based inference is an outbound request Principle VI and FR-007a govern,
which makes it a founder decision rather than a backend detail (N3). Deleting
the variant is not available either — FR-015 names four causes and SC-009
requires four exercised, so removal is a specification amendment; scoping its
production instead keeps SC-009's fourth case testable by the headless engine
today. Carrying ADR-0001's Windows case forward and confirming its mechanism at
a pinned version: wry raises `PageLoadEvent::Finished` from
`add_NavigationCompleted` with a closure written `|webview, _|`, discarding the
args that carry `IsSuccess` and `WebErrorStatus`
(`src/webview2/mod.rs:724-733`), so a Windows backend built on wry's page-load
handler alone reports all nineteen statuses as success — the exact defect FR-015
names. *Rejected*: backend-synthesised `Intercepted` (nothing in either API
supports the synthesis, so the variant would be guesswork differing per
platform, which is the indistinguishable-cause state FR-015 exists to forbid);
delete the variant (premature — an amendment, not a refactor). *Established for
the three mappable causes; that neither enumeration contains a value denoting
interception is read off those two enumerations rather than measured, so the
fourth cause's non-derivability is indicative and N3 is the measurement that
would settle it · webview2-com-sys 0.38.2; objc2-foundation 0.3.2
`NSURLError.rs:113-165`; wry 0.56.1; ADR-0001 accepted costs; FR-015, SC-009.*

**1.4 A host/factory seam above `Engine` owning the shared platform context.**
ADR-0001 records as an accepted cost that "environment sharing must be an
explicit requirement of the `Engine` trait, on Windows" — and it is not in the
trait: there is no construction seam at all, and `HeadlessEngine::new()` is a
free constructor. The mechanism is per-view-by-default on *both* tiers, which
generalises the ADR's Windows scoping: wry uses a passed-in environment if
`with_environment` supplied one and otherwise calls `create_environment` per
webview (`src/webview2/mod.rs:133-136`; builder at `src/lib.rs:1859`), and uses
a passed-in `WKWebViewConfiguration` if `with_webview_configuration` supplied
one, otherwise constructing a fresh one per webview and, when shared, reusing
that configuration's `websiteDataStore` (`src/wkwebview/mod.rs:210-247`). Ten
tabs each minting their own context loses SC-004 before any product code exists,
and the interface as merged cannot say otherwise. This does not reach ADR-0001
risk 9: that a shared configuration shares a *data store* is established; that
it shares a web content *process* is not, and stays the SC-004 tier-2 spike.
*Rejected*: leave sharing to backend internals (it makes a property SC-004
depends on invisible at the seam and untestable by the headless implementation —
exactly the class of assumption Principle III's second implementation exists to
convert into fact); a `shared_context` parameter on `load` (the context is
needed at construction, not at navigation). *Established for the mechanism;
ADR-0001 risk 9 unchanged · wry 0.56.1; SC-004.*

**1.5 An addressable rendering-surface handle — create, activate, suspend, close
— plus a data-store selector, before any tab work starts.** The merged trait is
single-surface: one `load`, one `current()`, one page per engine. FR-001 needs N
independently addressable contexts with reorder and restore; FR-002 needs
per-context suspend and resume; FR-007 needs a context whose data store is
non-persistent and separate; FR-016 needs an app surface alongside tabs.
Retrofitting after tabs exist means rewriting tabs. wry's `build_as_child`
creates a real child view per tab — a child HWND on Windows, an NSView subview
on macOS, with no caveat on either at 0.56.1 (`src/lib.rs:1551-1556`; the
X11-only warning concerns deferred Linux) — so a tab switch is
`set_visible`/`set_bounds` on two native views rather than a re-navigation, and
SC-006's 16 ms is reachable in principle. *Rejected*: one `Engine` per tab with
the trait left single-surface (it puts environment sharing outside the seam,
where ADR-0001 says it must not be, and gives the shell no way to express
"suspend the nine tabs that are not visible"); a tab abstraction above the seam
owning webviews directly (it reaches past the trait to a concrete backend — the
failure the seam exists to prevent). *Established · `feat/engine-seam`; wry
0.56.1; FR-001, FR-002, FR-007, FR-016, FR-044.*

**1.6 Blocking is a policy surface on the seam — install or replace a compiled
policy, exempt a site, report what was blocked — not a per-request veto.** The
tiers enforce in structurally different places: on tier 1 the decision can be
made per request in the host process; on tier 2 at the macOS 13 floor it is made
*inside* WebKit from a precompiled rule list, and the shell never sees the
request. A per-request `should_block` is therefore implementable on exactly one
of the two backends — the seam failure `evreos-engine`'s own doc comment warns
against ("a seam that leaks its default implementation's vocabulary is a seam
only until the second implementation arrives"). The reporting half is not
decoration: FR-008's per-site control must be discoverable at the moment of
breakage (Edge Cases), which needs a per-page blocked count in the chrome, and
ADR-0001 risk 5 wants blocking parity measured as a CI gate, which needs the
same observation exposed to tests. *Rejected*: blocking above the seam with the
shell proxying page traffic (it terminates TLS for page loads, contradicting
FR-007a's certificate-status entry and FR-015's certificate failure state, and
puts a second network stack inside SC-001); blocking below the seam, invisible
to the shell (FR-008's control and the parity gate both need shell-visible
state). *Established · `feat/engine-seam`; ADR-0001 capability floor and risk 5;
FR-008.*

**1.7 Four further seam additions the app and money surfaces require.** (a) Host
a surface from shell-supplied bytes under a shell-chosen surface identity, so no
scheme or protocol vocabulary leaks into the trait — FR-019a requires
verification before rendering or writing to the FR-020 cache, so bytes must
reach the engine from the shell rather than over the wire. (b) A navigation
observation carrying an **epoch** that increments on every change to the
address, same-document navigation included — FR-018a defines a navigation that
way and scopes an occasion to one page load, and the same epoch bounds click-out
completion and offer-control lifetime. (c) A message channel whose messages
arrive tagged with the app identity the shell assigned, because FR-018 keys
page-adjacent grants to an app and checks must key off an identity the engine
cannot forge. (d) A request-gating hook, deny-all for surface webviews and
FR-008's pipeline for page webviews. No requirement states that a surface may
never originate its own network traffic: FR-023 governs only where account
credentials are held — "the operating system's secure credential store … and
nowhere else" — and FR-007a binds what may be transmitted rather than which
component opens the socket. The confinement is this design's proposal, carried
as a gap at §6.4 (G15); what the requirements supply is the argument for it,
FR-007a's conformance capture having to fail on any outbound request no entry
accounts for, and FR-040's client-type marker being enforceable only where one
code path issues requests. `LoadError` stays the closed four-cause enum; none of
these adds a failure cause. *Indicative — the requirements are established, the
seam shapes are proposals, and (d)'s confinement is a gap (G15) · FR-018,
FR-018a, FR-019a, FR-020, FR-023, FR-007a, FR-040.*

**1.8 No `Send` bound anywhere on the engine path; a single-threaded UI event
loop with a thread-safe wake and a small worker pool; no `block_on` on the UI
thread and no async runtime owning the loop.** The engine cannot leave the UI
thread (§1.1), so `Send` is unimplementable on both tiers and is cheaper to
forbid now than to unwind later. A runtime cannot own the loop either: on
Windows it must be a Win32 message pump and on macOS an NSApplication run loop,
neither of which is a tokio reactor, so a runtime becomes the nested-loop
problem again — and it costs bytes against SC-001's ratified 20 MB for
scheduling the loop's own thread-safe proxy already provides. What SC-006
actually requires is narrower than it looks: the risk is not the tab switch
(§1.5) but anything done on the UI thread while the member types — address-field
suggestion lookups against the history index, blocking-list compilation, session
writes — all of which move to worker threads and return by posting a user event.
*Rejected*: tokio on a current-thread runtime driving the loop (above); tokio on
worker threads for the shell's own I/O (defensible but not yet justified — v1's
off-thread work is a handful of blocking file and index operations, which
`std::thread` plus a channel covers; revisit if FR-014's update client or the
Apivo surfaces need concurrent HTTP, stating the byte cost as Principle II and
FR-043 require). *UI-thread affinity established; worker-pool sufficiency for
SC-006 indicative (N7) · wry 0.56.1; webview2-com 0.38.2; SC-001, SC-006.*

**1.9 A conformance battery both implementations run** — `#[cfg(feature =
"conformance")] pub fn conformance_suite<E: Engine>(..)` inside `evreos-engine`,
a module rather than a crate. Today the only tests are
`crates/evreos-shell/tests/navigation_failures.rs`, and they exercise the
headless engine alone, so nothing forces the two implementations to agree: as it
stands the second implementation proves the seam *compiles* against two
backends, not that it *means* the same thing to both, which is what Principle
III asks it to prove and FR-044 requires kept working from M0. It is also the
only check the tier-2 delegate replacement (§2.2) has to be measured against,
which is the argument for it preceding that work. *Rejected*: a separate
`evreos-engine-testkit` crate (a crate for the same reason a battery is not one;
revisit only if it needs dependencies `evreos-engine` must not carry).
*Established · `feat/engine-seam`; Principle III; FR-044.*

---

## 2. Platform backends, crates, and the unsafe boundary

**2.1 Tier 1 (Windows): an additive wrapper over the handles wry exposes, and no
fork.** Register Evreos's own `add_NavigationCompleted` (reading
`IsSuccess`/`WebErrorStatus`), `add_ServerCertificateErrorDetected` and
`add_BasicAuthenticationRequested` beside wry's. `WebViewExtWindows` returns
`ICoreWebView2`, `ICoreWebView2Controller` and `ICoreWebView2Environment` (wry
0.56.1 `src/lib.rs:2340-2375`), and WebView2 registrations are additive
`add_*`/token pairs — ADR-0001's accepted-cost position, confirmed here at a
pinned version. Every binding needed is already in webview2-com-sys 0.38.2
(`:39974`, `:39380`), which wry already depends on, so this adds no third-party
tree. *Established · wry 0.56.1; webview2-com-sys 0.38.2; ADR-0001.*

**2.2 Tier 2 (macOS): replace the `WKNavigationDelegate` through wry's public
API, and no fork.** `WebViewExtMacOS::webview()` returns `Retained<WryWebView>`
(`src/lib.rs:2492`) and `WryWebView` is declared `#[unsafe(super(WKWebView))]`
(`src/wkwebview/class/wry_web_view.rs:38`), so it *is* a WKWebView and
`setNavigationDelegate:` (objc2-web-kit 0.3.2 `WKWebView.rs:188`) can be called
on it. This narrows ADR-0001, whose accepted-cost bullet scopes "wry exposes the
raw native handles" to Windows and Linux; at wry 0.56.1 macOS exposes one too.
The ADR's conclusion is unaffected and confirmed by source: `navigationDelegate`
is a single-slot property, so installing ours displaces wry's, and what is lost
is exactly what `WryNavigationDelegate` implements — navigation gating
(`decidePolicyForNavigationAction`), response policy, `Started` (raised from
`didCommitNavigation`), `Finished` (`didFinishNavigation`), deferred init-script
injection (flushed in `did_commit_navigation`,
`src/wkwebview/navigation.rs:28-35`), the two `didBecomeDownload` paths, and
`webViewWebContentProcessDidTerminate`: seven behaviours, all reimplementable,
none of them things a browser shell would delegate to wry, with objc2-web-kit
binding all fourteen `WKNavigationDelegate` methods so the replacement is
written rather than bound. The macOS case is also sharper than ADR-0001 states,
in the same direction: because `Started` comes from `didCommitNavigation`, which
a failed provisional navigation never reaches, and neither `didFail` method is
implemented, a failed macOS load under wry's delegate produces **no event at
all** — not merely a load that never terminates. *Rejected*: fork wry
(unnecessary, and it buys the rebase treadmill ADR-0001's rejected-options
section spent its argument avoiding); an intercepting delegate forwarding
unhandled selectors, as the *default* (WKWebView gates each callback on
`respondsToSelector:`, so `forwardingTargetForSelector:` alone is insufficient
and `respondsToSelector:` must be overridden too — more unsafe surface to keep
correct, in exchange for behaviour the shell wants to own regardless; keep it as
the fallback if replacement loses something unanticipated). Upstream the
load-failure and TLS hooks whichever route is taken, as ADR-0001 risk 3 directs.
*Established · wry 0.56.1; objc2-web-kit 0.3.2; ADR-0001 accepted costs and risk
3.*

**2.3 Tier 2 is one "reach WebKit past wry" workstream, sized once.** Compiled
content rule lists (Q-E12), find-in-page, page zoom and the delegate replacement
all exist in WebKit at or below the macOS 13 floor and are all unbound by wry,
so they take the same route and pay the integration cost once:
`findString:withConfiguration:completionHandler:` and `pageZoom` are both
`WK_API_AVAILABLE(macos(11.0))` in WebKit's own header, two major releases below
the floor, and ADR-0001 records `WKContentRuleList` as available since macOS
10.13 and unbound by wry. Scoping Q-E12 to blocking parity alone would price the
tier-2 floor decision on a fraction of its real cost. FR-018a exempts
find-in-page by name, so a script-based find remains a fallback if the native
route proves expensive. *Established · WebKit `WKWebView.h`; ADR-0001; Platform
Scope tier 2, Q-E12, FR-005.*

**2.4 FR-006 cannot be met in full on tier 2 at the macOS 13 floor.** WebKit's
public `WKUIDelegate.h` declares media-capture permission as
`WK_API_AVAILABLE(macos(12.0))` — available at the floor, so camera and
microphone prompts are reachable — but declares geolocation permission as
`WK_API_AVAILABLE(macos(27.0))` and declares no notification-permission delegate
at all, while FR-006 requires prompting per site for all four. This is not a
floor that can be raised past: no floor this product could declare reaches macOS
27. So do not build one four-permission surface from the tier-1 shape: measure
what a page actually experiences at the floor (N9), then route each capability
per tier either to a prompt or to FR-037's hand-off, and let FR-041 carry the
resulting statement to the distribution page. FR-041 forbids asserting a
capability or a limitation that has not been established, and a permission
prompt granting something the engine will not deliver is exactly the failure the
member must diagnose that FR-037 forbids. *Established for API availability;
unestablished for what a page observes (N9) · WebKit `WKUIDelegate.h`; FR-006,
FR-037, FR-041.*

**2.5 FR-002's suspension policy is written per tier, and only tier 1's lever is
verified.** Tier 1 suspends hidden tabs with WebView2's `TrySuspend` and resumes
on activation: Microsoft documents it as pausing script timers and animations,
minimising renderer CPU and letting the OS reclaim renderer memory, requiring
`IsVisible` false, with automatic resume — which matches FR-002's "reversible
without losing the page's state visible to the user" and feeds SC-004 and SC-005
directly. Tier 2 has no verified lever at its floor:
`WKPreferences.inactiveSchedulingPolicy` is macOS 14.0+ and unexposed by wry,
and wry's own `with_background_throttling` is documented Unsupported on Windows
and Supported since macOS 14.0 (`src/lib.rs:1488-1493`), so the policy must be
written from measurement (N5). FR-002 requires a *stated* policy, and one stated
from an API that does not exist at the floor would be a claim about behaviour
nobody measured — if no lever suffices on a tier, FR-002's suspension has no
mechanism there, and that is a finding for the plan rather than a bug to be
found later. *Rejected*: discard and reload background tabs (FR-002 requires
reversal without losing visible page state, and a reload loses form state and
scroll position, which on a bank or government form is the failure that ends the
install); suspend on a timer (`TrySuspend` requires `IsVisible == false`, so
visibility is the gate whatever the timer says). *Established on tier 1;
unestablished on tier 2 (N5) · Microsoft Learn `ICoreWebView2_3.TrySuspend`; wry
0.56.1; FR-002, SC-004, SC-005.*

**2.6 WebView2 environment options are FR-007a compliance, and wry's Windows
defaults are a standing regression risk.** Evreos creates the
`CoreWebView2Environment` itself and hands it to wry with
`IsCustomCrashReportingEnabled = true`, deleting the resulting dumps unread, and
sets `IsReputationCheckingRequired = false` through the raw `ICoreWebView2`
handle on **every** webview before its first navigation; both belong to the
engine-integration crate rather than to diagnostics, and both are asserted in
the SC-014 capture rather than in a unit test. Microsoft's own documentation
states that if any WebView2 process crashes, minidumps are created and sent to
Microsoft, and that custom crash reporting stops that — and a renderer minidump
contains page memory, hence URLs and page content. FR-007a makes a transmission
the system web runtime makes while serving Evreos into Evreos's own, no entry in
its closed list accounts for a crash dump, and FR-039 requires diagnostics off
until the member turns them on, so a default embedding ships a second,
unconsented, URL-bearing crash channel to a third country, failing FR-007a,
FR-039, FR-039f and SC-014 at once. SmartScreen is the concrete instance of the
case FR-007a names in the abstract ("a reputation or address-filtering service
is the case to expect"): Microsoft's API specification records
`IsReputationCheckingRequired` as defaulting to **true**, operating at
browser-process level across all WebView2s sharing a user data folder, enabled
if *any* such WebView2 requires it, and — the trap — resetting to enabled for
all instances when a new WebView2 is created against that folder, taking effect
only on the next navigation or download. Since every tab is a WebView2 sharing
one user data folder, a tab created without the setting re-enables reputation
lookups for the whole browser: this is a per-webview invariant with a test, not
a one-time call at startup. Two wry traps, both visible in its source at `dev`
and both silent: supplying `additional_browser_args` *replaces* wry's defaults
wholesale — the default is
`--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` inside an
`unwrap_or_else`, so any unrelated argument re-enables SmartScreen protection —
and `create_environment` runs only when no environment was supplied, so the
environment Evreos must supply also loses those default arguments plus the
language and scrollbar options and must reproduce them explicitly. Neither is
caught by review, which is why SC-014's capture must run on **every release
build** rather than only at acceptance. *Rejected*: rely on the member's Windows
diagnostic-data setting (Microsoft's own document says developers do not control
overall diagnostic data collection and that required data is collected
regardless, so it is not a control Evreos can point at); rely on wry's default
`--disable-features` argument as the sole mechanism (the first trap); disable
SmartScreen by system setting or policy (it is the member's machine, and the
documentation records that setting as an override in the *off* direction only);
fork wry (the public extension points suffice). *Established ·
MicrosoftDocs/edge-developer `webview2/concepts/data-privacy.md`;
WebView2Feedback `specs/IsSmartScreenRequired.md`; tauri-apps/wry; FR-007a,
FR-039, FR-039f, SC-014.*

**2.7 The windowing crate stays spike S4's output.** This research narrows its
constraint set without pre-empting it: on tiers 1 and 2 wry's
`build`/`build_as_child` are generic over `HasWindowHandle` alone, so both tao
0.36 and winit 0.30 satisfy it, and only deferred Linux constrains further.
Worth recording against the current release: wry 0.56.1 carries `tao` as a
dev-dependency only, plus `tao-macros` as a real dependency on Android alone —
confirming ADR-0001's "wry/tao is not a package deal". *Established · wry 0.56.1
`Cargo.toml`, `src/lib.rs:1543-1576`.*

**2.8 The crate set.** Shipped: `evreos-engine-webview` (one backend crate,
`#[cfg(target_os)]` modules and Cargo target-specific dependency tables);
`evreos-blocking` (platform-free — corpus, `adblock` engine, content-blocking
conversion, exception-closed partitioner, rule-count budget, failure-taxonomy
report); `evreos-net` (the sole egress chokepoint); `evreos-i18n`;
`evreos-chrome` (whatever S4 selects); `evreos-platform` (default-browser
registration, secure credential store, update verification, local rollout
evaluation); `evreos-signing`, `evreos-appreg`, `evreos-capabilities`,
`evreos-surface`, `evreos-money`; `evreos-diag-state`, `evreos-diag-transport`,
`evreos-crash`. Dev-only and never on the release path: `evreos-probe`
(per-platform sampling), `evreos-bench` (trial driver, run record, discard
ledger), the ten-tab corpus with its loopback server, and the signing tool — so
no signing code ships. The conformance battery (§1.9) and the SC-005 timer
facility (§9.5) are modules rather than crates. One backend crate rather than
per-platform crates because Cargo's target-specific dependency tables already
provide the isolation and two manifests would duplicate the shared wry-facing
wrapper; every dependency the backend needs — `wry`, plus `windows`,
`windows-core`, `webview2-com`, `webview2-com-sys` on Windows and `objc2`,
`objc2-foundation`, `objc2-web-kit`, `objc2-app-kit`, `block2` on macOS — is
already in wry 0.56.1's own graph at the versions it pins, so the backend adds
no third-party tree beyond what the engine decision already committed to.
`evreos-blocking` must not name a platform, for the same reason `evreos-engine`
does not; its parsing half is squarely the layer ADR-0001 gives as the reason
for Rust. Dev-only members ship zero bytes, which is what FR-043's
per-pull-request cost statement will say for them and is only true if the
workspace is arranged this way from the start; it also keeps the memory sampler
outside the memory budget it measures. *Rejected*: per-platform backend crates
(above); a general crate such as `sysinfo` instead of `evreos-probe` (it
surfaces resident-set-shaped figures, and SC-004 rejects a resident-set counter
by name); the sampler inside the shell behind a feature flag (the Success
Criteria preamble holds that "code reachable in a shipped binary is behaviour
whatever flag guards it"). *Established · wry 0.56.1;
`feat/engine-seam:Cargo.toml`; FR-043, SC-001.*

**2.9 Exactly one shipped crate holds `unsafe`, and the decision lands with the
first FFI line.** `evreos-engine-webview` opts out of the workspace `unsafe_code
= "forbid"`, sets `#![deny(unsafe_op_in_unsafe_fn)]` and requires a `// SAFETY:`
note on every block; every other shipped crate keeps `forbid`; `evreos-probe` is
the second holder and is dev-only; a CI check asserts that no other crate lifts
the lint. The workspace as merged sets `[workspace.lints.rust] unsafe_code =
"forbid"` and each crate repeats `#![forbid(unsafe_code)]` — verified in this
repository — while every call a backend must make is `pub unsafe fn`
(`ICoreWebView2::Navigate`, `add_NavigationCompleted`,
`add_ServerCertificateErrorDetected`, `setNavigationDelegate`,
`addContentRuleList`), so a real backend is not buildable in this workspace
today. That is a fact about the repository, not a criticism of it: the policy is
right for the other crates, and the exception should be narrow, named and
reviewable — exactly one manifest differing is the property that makes it
reviewable. Retrofitting means arguing about an `allow` that is already in the
tree, which typically ends as a blanket allow on whichever crate hit it first,
silently deleting the policy. **For review:** nothing in the constitution
forbids `unsafe` — Principle III constrains nightly features, not unsafety — so
this is a repository policy decision rather than a constitutional one, and the
pull request that makes it should say so. *Rejected*: relax the lint
workspace-wide to `deny` (it spreads the exception to crates that will never
need it and removes the property that makes it reviewable); split the unsafe
into a `-sys`-style crate below the backend (the unsafe *is* the backend, so the
split produces a boundary with nothing on one side); drive the platform APIs
through an out-of-process helper to keep `forbid` (SC-001 and SC-004, and it
invents an IPC surface the seam exists to avoid). *Established ·
`feat/engine-seam:Cargo.toml`; webview2-com-sys 0.38.2; objc2-web-kit 0.3.2.*

---

## 3. Content blocking (FR-008)

**3.1 Tier 1 enforces in-process from a `WebResourceRequested` handler on the
`ICoreWebView2` handle wry exposes, consulting the `adblock` crate's native
engine**, registered through
`ICoreWebView2_22::AddWebResourceRequestedFilterWithRequestSourceKinds` rather
than the older filter. This route keeps full ABP semantics: no rule is lost in
translation. The SDK carries 17 resource contexts, and the newer call extends
coverage to SHARED_WORKER and SERVICE_WORKER sources where the default filter
covers DOCUMENT sources only; `SetUri` and `SetMethod` are settable, so
`$removeparam` and `$redirect`-to-surrogate are reachable on this tier.
*Rejected*: bundle an MV3 extension loaded through
`ICoreWebView2Profile7::AddBrowserExtension` and match in-engine with no host
round-trip, as the *primary* route — it is Windows-only (ADR-0001 records the
extension API as absent at the macOS 13 floor), so it creates a *third* rule
format rather than removing one; ADR-0001 records WebView2 extension hosting as
UI-less and sideload-only; and it puts a second, engine-shaped delivery
mechanism next to FR-019a/FR-019b. Keep it, and CDP (`Fetch.enable` /
`Network.setBlockedURLs`), named as fallbacks only if coverage measures short
(N2) or the per-request cost breaches SC-005 or SC-006. *Established for the API
surface; unestablished for coverage (N2) · webview2-com-sys 0.39.1
`src/bindings.rs:927-961, :998-1012, :1708, :37971, :37992, :41923`.*

**3.2 Tier 2 compiles the corpus through `adblock`'s `content-blocking`
conversion and attaches it with `WKContentRuleListStore` and
`-addContentRuleList:` through objc2-web-kit inside `evreos-engine-webview` — no
new binding crate, no fork, no macOS-14 proxy. This establishes the route only;
Q-E12's parity measurement is unaffected and nothing here predicts it.**
ADR-0001 risk 11 leaves the *route* undetermined — "through a native handle if
one is exposed on this platform, otherwise a binding or a fork; which route
applies, and its cost, is part of what this risk measures" — and both halves are
already in wry's dependency graph: objc2-web-kit 0.3.x binds
`WKContentRuleListStore::defaultStore` with its compile, look-up and remove
calls, and
`WKUserContentController::addContentRuleList:`/`removeContentRuleList:`/`removeAllContentRuleLists`,
while wry hands over the controller (`WebViewExtMacOS::manager()`,
`src/lib.rs:2494`). ADR-0001's own statement that rule lists attach to the
user-content controller, and so do not depend on the navigation delegate, is
what makes this independent of §2.2's cost. Two limits stated deliberately: this
establishes API availability *in the bindings*, not runtime availability on
macOS 13 — for that this document relies on ADR-0001's citation that Apple
documents `WKContentRuleList` as introduced in macOS 10.13 — and it says nothing
about whether parity is achievable at the ceiling. *Rejected*: the macOS-14
`with_proxy_config` route (unavailable as a baseline at the floor, which is what
makes Q-E12 a question at all, and see §3.7); a dedicated Rust binding for
`WKContentRuleList` (redundant); a wry fork adding a rule-list builder (nothing
needs it, since the controller handle is public — revisit only as an upstream
contribution); top-level navigation gating alone (it cannot block subresources,
where trackers and adverts live, and is useful only as a per-site-control
trigger). *Established for the route; unestablished for parity (Q-E12) ·
objc2-web-kit 0.3.x; wry 0.56.1; adblock-rust 0.13.3; ADR-0001 risk 11.*

**3.3 The 150,000 ceiling is a budget on emitted JSON rule objects per compiled
list, gated in CI on the emitted artefact rather than on source rules.** WebKit
checks the length of the top-level JSON array (`maxRuleCount = 150000`,
returning `JSONTooManyRules`), and the conversion is not one-to-one: `adblock`'s
`CbRuleEquivalent::SplitDocument` emits two content-blocking rules for one ABP
rule whenever a network rule names more than one resource type, one of them
`Document`, with no load type specified. A source-rule count under 150,000
therefore does not imply a compiled list that loads, and the failure is total —
the whole list fails to compile — so it cannot be discovered on a member's
machine. ADR-0001 records the 150,000 figure as verified; this refines the unit
it is counted in. *Rejected*: counting source rules (it undercounts by an
unknown factor); discovering the ceiling at runtime and splitting reactively (a
member-visible failure mode for a number CI can settle at build time).
*Established · WebKit `ContentExtensionParser.cpp:333-334`; adblock-rust
`src/content_blocking.rs:253-261, 582-592`.*

**3.4 Multi-list splitting must be exception-closed: duplicate the full
exception set into every partition, partition only the block rules, and prove it
by test on the emitted JSON.** Verified in WebKit's own evaluator:
`ContentExtensionsBackend::actionsForResourceLoad` maps over
`m_contentExtensions` and calls `actionsFromContentRuleList` once per list;
`ignore-previous-rules` is resolved *inside* that per-list function, truncating
that list's own action vector, and the cross-list combination loop treats
encountering it as unreachable (`RELEASE_ASSERT_NOT_REACHED`), combining by
logical OR so any list's `BlockLoadAction` sets `blockedLoad`. An `@@` exception
compiled into list B therefore cannot cancel a block compiled into list A: split
EasyList into three chunks by line order and every exception whose block landed
in a different chunk silently stops working — precisely the class of breakage
the spec's Edge Case about bank and government sites names, and whose cohort
response is to uninstall. This is a correctness hazard rather than an
inelegance; the duplication cost then becomes a line in the rule budget rather
than a silent failure. *Rejected*: partition by domain scope (most ABP rules are
universal, so it degenerates); accept exception loss and measure it (the losses
land on exactly the sites the specification names as abandonment triggers); one
list under the ceiling by aggressive pruning (a legitimate lever, but the
pruning must then itself be measured for parity, which is the same measurement).
*Established · WebKit `ContentExtensionsBackend.cpp:152-168, 172-200, 280-310`;
FR-008, Edge Cases.*

**3.5 FR-008's per-site control on tier 2 has two candidate implementations,
both carried until measured (N4).** **(A)** Detach the lists from that tab's
`WKUserContentController` — `removeAllContentRuleLists`, re-adding on leaving
the exempt site — triggered from a per-top-level-navigation hook. **(B)** Build
the exemptions into every compiled chunk as `ignore-previous-rules` with
`if_top_url`, recompiling on change. It follows from §3.4 that an exemption
expressed as its own small list cannot cancel blocks in the main lists, so the
obvious implementation is unavailable. (A) is cheap per change but couples the
control to navigation observation — the same delegate §2.2 replaces; (B) is
decoupled but pays a full recompile of a 150k-rule DFA on a member-visible
setting toggle, with `adblock` already exposing `ignore_previous_fp_documents()`
and `CbTrigger::if_top_url`/`unless_top_url`. Which is acceptable is a
measurement, not a choice. *Rejected*: a public per-navigation switch — none
exists, since `WKWebpagePreferences`'s public properties at the floor are
`preferredContentMode`, `allowsContentJavaScript` and `lockdownModeEnabled`, and
WebKit's internal `RuleListFilter` is not surfaced in the public Cocoa API.
*Established for the constraint; unestablished for which candidate is viable
(N4) · WebKit `WKWebpagePreferences.h`; adblock-rust; wry `src/lib.rs:1307`.*

**3.6 The conversion-failure taxonomy is the tier-parity ledger and a committed
CI artefact.** For each pinned list revision, record how many source rules each
`CbRuleCreationFailure` variant dropped, commit the table as a baseline, and
fail CI on unexplained growth, with `RuleContainsNonASCII` on its own line.
`adblock`'s converter returns a typed reason for every rule it cannot express in
Apple's format — redirect, generichide, badfilter, CSP, removeparam, full-regex,
cosmetic entities, cosmetic action rules, scriptlet injections, procedural
cosmetics, non-ASCII, unless-and-if-domain together, `from`, optimized rules, no
supported network options, needs-debug-mode — and that set *is* the
tier-1/tier-2 capability delta expressed as a number rather than an impression.
It is computable on any machine with no macOS present, so it gates on ordinary
CI while the site-by-site parity measurement waits on the tier-2 runner.
Non-ASCII deserves its own line because the German regional list is where such
rules concentrate, and it is the cohort's list. This artefact is also what lets
FR-041's distribution page stay honest about what blocking does on each platform
without asserting anything unmeasured. *Rejected*: measure parity only by
loading real sites (necessary under ADR-0001 risk 5, but slow, flaky and
dependent on the tier-2 runner — the taxonomy gate is the fast half that catches
a list update silently losing four thousand German rules); state parity from a
sample (an assertion). *Established · adblock-rust `enum CbRuleCreationFailure`;
ADR-0001 risk 5.*

**3.7 Collapsing is a second mechanism, needed on both tiers, and it is not a
by-product of network blocking.** Three parts: cosmetic element-hiding rules
from the same corpus; a `display:none` stylesheet applied to the main frame
**and** sub-frames; and a small script that hides elements whose load was
refused. Network blocking removes the byte stream but the reserved layout box
stays — an `<iframe width=300 height=250>` whose source was blocked still
occupies 300×250, which is the "page full of holes" FR-008 names as the reason
members turn blocking off. On tier 2 the WebKit action `CbType::CssDisplayNone`
and WebKit's per-list `globalDisplayNoneStyleSheet` supply the standing half,
though WebKit suppresses that stylesheet for a list that saw
`ignore-previous-rules` — a second reason exemptions must be modelled per list
(§3.5). On tier 1 nothing supplies it, so the cosmetic half must be injected via
`with_initialization_script_for_main_only(js, false)` so sub-frames are covered,
since ad slots are overwhelmingly in sub-frames; the residual case on both
tiers, an element with no cosmetic rule whose request we refused, needs the
collapser script. FR-018a settles the legal question in advance: collapsing "is
not an insertion of content", and content blocking including its collapsing is
named as meeting the three-part exemption test, so this needs no member action.
*Rejected*: serve a 1×1 transparent response so the box collapses naturally (it
does not collapse a sized iframe, and it makes the block invisible to the
blocked-count observation FR-008's control depends on). **Related correction:**
the macOS-14 proxy route is the *thinner* capability, not the benchmark Q-E12 is
measured against — wry documents `with_proxy_config` as supporting HTTP CONNECT
and SOCKSv5 proxies only, and for an HTTPS resource such a proxy sees
destination host and port while paths, query strings and resource types stay
inside the tunnel, whereas filter lists are overwhelmingly path- and
type-scoped; a local terminating proxy with a generated root CA in the member's
trust store is rejected on FR-007a (certificate status becomes ours), FR-015
(certificate failure states become unfaithful) and plain trust grounds.
*Established for the mechanisms; the HTTPS consequence indicative, being
standard protocol behaviour rather than a cited statement about wry ·
adblock-rust; WebKit `ContentExtensionsBackend.cpp:292-296, 353-358`; wry 0.56.1
`src/lib.rs:1011-1035`, `:1455-1461`; FR-008, FR-018a.*

**3.8 Blocking carries four FR-043 budget lines that do not yet exist**: corpus
bytes in the download; the compiled artefact in the installed footprint; the
per-request handler cost on tier 1 against SC-005 and SC-006; and rule-list
compile time at first run, which touches SC-002. SC-001's installed-footprint
entry is the disk delta after first run *excluding member data*, and a compiled
`WKContentRuleList` DFA is not member data — it is product data materialised at
first run in the `WKContentRuleListStore` directory, so it lands inside that
measurement, as does the serialised `adblock` engine on tier 1. FR-008 requires
blocking active on first launch *without configuration*, which forecloses the
usual escape of deferring the corpus to a background fetch. On tier 1,
registering `WebResourceRequested` with `..._CONTEXT_ALL` and a `*` URI puts
every request through our code in the host process, chargeable against SC-005
and SC-006 — and the documented rationale for filters is precisely to avoid that
overhead on unfiltered requests. None of these four numbers is predicted here;
they are measurements the blocking work must produce, and `check-budgets.py`
already reports an unmeasured entry as not a pass, so adding the entries before
measuring is safe and honest. Shipping a reduced corpus to fit is a legitimate
lever but is a parity decision and must be measured as one, never made silently
to pass a gate. *Established · `feat/budget-gate`; SC-001, FR-008, FR-043.*

---

## 4. Privacy enforcement (FR-007a, FR-036a, SC-014)

**4.1 FR-007a splits into a half enforceable by construction and a half
enforceable only by capture. [GAP]** *(i) Shell traffic*: every outbound request
goes through one crate, `evreos-net`, taking a `Purpose` argument from a closed
enum whose history-bearing variants are exactly FR-007a's four entries — page
load, certificate status, submitted search, hand-off — with a CI assertion over
the dependency graph that no other workspace crate depends on an HTTP or socket
API, so adding a variant shows up as a diff in one file, which is what FR-007a
means when it says adding an entry "is an amendment to this specification, made
in the pull request that would add the transmission". *(ii) Engine traffic*:
WebView2 and WKWebView open their own sockets and no lint sees them, so the only
instrument is SC-014's capture. Plan both, and state plainly which requirement
each covers. Consequently **SC-014 is not a portable CI job**: it must run
against the real engine on the tier's real platform, with a harness CA trusted
only in the test image, and it must capture DNS as well as TLS payloads because
FR-007a names name resolution explicitly. *Rejected*: code review as the
enforcement (it is what FR-007a's own text declines to rely on); a local proxy
the shell forces all traffic through (for page loads that means terminating TLS
in the product — it is the right shape for the *test* harness only); trusting
engine documentation instead of capturing (documentation is not evidence about
the built artefact). *Indicative — the construction is a proposal, the
requirements established · FR-007a, SC-014; Permanent Prohibition on server-side
collection of browsing history.*

**4.2 SC-014's "every URL-bearing payload" does not cover what a conforming
build emits. [GAP]** The harness must classify on *what the payload carries* —
history-bearing or not — against FR-007a's four entries plus a committed, closed
list of permitted non-history purposes, naming the invoking function for each as
SC-014 already requires. In SC-014's scripted session (first run, a search, ten
navigations, a download, a private window, close and reopen, signed out) a
conforming build still emits at least the FR-014 update check and the
blocking-list refresh FR-008 depends on — both named in SC-005's own wake
enumeration. Neither is one of FR-007a's four entries, and neither has to be:
FR-007a is scoped to transmissions carrying an address the member navigated to,
a term typed into the FR-003 field, page content, or a value derived from those,
and an update check carries none. But SC-014's stated test is that "every
URL-bearing payload in that capture is one FR-007a permits", and an update check
is a payload bearing a URL — so read literally, SC-014 fails a build for doing
something FR-007a permits. (App-surface delivery under FR-019 is a third such
payload, though FR-016a means it does not arise in this particular session: on a
fresh profile no Apivo surface is rendered until the member activates one.)
**Which reading governs is a founder decision landing as a specification
amendment, not an implementer's call.** *Rejected*: suppress the update check
during the capture so the literal reading passes (it makes the capture a
measurement of a build nobody ships); widen FR-007a's enumeration to include the
update check (wrong — that list is deliberately about history-bearing
transmissions, and adding non-history entries dilutes the Permanent Prohibition
it enforces). *Established as a textual discrepancy; unestablished as to which
reading governs (§12.3) · SC-014, FR-007a, FR-014, FR-008, SC-005, FR-016a,
FR-019.*

**4.3 FR-036a is a prohibition on what Evreos does, never a protection claim.**
Plan it as a set of things deliberately not built plus a rule for the capability
catalogue, and never let it become a member-facing anti-fingerprinting claim;
FR-041 governs the page where such a claim would land. Read against what a
browser does by default, FR-036a rules out: a per-install or per-machine
identifier seeded from a hardware value (Windows MachineGuid, volume serial, MAC
address, a macOS platform UUID) for update, FR-014 staged-rollout bucketing, or
crash grouping; the "device model", "screen geometry", "total memory", "CPU",
"installed fonts", "timezone" and "locale" fields that ship as defaults in every
crash reporter — FR-039d independently forbids counter keys on the same list and
FR-039c's closed frame contents already exclude them; a daily-rotated salted
hash of the same inputs, which FR-036a names and forecloses explicitly;
timing-derived correlators of the `performance.now()` class; and any device
provisioning identifier a content-protection path would require, which ADR-0001
risk 8 already routes to a founder decision. What it does *not* rule out is a
random per-install UUID — that is not derived from device characteristics — but
FR-039b bans identifiers in reports and Principle VI's aggregate condition bans
one elsewhere, so nothing is gained by the distinction. The scoping point
matters most: FR-036a says nothing about a *site* fingerprinting the member, and
on an OS-webview architecture very little can be done about that anyway, since
patching the engine's JavaScript surface is the fork this decision exists to
avoid. *Rejected*: build anti-fingerprinting defences against sites (not
required by FR-036a and not deliverable on this architecture); treat FR-036a as
satisfied by "we use no fingerprinting library" (the defaults in crash reporters
and update clients are where this breaks, not in a library anyone chose).
*Indicative — the requirement established, the enumeration reasoned from its own
categories rather than from a cited inventory · FR-036a, FR-039b–d, FR-040,
FR-041; Principle VI; ADR-0001 risk 8.*

---

## 5. Shell surfaces, accessibility, language

**5.1 Three rendering classes, not two**: (a) browser chrome the shell renders
and owns; (b) shell-owned money and settings surfaces shipping in the release —
wallet, claim surface, cashback offer control; (c) server-delivered signed app
surfaces that never ship in the release — epiloYES, the home surface's app
tiles. The spec forces the partition: FR-018a defines a qualifying action as one
taken "in the browser's own chrome — a control the shell renders and owns,
outside the page"; FR-018b requires the offer surface rendered in the chrome,
never in the page; FR-031 requires the wallet delivered as part of the shell and
usable in a build with no extension host. Against that, FR-019 requires app
surfaces updatable without a browser release, FR-019a requires each signed and
verified before render or cache write, and FR-019b forbids any surface or cached
copy shipping in an installer. A plan that treats "Apivo UI" as one layer breaks
one of the two rules whichever way it resolves. *Established · FR-016, FR-018a,
FR-018b, FR-019, FR-019a, FR-019b, FR-031.*

**5.2 The chrome renderer stays spike S4, settled on measured criteria.** Each
candidate must demonstrate on the tier-1 pinned runner: SC-006's 16 ms with no
trial over 16 ms across ≥1000 trials for both tab switch and address-field
keystroke; a screen-reader pass with Narrator and NVDA; correct German dead-key
and Greek entry in the FR-003 field; legible layout at 200% with no clipping.
ADR-0001 states the limit of its own accessibility rationale — the OS-engine
argument "covers page content, not the shell's own chrome … and what renders
them is an output of spike S4, not settled here" — and SC-006, being ratified,
binds before the spike runs and is the sharpest discriminator: a chrome rendered
in a second webview pays an IPC hop plus layout plus a compositor round trip per
keystroke on an 8th-generation i3 with 8 GB, against a hard maximum with zero
permitted trial-level discards; SC-004's ratified tier-1 ten-tab figure is the
second, since a chrome webview adds a renderer process to a count that already
includes every process Evreos causes to exist. The candidates and their costs:
platform-native widgets per tier (best accessibility, IME and focus behaviour
for free, but two chrome implementations for a solo founder, with FR-035 and
FR-042 seams crossing two toolkits); a second webview rendering HTML (one
implementation and accessibility through the engine's own tree, but the
candidate most at risk on SC-006 and SC-004, and it makes the shell's chrome
depend on the engine whose failure modes ADR-0001 catalogues); a Rust GPU
toolkit with AccessKit (one implementation and full latency control, but chrome
accessibility becomes the project's own problem — §5.3). *Indicative — the
criteria are established, the outcome is S4's · ADR-0001 rationale 1 and risk 7;
SC-004, SC-006; Principle X.*

**5.3 If the chrome is drawn, AccessKit is the mechanism and an explicit
conformance milestone is owed; the chrome/content tree join is unestablished
(N6).** AccessKit's released adapters are `accesskit_windows` (UI Automation),
`accesskit_macos` (NSAccessibility) and `accesskit_unix` (AT-SPI), plus Android
and iOS, and its own README qualifies them: the released adapters "are all at
rough feature parity. They don't yet support all types of UI elements or all of
the properties in the schema, but they have enough functionality to make
non-trivial applications accessible." For a product where WCAG 2.1 AA is a
release blocker (Principle X, FR-034, SC-008) that is a starting point, not a
conformance claim. Its multiple-tree support (PR #655, merged 2026-01-10) is
explicitly scoped to AccessKit-provided subtrees: the author records that it
does not consume a native WebView2 or WKWebView accessibility tree, and that
nodes cannot reference nodes in a different tree, so a chrome node cannot be
`labelled-by` a content node — and the spec's own Edge Case names this boundary
as where this class of interface commonly fails. *Rejected*: an in-toolkit
screen reader (SC-008 is about the platform's own assistive technology, and
ADR-0001 risk 7 requires driving each shell surface with it); assume the OS
composes the trees because the webview is a child window (plausible on Windows
via the HWND-rooted UIA hierarchy, but nothing located establishes it for an
AccessKit host — so N6, not a finding). *Established for AccessKit's stated
scope; unestablished for the join · AccessKit README and PR #655; ADR-0001 risk
7; FR-034, SC-008.*

**5.4 The tier-1 chrome/content focus boundary has a documented mechanism, with
one gap.** Handle `MoveFocusRequested` to take focus out of content and call
`MoveFocus(Next|Previous|Programmatic)` to put it in; route shortcuts through
`AcceleratorKeyPressed`, with a separate path for the keys that event does not
cover. Microsoft documents `MoveFocusRequested` as raised "when user tries to
tab out of the WebView", with focus still on the WebView when it fires, and
`MoveFocus` Next/Previous aligning to Tab and Shift+Tab — exactly the boundary
the spec's Edge Case names. The gap: `AcceleratorKeyPressed` is documented as
raised only when Ctrl or Alt is held, so F3, F6, F11, Escape and bare Tab are
not accelerators by that definition and need their own path, while FR-011
requires every pointer-reachable action keyboard-reachable, which includes
escaping a focus trap in page content. On tier 2 no equivalent mechanism was
located: an AppKit chrome gets it from the responder chain and key-view loop, a
drawn chrome does not — a further way SC-008 discriminates between the S4
candidates. *Established on tier 1; unestablished on tier 2 · Microsoft Learn
`CoreWebView2Controller`; Edge Cases, FR-011.*

**5.5 Commit a written WCAG2ICT reading of FR-034/SC-008 and name EN 301 549 as
the conformance target. [GAP]** FR-034 states WCAG 2.1 AA and names no mapping,
so this is a proposal — but WCAG 2.1 is written for web pages while FR-034
applies it to "every shell surface", which for a native chrome needs a stated
mapping or every review argues it afresh. WCAG2ICT (W3C Group Note, 8 October
2024, covering WCAG 2.0, 2.1 and 2.2) supplies the substitutions — a single web
page equated to a software program, a set of web pages to a set of software
programs — and records that Section 508 and EN 301 549 do not apply 2.4.1 Bypass
Blocks, 2.4.5 Multiple Ways, 3.2.3 Consistent Navigation and 3.2.4 Consistent
Identification to non-web software, with EN 301 549 additionally excluding 2.4.2
Page Titled and 3.1.2 Language of Parts. EN 301 549 V3.2.1 incorporates WCAG 2.1
Level AA and is the harmonised standard behind the European Accessibility Act,
enforceable since 28 June 2025, and Evreos ships into Germany and Greece; naming
it also keeps FR-041's obligation on the distribution page (a web page: plain
WCAG 2.1 AA) and the shell obligation (non-web software) as two clearly
different tests. Adopt it on Principle X's own authority regardless of how the
Act's legal scope resolves (§12.4). *Rejected*: apply WCAG 2.1 AA verbatim to
native surfaces (2.4.2 Page Titled and 3.2.3 Consistent Navigation have no
coherent meaning for a tab strip, and a reviewer with no written mapping will
either wave them through or block arbitrarily); adopt WCAG 2.2 AA instead
(attractive for 2.5.8 but not what FR-034 states — an amendment, not a plan
decision). *Established for the standards; the adoption is a **[GAP]** · W3C
WCAG2ICT; ETSI EN 301 549 V3.2.1; FR-034, FR-041, SC-008.*

**5.6 FR-005's 200% is two mechanisms driven from one value, with page zoom a
distinct third.** Set the shell's chrome layout scale and the engine's
rasterization scale together, and keep page zoom as a separate per-site value.
WebView2 documents `RasterizationScale` as "the combination of the monitor DPI
scale and text scaling set by the user", applying to WebView content, popups,
context menus and scroll bars; `ShouldDetectMonitorScaleChanges` decides whether
the WebView tracks DPI changes itself or the app sets the scale; and
`BoundsMode` chooses whether `Bounds` is raw or logical pixels under `logical
size × rasterization scale = raw pixel size`. If the shell scales its chrome and
leaves the WebView tracking DPI on its own, the two disagree at 200% and content
scales twice or not at all. FR-005 additionally requires page zoom as a separate
member-facing control, so three values exist and must not be conflated in the
settings model; SC-008 requires the surface *usable* at 200%, which is a layout
property — targets, spacing, reflow — not a type-size one. *Established ·
Microsoft Learn `ICoreWebView2Controller3`; WebView2Feedback
`specs/RasterizationScale.md`; FR-005, SC-008.*

**5.7 FR-036 is the shell's problem, not the engine's.** Own German dead keys
and Greek entry as a shell test over the FR-003 combined field, the find-in-page
field, and every chrome text input. ADR-0001's rationale 1 hands input, IME and
the accessibility tree to the OS engine "for page content on that platform" and
immediately limits it — "It covers page content, not the shell's own chrome" —
and the address field is the most-used text input in the product and it is ours.
If winit is S4's output the mechanism exists and is documented:
`WindowEvent::Ime` (enabled via `Window::set_ime_allowed`; only
iOS/Android/Web/Orbital marked unsupported), `KeyEvent` carrying composed
`text`, and a query for whether the current input is a Dead key — and winit
documents a Windows behaviour worth encoding as a test expectation rather than
discovering: when a dead key was pressed earlier but cannot be combined with the
character from this keypress, the produced text consists of two characters.
*Rejected*: read raw virtual-key codes and map them ourselves (the classic way
this breaks — it bypasses composition and produces `´a` instead of `á`); test
text entry only in page content (that tests the engine, not the shell, and
passes while the address field is broken). *Established · ADR-0001 rationale 1;
winit doc comments; FR-036, SC-008.*

**5.8 Cohort rules, stated as build rules rather than as tone.** (i) The FR-008
per-site blocking control is surfaced at the moment of breakage, in the failing
page's own chrome affordance, not only in settings. (ii) Every disabled control
under FR-029 and FR-029a reads as "not yet available" and is verified by
FR-029a's own observer test — present, reachable, explained, no outbound
request, no error state. (iii) No hover-only affordance anywhere in the chrome,
and a 24×24 minimum pointer target adopted as a **project rule — [GAP]**: 24×24
is WCAG 2.2 SC 2.5.8 at Level AA, while WCAG 2.1's target-size criterion 2.5.5
is Level AAA at 44×44, so FR-034's WCAG 2.1 AA does not require it and this is a
founder decision for this cohort rather than a conformance obligation. (iv)
SC-003's runtime-absent first run gets designed progress, since on this cohort
it is the most likely first experience. Principle X's rationale states the stake
— "the difference between a product this cohort adopts and one they abandon at
the first dialog they cannot read" — and the spec's Edge Case says the per-site
control "must be discoverable at the moment of failure, because this cohort will
otherwise abandon the browser rather than hunt through settings". All four are
testable now; deferring them to usability testing means discovering them after
the interface is built. *Indicative, except the WCAG provenance, which is
established · Principle X; Edge Cases, FR-008, FR-029, FR-029a, FR-034, SC-003,
SC-007, SC-008.*

**5.9 Restore sessions suspended. [GAP]** FR-001 restores tab identity, order
and address eagerly; the page load happens on first activation; the session file
is written atomically (temp file plus rename) and never contains a private
window. SC-002's entries are measured per launch, and a restore that loads ten
pages spends the whole budget inside the engine on the oldest machine the floor
admits; SC-004 counts memory "from the first tab opening", and ten live
renderers at launch is the worst possible entry into a 150 MB ceiling; FR-007
requires a private window to leave no browsing trace after it closes, which
makes exclusion from the session store a correctness requirement rather than a
nicety. Restore-as-suspended also makes FR-001 and FR-002 the same mechanism
instead of two. *Rejected*: restore all tabs live (both budgets); restore only
the last active tab (FR-001 and Story 1 scenario 3 require the session restored,
and a cohort that leaves ten tabs open all day would read it as data loss).
*Indicative · FR-001, FR-002, FR-007, SC-002, SC-004.*

**5.10 Language and place (FR-035, Principle VII).** Fluent (`.ftl`), one
catalogue per primary language subtag — `de.ftl`, `el.ftl`, `en.ftl` — embedded
in the binary at build time, indexed by a closed Rust enum `Language { De, El,
En }`, with `Place` as a separate type that never appears in a catalogue key,
plus a CI check that fails on any region subtag in a catalogue filename or
message key and on any request builder serialising language and place into one
field. Cost the format against SC-001 before adopting it (N10). `fluent-bundle`
is documented as "a low-level implementation of a collection of localization
messages for a single locale", so one bundle per language is the format's native
unit — what Principle VII asks for; named-argument interpolation matters
specifically because FR-042 forbids any brand name outside the brand
configuration, so the brand cannot be baked into strings and the catalogue needs
`{ $brand }`-style arguments rather than a flat table; and FR-016a requires the
neutral menu entry to be "a static label drawn from the interface catalogues
under FR-035", so the catalogue must resolve at first run with no account, no
network and no Apivo state, which argues for embedding rather than fetching. The
enum-not-`LanguageIdentifier` rule is the enforcement point:
`unic-langid::LanguageIdentifier` carries language, script, region and variants,
so typing the key as one re-admits `de-DE`, which FR-035 names as the exact
failure. *Rejected*: gettext (the message key is the source string, which fights
brand-name interpolation and makes an untranslated-string check — which FR-041
requires on the distribution page — harder to write); plain TOML/JSON tables
(smallest against SC-001 with no plural or selector machinery, viable if the
wallet's plural and gender cases turn out trivial, and worth costing before
Fluent is adopted); ICU MessageFormat (it needs CLDR data, which is bytes
against a 20 MB download and 60 MB installed budget). *Indicative; the byte cost
is unmeasured (N10) · projectfluent/fluent-rs; FR-016a, FR-035, FR-041, FR-042,
SC-001.*

---

## 6. Apps: signing, capabilities, delivery (FR-016 – FR-020)

**6.1 Ed25519 over a fixed-layout, length-prefixed, domain-separated preimage.**
The FR-019a surface preimage is `DOMAIN("evreos.surface.v1\0") || len||app_id ||
len||manifest_digest(SHA-256) || u64_be surface_version ||
len||surface_digest(SHA-256) || u64_be not_after`; the FR-017 manifest carries
its own domain string `"evreos.manifest.v1"`; verification uses Ed25519
**strict** verification (small-order R and public key rejected, non-canonical R
encodings rejected); the format lives in an in-repo format document. FR-019a
demands one signature covering surface bytes, app identity, manifest digest and
version *together*, and a fixed preimage makes "together" literal;
length-prefixing removes the concatenation ambiguity that lets (`app_id="a"`,
`digest="bc"`) and (`app_id="ab"`, `digest="c"`) share a preimage; the domain
string is the device COSE uses — a context string so a signature made in one
context cannot be replayed in another, here stopping a manifest signature being
presented as a surface signature. Strict verification because ed25519-dalek's
own source records that the RFC permits but does not require the small-order
check and that implementations historically differed, producing signatures that
verify singly but fail in batch — malleability that would let a delivery host
hold two distinct valid encodings for one surface. Ed25519 keys are 32 bytes and
signatures 64, so the pinned root costs 32 bytes of SC-001 and each delivered
signature 64: the choice is decided by auditability, not by Principle II.
*Rejected*: JWS over a JSON manifest (JSON signing needs a canonicalisation
rule, and JWS's `alg` header is a long-standing source of algorithm-confusion
bugs; a fixed preimage has no algorithm field to confuse); COSE_Sign1
(structurally a good fit — protected header for the binding, payload for the
digest — rejected only because it pulls in a CBOR stack plus
deterministic-encoding rules that must themselves be reviewed, where the
preimage is about thirty lines a reviewer reads whole); minisign (a reasonable
off-the-shelf container, but its trusted-comment field is free text and would
have to be re-specified to carry the four bound fields, at which point the
preimage is being defined anyway); ECDSA P-256 (no advantage here, and a
per-signature nonce requirement Ed25519 does not have). *Established for
FR-019a's demand and for ed25519-dalek's constants and `verify_strict` doc
comment; the COSE detail comes from a search summary, the RFC hosts being
unreachable from the investigating session — treat it as indicative and re-read
RFC 9052 §4.4 and §9 before the format document is written · FR-017, FR-019a.*

**6.2 Two key levels: an offline root pinned in the shell, an online publishing
key under a root-signed delegation. [GAP]** The delegation, fetched alongside
surfaces, carries a monotonic version, a validity window, and the app registry
entries (app_id → publishing key → capability ceiling → optional `supersedes`);
the client accepts it only under the pinned root, only at a version greater than
or equal to the highest it has seen, and only inside its validity window.
FR-019a requires a pinned root and says nothing about intermediate keys, so the
split is a proposal — but TUF states the case for it directly: the client "MUST
ship with trusted root keys for each configured repository" and "the root role's
private keys MUST be kept very secure and thus should be kept offline", against
operational keys that are online. A delegation fetched from the delivery host
does not weaken the pin, because the host cannot forge one; what the host *can*
do is withhold a new delegation and replay an old one — TUF's freeze attack —
and the validity window plus the monotonic version floor is the countermeasure
TUF names. Without the split, every publish puts the root key online, which is
the arrangement a root exists to avoid, and Principle IX expects app content to
be published often. *Rejected*: sign every surface directly with the pinned root
(simpler to verify and review, but it forces the root online for routine
publishing and makes compromise unrecoverable without an emergency release);
adopt TUF itself (four metadata roles, a Rust implementation and its own refresh
loop is a large surface for a solo founder with two first-party apps — TUF is
used here as the attack taxonomy and the offline/online key discipline, not as
the wire format); ship k-of-n root keys (theatre at this scale: with one holder,
n keys are one key). **Residual risk, stated:** root compromise is recoverable
only by a browser release on a staged FR-014 channel. *Established for TUF's
text; the design is a **[GAP]** · FR-019a.*

**6.3 Effective capabilities are the intersection of four sets**: the registry
ceiling shipped in the release **[GAP]**, the capability catalogue shipped in
the release, the capabilities the verified manifest declares, and the member's
per-app grants — with an app able to ask the shell what it actually holds, so it
can degrade honestly. FR-018 already requires every declarable capability
classified page-adjacent or not in a published catalogue and an unclassified
capability never granted, but that closes the naming escape only if the
catalogue is a property of the *build*, since a fetched catalogue is one the
delivery host can extend, which is exactly the escape FR-018's last sentence
forecloses. The registry ceiling is the second half and is the gap: without it,
compromise of the online publishing key yields a manifest declaring every
catalogued capability, and every non-page-adjacent one is then held with no
member in the loop; with it, the blast radius is bounded to what the app already
had, and FR-017's "MUST NOT be able to widen them from inside" holds
structurally at the shell boundary rather than by the publisher's restraint. The
intersection also gives forward compatibility a defined answer: an app published
for a newer shell running on an older one is not refused, it simply does not
hold the unknown capability. *Rejected*: manifest alone bounds capabilities (the
compromise argument); refuse the whole app when its manifest names an unknown
capability (it turns every capability addition into a hard break for members on
older shells, and FR-018's rule is about *granting* rather than loading); fetch
the catalogue with the manifest so the vocabulary can evolve without a release
(it hands the delivery host the power to name new capabilities). **Accepted
cost:** widening an app's ceiling needs a browser release — defensible because
Principle IX keeps app *content* off the release cycle, and what an app may *do*
is not content. *Established for FR-018's rule; the ceiling is a **[GAP]** ·
FR-016a, FR-017, FR-018; Principle IX.*

**6.4 App surfaces make no network requests of their own, and are served from
shell memory under one custom scheme.** Every remote interaction — Apivo API,
catalogue, click-out issuance — goes through shell-mediated IPC gated on the
effective capability set, and the surface webview is configured to deny all
remote loads **[GAP]**. Three requirements converge on confinement: FR-023 puts
the account credential in the OS secure store and nowhere else, so a surface
holding a token would put it in a store Evreos writes; FR-040 requires the
client-type marker on exactly four member-initiated acts and on no others,
enforceable only where one code path issues requests; and FR-007a's conformance
test must fail on any outbound request no entry accounts for, which is tractable
when the shell is the sole originator and near-untestable when arbitrary
delivered script can fetch. Confinement also removes the CORS problem created by
platform-varying custom-protocol origins: wry's own documentation states that
pages loaded from a custom protocol "will have different Origin on different
platforms", which would otherwise make the Apivo API's allowed-origin list a
per-platform, per-wry-version dependency. On hosting: FR-019a requires
verification before rendering or writing to the FR-020 cache, so the bytes must
reach the engine from the shell rather than over the wire, and ADR-0001 records
that custom protocols are URL scheme handlers on the configuration and do not
depend on the navigation delegate, so this route survives §2.2's replacement.
Putting the app id in the first path segment yields a distinct host on both
tiers under wry's documented formats — `evapp://<app-id>/…` on macOS,
`http(s)://evapp.<app-id>/…` on Windows — and therefore a distinct origin per
app without one webview environment per app, which would be paid for against
SC-004's ten-tab budget; the https variant matters on Windows because wry's
default there is http and a non-secure origin is not a secure context.
*Rejected*: give surfaces a scoped token and let them call the API directly
(FR-023, FR-040, FR-007a); proxy through a loopback HTTP server the surface can
fetch (it creates an unauthenticated local surface and still needs per-request
capability checks keyed to something weaker than the app identity the shell
already knows); a distinct scheme per app (it also gives origin separation, but
the scheme set becomes dynamic and schemes are registered at
webview-configuration time rather than per navigation); `file://` or a
virtual-host-to-folder mapping (it requires writing verified bytes to disk
before rendering and blurs the FR-019b cache boundary); one webview environment
per app for hard isolation (SC-004). *Established for the requirements and for
wry's documented URL shapes; unestablished that a per-app path segment actually
produces a distinct origin, a secure context and partitioned storage on each
tier (N8) · FR-007a, FR-019a, FR-020, FR-023, FR-040, SC-004; ADR-0001 accepted
costs.*

**6.5 v1 ships no page-injection mechanism on behalf of an app or a commercial
surface.** It does inject on behalf of blocking, and §3.7 states that mechanism:
FR-008 requires the space a blocked element leaves to be collapsed — which, as
FR-008's own text says, "modifies the page's rendering" — and §3.7's mechanism
is a `display:none` stylesheet applied to the main frame **and** sub-frames plus
a script that hides elements whose load was refused, with the cosmetic half
injected on tier 1 because nothing on that tier supplies it. FR-018a settles
that case in advance: collapsing "is not an insertion of content", and "Content
blocking under FR-008 meets" the three-part exemption test "including the
collapsing of blocked slots that requirement mandates", so it needs no member
action. What v1 ships no mechanism for is injection on behalf of an app or a
commercial surface: the capability catalogue contains no capability that writes
into, reads from, or executes script in a web page the member visits: offer
detection runs in the shell against the current address, matched locally against
a downloaded merchant list; the offer is rendered in the shell's own chrome; and
the member's activation of that chrome control is what causes the FR-025
click-out request and the navigation. Outside the blocking exemption, FR-018a is
therefore satisfied structurally rather than implemented. FR-018b forbids
advertising in a page outright and requires a cashback offer surface rendered in
the browser's own chrome, and FR-018a's exemption test has three parts that must
all hold — the shell speaks on the member's behalf, carries no commercial
interest, places no third party's content — of which a cashback function fails
the second by construction, so no compliant in-page cashback path exists and
occasion-token machinery would be building apparatus for something already
prohibited. **This resolves a real conflict:** ADR-0001's capability floor
sketches the wallet as built "using navigation gating, initialization scripts
and the cross-platform cookie API", and an initialization script inserted into a
*merchant's* page is an insertion of content in FR-018a's ordinary sense, made
in a commercial interest and therefore not exempt. The ADR predates FR-018a; the
constitution supersedes and FR-018a fixes the occasion at the narrowest
available reading, so the specification governs and that one mechanism is
unavailable for that one purpose — the ADR's conclusion, that the wallet is
built once natively in the shell, is unaffected, as are navigation gating and
the cookie API. Local matching is also what keeps FR-007a intact: asking a
service whether an offer applies to the current address is not one of the four
enumerated transmissions. *Rejected*: build the occasion-token mechanism anyway
— token bound to tab, page-load epoch, offer id and app id, invalidated by any
address change including same-document navigation (machinery with no compliant
consumer in v1, though the seam it needs, the navigation epoch, is required for
other reasons and is carried in §1.7); ask the service whether an offer applies
to the current address (FR-007a's list is exhaustive); ship no in-page offer
detection at all and require members to start from the catalogue (viable and
cheaper, and worth costing under FR-043 — it removes the merchant-list bytes and
any refresh wake from the idle path). *Established · FR-007a, FR-008, FR-018a,
FR-018b, FR-025, FR-043; ADR-0001 capability floor; Principle IV and the
Permanent Prohibitions.*

**6.6 Version floor and expiry. [GAP]** Surface versions are a
publisher-assigned monotonic `u64` that is not the display version; the
anti-downgrade floor is stored per app id in a store **separate from the FR-020
cache**, so clearing the cache does not clear the floor; and the signature's
`not_after` bounds acceptance of a *fetched* surface only, while a cached
surface is always renderable offline under FR-020, marked as the offline state,
however old. FR-019a requires the delivered surface's version to be at least the
cached copy's, which on a fresh install or after a cache clear has no floor to
compare against — whoever controls delivery at that moment replays a correctly
signed old surface with a known defect, which is TUF's rollback attack — so
keeping the floor outside the cache is what makes "previously knew to be
available" survive eviction. A monotonic integer rather than semver because
ordering must be total and unambiguous, and semver's pre-release ordering is a
recurring source of comparison bugs in exactly this position. The expiry is the
freeze-attack countermeasure and must be scoped to acceptance rather than
rendering, or it silently converts FR-020's offline guarantee into a blank
surface after some interval — the outcome FR-019a and FR-020 both name as the
failure. **Residual, stated:** the floor is a local file, so an attacker with
profile write access can lower it; that attacker already has the machine and no
client-side mechanism improves on it. *Rejected*: the floor stored inside the
cache entry (a cache clear becomes a downgrade window); no expiry at all
(FR-019a does not require one and this is the minimum-compliance option,
rejected because an indefinitely replayed "current" surface is otherwise
undetectable, at a cost of one `u64` in the preimage); an expiry that also stops
a cached surface rendering (FR-020). *Established for the attack shapes; the
design is a **[GAP]** · FR-019a, FR-020.*

**6.7 FR-019b is enforced by four independent mechanisms, none of them the
signature.** (1) A `VerifiedSurface` type whose constructor is private to the
verifier and whose only producer takes bytes handed over by the delivery client,
with the cache write path accepting nothing else. (2) A release-artefact scan —
a script in the idiom of `scripts/check-budgets.py` — that fails the release job
when the installer or the installed tree contains any file bearing the
surface-bundle magic, any app manifest, or any path under the surface cache
directory, and that also refuses a shell binary containing the CI fixture app's
magic. (3) A post-install acceptance test: install, assert the cache directory
is absent or empty before any network activity, then launch offline and require
every app to present FR-020's stated offline state rather than content. (4) The
SC-014-style capture extended to assert that the first render of any surface is
preceded by that surface's delivery fetch. FR-019b itself notes that a
pre-cached surface shipped in an installer would carry a valid signature and so
would satisfy FR-019a — signature verification cannot be the enforcement,
because the excluded thing is correctly signed. That leaves provenance and
artefact contents, which (1) and (2) check, and observable behaviour, which (3)
checks: a freshly installed build with no network showing app content is the
violation in a form an observer can reproduce without reading the build. (4)
closes the case where content is embedded but made to look fetched. *Rejected*:
review the installer manifest (exactly the class of rule the constitution says
fails silently under time pressure); rely on SC-001's installed-footprint delta
(a small surface hides inside a 60 MB budget, and the delta measures bytes, not
provenance); ban `include_bytes!`/`include_str!` by lint alone (useful as a
fifth check, insufficient alone, since content can arrive through a build script
or a packaged resource directory the lint never sees). *Established · FR-019a,
FR-019b, FR-020, SC-014.*

**6.8 FR-016a's anti-rename rule: succession in the root-signed delegation, plus
a shipped roster. [GAP]** Record succession (`supersedes`) in the root-signed
delegation, never in the app's own manifest, and additionally pin the
first-party app roster in the shipped release so the client refuses to present
on the home surface any identity the shipped roster does not carry — stating
plainly that this binds the delivery host and the publisher but **not** the
holder of the root key, where the remaining guarantee is procedural. FR-016a
exists because otherwise "an operator clears every dismissal by renaming", and
it is a release criterion under Principle IV; if succession were declared in the
manifest, the party with an interest in clearing dismissals is the same party
that writes the manifest, so the rule would hold exactly as long as it was
convenient — the failure the requirement names. Moving it into the document
signed by the offline root makes adding a successor a deliberate, reviewable
act, and keeping the registry in this repository puts it in front of the
adversarial review the Development Workflow requires; the roster moves a new app
identity out of the operator's unilateral reach and into a browser release,
which is the same trust move FR-019a already makes for the signature root.
FR-019b is not breached: a list of identities and public keys is not an app
surface and not a cached copy of one. **Residual, stated:** no client mechanism
can compel the root holder to write `supersedes`; the guarantee against that
party is that the registry is version-controlled and reviewed, not that it is
cryptographically enforced. *Rejected*: derive app identity from the publishing
key so a rename requires a new key (it helps only in that the new key must also
be authorised by the root — the same root-holder trust — and it couples identity
to key rotation so routine rotation looks like a new app); client-side heuristic
matching of a renamed app (unfalsifiable and gameable); dismissal state held
server-side (it turns each member's dismissal into a record on the operator's
server and makes the operator the enforcer of a rule written against the
operator). *Established for the requirement and its stated reason; the roster is
a **[GAP]** · FR-016, FR-016a, FR-017, FR-019a, FR-019b; Principle IV.*

**6.9 The FR-016a neutral menu entry is held by an automated invariant**: a
catalogue-keyed static label rendered from the menu's own type and colour
tokens, with a test asserting that the entry's accessible name, its rendered
style tokens and its node shape are identical on a fresh profile and on a
signed-in profile with wallet state present. FR-016a already writes the test —
the entry "MUST read identically on a fresh profile and on a signed-in member's
machine, which is the test a build can be held to" — and enumerates what is
forbidden: brand colour, badge, counter, amount, promotional string, or any
state derived from the wallet, the claim surface or any other money surface.
That is a diffable assertion over two profile fixtures, and it belongs in the
same test suite as FR-007a's network-capture test rather than in review, since
Principle IV makes a violation a release blocker. It also constrains the S4
candidates: whatever renders the chrome must let a test read a menu item's
accessible name and resolved style, which a drawn chrome offers only if its
accessibility tree exposes them — one more reason §5.3's conformance work is on
the critical path rather than beside it. *Established · FR-016a; Principle IV.*

---

## 7. Money surfaces (FR-021 – FR-033)

**7.1 Money surfaces are shell-native; content apps are delivered.** Wallet,
claim and the offer control ship in the release; epiloYES is a delivered signed
surface; the merchant catalogue is a decision the plan must take (§12.3). The
boundary is written into the capability catalogue and the app registry so an
implementer cannot reclassify across it. FR-031 requires the wallet delivered as
part of the shell and forbids any extension mechanism, and FR-016a's dismissal
clause enumerates "any first-party app … the home surface … the wallet and claim
surfaces", listing wallet and claim *beside* apps rather than among them. The
security argument is stronger: FR-026 and FR-026a impose properties — no
computation, no aggregation, no omission, no cached-as-truth, no client
deduplication of money actions — that are mechanically checkable only in code
shipping in the reviewed build and passing the budget gates, and a
server-delivered wallet moves them outside every gate this repository has.
FR-032 also wants the claim flow to open directly after installation, which a
delivered surface cannot guarantee: FR-019b forbids pre-caching, so a freshly
installed offline client would have nothing to show. *Rejected*: deliver the
wallet as a signed surface (not forbidden by FR-031's letter, since a signed
surface is not an extension, and rejected on the reviewability and FR-019b
arguments); make the merchant catalogue shell-native (rejected as the *default*
because catalogue content changes constantly and Principle IX's motivation
applies squarely; as a delivered surface it holds a non-page-adjacent capability
to read catalogue data and to ask the shell to open an offer, and never
constructs a URL itself). *Established · FR-016a, FR-019b, FR-026, FR-026a,
FR-031, FR-032; Principles V and IX.*

**7.2 The wallet renders and does nothing else, enforced in the type system.**
It displays every entry the service reports in the state the service reports; it
displays each state's total and any payable amount only as fields the service
itself sent; it never performs arithmetic, rounding, currency conversion or
cross-entry summation; it never hides a state; any value held on the device is
typed as stale and carries the time it was received; and on reconnection the
service's value replaces the cached one outright, with no reconcile, merge or
diff. Enforced by an `Amount` with no arithmetic trait implementations and no
public constructor other than the API deserialiser, a distinct `Stale { amount,
received_at }` with no path back to a plain `Amount`, and a trybuild
compile-fail test asserting that `a + b` and `iter.sum()` do not typecheck. This
is FR-026 and FR-026a read literally, and the literal reading has a sharp
consequence: "MUST NOT … aggregate any amount" prohibits a client-computed total
*even when it is arithmetically correct*, so every total the wallet shows must
be a field in the response — and whether the API provides one is unverified
(§12.4). Principle V's rationale says why: a client that computes money will
eventually disagree with the ledger, and the member will believe the client;
making the prohibited program fail to compile turns that central prohibition
into a CI artefact, which is the standard the constitution sets for anything
measurable. *Rejected*: enforce by review and a style rule (Principle II's own
argument about rules kept by discipline); let the client format amounts from
minor units (presentation rather than computation and defensible, but it puts
rounding and symbol placement in the client while FR-035 requires language and
place to stay separate values — the safer arrangement is a service-rendered
display string for the requested (language, place) pair beside the structured
amount); filter or paginate entries (permitted for entries, prohibited where it
removes a *state*: FR-026 names "shows pending and confirmed but drops declined
and reversed" as the failure, so each of the four states and its
service-reported total is present even when empty). *Established · FR-026,
FR-026a, FR-027, FR-035; Principle V.*

**7.3 Withdrawal is a two-step against the service; click-out URLs pass byte for
byte.** Request a withdrawal token, then submit with it; on a submission whose
response is lost, the client neither retries blind nor infers an outcome — it
re-reads withdrawal status and shows an explicit unknown state until the service
answers. Click-outs are opened by passing the service-issued URL to navigation
byte for byte, through a newtype constructible only from the API response, with
a test asserting byte equality between the response field and the navigated
address; and the client must not decide that a status is terminal. FR-026a
forbids the client deduplicating money actions or treating a retry of its own as
having settled one, and forbids it approving, pre-approving or predicting a
payout outcome: a client-generated idempotency key is the tempting shortcut and
sits uncomfortably close to the client owning exactly-once, whereas a
server-issued token keeps exactly-once wholly behind the API where Principle V
puts it, leaving the client a state it *reads* rather than one it decides.
FR-025 forbids the client constructing, templating or modifying an affiliate
link or any parameter of it, and the newtype plus byte-equality test is how that
becomes checkable rather than asserted — it catches the plausible accident,
which is a URL round-tripped through a parser that normalises it. FR-028's
follow-to-terminal-state requires the service to publish the terminal set.
*Rejected*: a client-generated idempotency key on submission (common practice,
arguably compliant since the server still deduplicates, rejected as the default
because it puts a money-action identity in the client for a reviewer to argue
over, against a two-step costing one round trip); optimistic local state after
submission (rejected outright by FR-026a); reconstructing the click-out from an
offer id and a template (rejected by FR-025). *Established · FR-025, FR-026a,
FR-028; Principle V.*

**7.4 FR-029's disabled state is a build constant, and the disabled control is
focusable and announced.** Not a fetched configuration: the control carries
`aria-disabled` (or the platform equivalent) with the explanation
programmatically associated, rather than being natively disabled, and its
activation handler contains no network call path at all; enabling it is a
browser release once Q-E11a is answered. FR-029 and FR-029a require that
activating the control makes no request to any service, and FR-029a's observer
test is stated over "a build with no backing service", so the state must be a
property of the build for the test to mean anything; a fetched flag also creates
a launch-time request whose only purpose is a UI state and makes the disabled
state network-dependent, so the control would behave differently offline — the
"control that failed" presentation both requirements forbid. The accessibility
half is not cosmetic: a natively disabled control is removed from the tab order
and its accessible name is commonly not announced, so a build could satisfy
FR-029's letter — control present, reason stated — while a keyboard or
screen-reader member cannot reach the reason at all, failing FR-034 and SC-008;
since the whole point of present-but-disabled is that the member reads the
explanation, the explanation must be reachable by everyone who can reach the
control. Enabling by release is compliant: the claim surface is shell-native, so
this is not app content and Principle IX is not engaged. *Rejected*: a
server-driven feature flag (the observer-test and offline-behaviour arguments,
and FR-029's own words are that the control makes no request to any service);
hide the control until the service exists (FR-029 requires it present in the
interface); show it enabled and fail on activation (exactly the failed-control
presentation both requirements prohibit). *Established · FR-029, FR-029a,
FR-034, SC-008, SC-010, Q-E11a.*

---

## 8. Diagnostics (FR-039 – FR-039f)

Q-E16 settles the shape: crash reporting ships in v1, counters only. Three
crates split along the privacy boundaries rather than along convenience —
`evreos-diag-state` (the FR-039a state machine and the FR-039e cap set; no
network dependency at all, testable entirely offline), `evreos-diag-transport`
(encapsulation, pinned keys, padding, the one-report-per-connection rule; the
only crate that can open a socket), and `evreos-crash` (capture, the shipped
symbol table, symbolisation; depends on neither of the others and hands a
symbolised report to state, which hands it to transport). The crash-reason
enumeration and the committed OS-version granularity live in one file at the
repository root beside `budgets.toml`, because FR-039c and FR-039d both make
widening them an amendment and a file that is hard to find is a file that gets
edited quietly.

**8.1 The client's FR-039a state contains no generated value of any kind** —
three calendar-derived fields (enrolment date in UTC, the ISO week of that date,
and a terminal-state enum `Enrolled | RetentionSent | Withdrawn | Abandoned`),
with no UUID, nonce, salt, counter or hash, and a test asserting the serialised
field set so that adding one is visible in review. What turns local state into a
per-install identifier is a field with more entropy than the calendar: the
enrolment date is one of ~10⁴ values shared by every install enrolling that day,
and only the week (~52 a year) is ever transmitted, 24–30 days stale by the time
a retention report carries it. A nonce held "only locally" becomes an identifier
the moment any log line, crash frame or future field puts it on the wire, and
FR-036a's own reasoning is that the prohibition binds on the derivation rather
than on the lifetime of what it produces — "no generated value exists" is
checkable, "we will not send the generated value" is a promise. UTC because
Assumptions fix cohort week as ISO-8601 Monday–Sunday UTC; a local-time
enrolment date would put the same install in different weeks by timezone and
make timezone weakly inferable at week boundaries. *Rejected*: a per-enrolment
random id for local idempotence (it buys nothing the date does not and creates a
value that must be defended forever); hashing the enrolment date to key the
state file (a hash of a low-entropy field is not protection, and FR-007a names
hashes among derived forms that stay governed). *Indicative · FR-036a, FR-039a,
FR-039b, Assumptions.*

**8.2 State lives outside the browsing profile, and the pre-consent disclosure
says so** — the per-user application-data directory, not cleared by "clear
browsing data" or a profile reset, removed by uninstall. FR-039a caps at one
enrolment per install, and state cleared with the profile lets one install enrol
repeatedly, inflating the enrolment denominator and depressing the published
retention rate. Placing it outside the profile costs nothing under FR-007a — the
file records no address and no derivative of one — but it does mean a file
survives a privacy-motivated profile wipe, which members reasonably expect to
clear everything; that is a disclosure obligation rather than a reason to move
it back, since FR-039 requires every transmission and its occasion stated in
plain language before consent, and where the state governing those transmissions
lives belongs in the same statement. *Rejected*: profile-scoped state (it
reinstates re-enrolment); machine-scoped state in HKLM or ProgramData (it shares
state across Windows user accounts, a cross-user linkage the design has no need
for, and it needs elevation). *Indicative · FR-007a, FR-039, FR-039a.*

**8.3 At-most-once is the client state machine plus a gateway rule: acknowledge
only after the counter increment is durably committed.** The client transitions
on the acknowledgement; absent one it goes terminal (`Abandoned`) and never
retries. FR-039b forbids retransmission because a report carries no identifier
and so cannot be deduplicated; acknowledging *before* commit would let a lost
commit both lose a counted report and convince the client it was sent, so the
counter silently undercounts, whereas acknowledging after commit makes "acked"
mean "counted" and leaves exactly one failure mode — a report counted at the
gateway whose acknowledgement is lost in transit, so the enrolment abandons and
never emits its retention report. That biases signed-out retention downward. The
bias is structural, is not measurable in production without an identifier the
design excludes, and must be published beside the figure on the same footing as
the "unverified" label FR-039a already mandates. *Rejected*: an idempotency
token (that is an identifier); retry with backoff (rejected by FR-039b's text);
gateway-side deduplication (impossible by construction). *Established · FR-039a,
FR-039b.*

**8.4 Implement FR-039b as Oblivious HTTP over HPKE, with the key configuration
compiled into the release.** OHTTP is precisely the architecture FR-039b
describes: the IETF working-group document states the roles and the split — the
relay "forwards encrypted requests and responses … without inspecting plaintext
contents" and "observes message boundaries, timing, and size but cannot decrypt
payloads", while the gateway "learns the decrypted request content but not the
client's IP address" — and states the distinct-entity rule as protocol text,
"the Oblivious Relay Resource cannot be operated by the same entity as the
Oblivious Gateway Resource", which is FR-039b's distinct-legal-entity rule
stated by the specification rather than invented here. It is deployed at
consumer scale, and Rust implementations exist for both layers
(`martinthomson/ohttp`, `rozbb/rust-hpke`; no paid formal audit was located for
the latter), so the encapsulation, the response path and the wire format are not
this project's invention. The one deliberate departure is required and is
strictly stronger: RFC 9458 deliberately does not define how clients acquire key
configurations, and the common deployment fetches them from the gateway — an
unencapsulated request straight to the gateway that reveals exactly the source
address the relay exists to hide, accounted for by no entry in FR-007a's closed
list — while FR-039b's "pinned in the build and rotated only by a release"
removes that request entirely. *Rejected*: plain TLS to an EU endpoint under a
contractual no-log promise (FR-039b's own text names "a terminating proxy run on
the receiving service's own account" as the arrangement the requirement exists
to prevent); DAP/Prio with two non-colluding aggregators (genuinely aggregate by
cryptographic construction and arguably stronger for FR-039a's three counters,
but it cannot carry FR-039c's high-cardinality symbolised stacks, it needs two
aggregators rather than one relay, and FR-039b mandates encryption to a pinned
receiving-service key, which is a different construction — reopening it is an
amendment); Tor or a mixnet (rejected on the size, start-up and reliability
budgets Principle II gates). *Established (the working-group source repository
carries the RFC text; direct RFC-editor and datatracker fetches were blocked
from the investigating session) · ietf-wg-ohai/oblivious-http; FR-007a,
FR-039b.*

**8.5 Pin two key configurations per release and rotate by promotion. [GAP]**
Current and next, each with its own 8-bit key identifier, with the gateway
retaining a private key for as long as any supported release pins it. Pinning
plus release-only rotation means a key cannot be retired faster than the slowest
updater: with a single pinned key, rotation silently breaks every install that
has not yet updated, their reports becoming undecryptable and dropped — which
fails safe for confidentiality but loses a cohort invisibly, biasing the
retention figure by an amount nobody can see. OHTTP's key configuration carries
an 8-bit key identifier precisely so a gateway can hold several concurrently.
**State the honest cost rather than eliding it:** HPKE base mode gives no
forward secrecy against compromise of the receiver's static key, so recorded
ciphertext is decryptable by whoever later holds it; the real mitigations are
EU-resident HSM or KMS custody, a short pinned-key lifetime tied to the release
cadence, and the fact that the plaintext is at most an enrolment week or a list
of Evreos's own symbol names. Claiming the design is forward secret is not a
mitigation. *Rejected*: one pinned key (no overlap window); a fetched key
configuration (§8.4); rotation by a configuration file the client updates out of
band (FR-039b says "rotated only by a release", and an updatable config is a
channel through which an attacker-held key can be substituted). *Indicative; the
forward-secrecy property is stated from RFC 9180 and should be re-verified
against the RFC text before it enters an ADR · ietf-wg-ohai/oblivious-http;
FR-039b.*

**8.6 Fixed-length padding across all four report kinds, and one report per
connection. [GAP]** Pad every encapsulated report to one fixed ciphertext
length, identical for enrolment, retention, withdrawal and crash; send at most
one report per connection and close after the acknowledgement. FR-039b concedes
that the relay sees "the ciphertext, its length and the destination", and with
four report kinds of naturally different sizes, length alone tells the relay —
which does see the source address — whether a given address withdrew, or
crashed: a per-address behavioural fact about a member, and exactly what
"structurally blind" is meant to exclude. The specification permits length to be
observed; it does not require length to be informative, and RFC 9458's security
considerations name traffic analysis on size and timing and point at padding,
which binary HTTP supports. One report per connection stops the relay linking a
crash report and a retention report to one source address by co-occurrence. The
cost is real and must be stated under FR-043: the fixed size must exceed the
largest crash report, so every enrolment report pays for the largest stack — a
further reason to bound the captured frame count (N11). *Rejected*: length
bucketing (buckets still separate the kinds); no padding (it hands the relay a
per-address withdrawal and crash signal); batching several reports per
connection (it creates the co-occurrence linkage). *Indicative · FR-039b;
ietf-wg-ohai/oblivious-http.*

**8.7 The delivery acknowledgement is encapsulated, and relay ingress must
resolve only to EU infrastructure.** FR-039b requires the relay to forward the
acknowledgement on the same connection without retaining it, and a bare HTTP
status would tell the relay, per source address, whether a report was accepted —
for instance whether a crash key had already been counted, which is a fact about
that machine's history; encapsulation makes acceptance and rejection
indistinguishable to the relay. On hosting: FR-039f binds what is "received,
processed and retained", and the relay receives the report, ciphertext or not,
and processes a source IP address, which is personal data whatever the payload
is — so a global anycast relay whose nearest point of presence may sit outside
the EU does not satisfy FR-039f on its face, and EU-only ingress has to be a
contractual term rather than an observed default. *Rejected*: accept a non-EU
relay on the argument that ciphertext is not personal data — the relay's
processing of the source address is the processing FR-039f's hosting rule has to
reach, and FR-039f binds on receipt of the payload rather than on readability of
it. **This reading should be confirmed by counsel rather than settled here
(§12.4).** *Indicative; the legal characterisation is unestablished · FR-039b,
FR-039f.*

**8.8 Plan the signal to ship dark; the release gate is a contract, not code.**
The milestone that ships diagnostics must be able to ship with the whole feature
unofferable, and the client must have a build-time state in which no report path
exists at all — which is also the state SC-014's capture exercises on a fresh
profile. FR-039b is unusually explicit: "Where no operator is named or no such
contract is in force, the diagnostic signal MUST NOT be offered and no report
may be transmitted." The relay operators located are Fastly (which operates
Firefox's) and Cloudflare, both US-incorporated, while FR-039b requires the
entity and its jurisdiction named in the pre-consent disclosure and FR-039f
requires EU infrastructure; whether either will contract for EU-only ingress
plus the three no-retention obligations FR-039b enumerates is not established,
and **no EU-native OHTTP relay operator was located — which is a failed search,
not a negative finding.** Start that procurement and the DPIA in Phase 1, in
parallel with everything else. *Rejected*: operate the relay ourselves through a
second legal entity the founder controls — rejected, and rejected in writing
because it is the shortcut that will otherwise be taken: FR-039b's point is a
party that does not answer to the receiving service, and a controlled entity is
in substance the "different party that nevertheless sees both the source address
and the payload" the requirement names. *Established for FR-039b's bar and for
the two operators located · FR-039b, FR-039f.*

**8.9 The client symbolises, from a compact symbol table shipped in the
installer** — generated from that release's own debug information, with a
SymCache built by `symbolic` (MIT, reads PDB and DWARF) as the primary candidate
and a bespoke sorted table of function-start RVA to symbol name as the fallback.
This is settled by the specification rather than chosen: FR-039c enumerates the
three fields a frame may carry — module name, symbol name, source file and line
— and a raw address or module-relative offset is not among them. It is also the
better privacy answer, worth recording so nobody reopens it on efficiency
grounds: a module-plus-offset stack is *finer-grained* than a symbolised one,
since an offset distinguishes call sites, inlined copies and code-layout
variants inside a single function where the symbol name collapses all of them;
and server-side symbolisation would additionally require the gateway to hold the
report long enough to perform a lookup, which sits badly with FR-039d's "added
to its counter on receipt and discarded". The table is build output, so its
bytes are an FR-043 cost stated in the pull request that adds it and gated by
SC-001 from M0. The tension to plan for rather than assume away:
function-name-only tables are small while file-and-line requires line tables and
is the expensive part (N11, §12.3). *Rejected*: a client-queried symbol server
(rejected outright — a per-crash network request keyed to the crashing code
path, made by an identified client outside the relay, accounted for by no entry
in FR-007a's closed list, reintroducing exactly the correlation surface the
design exists to remove); ship the PDB (rejected on size, by an order of
magnitude); restrict the table to crash-prone paths (unpredictable, and a crash
in an unlisted function then symbolises to nothing, which is the case you most
need); symbolise from the export table (a Rust static binary exports almost
nothing). *Established for the FR-039c constraint; indicative for the candidate
and its cost · FR-007a, FR-039c, FR-039d, FR-043, SC-001; getsentry/symbolic.*

**8.10 Two disjoint crash-reason families, because on tier 1 web content does
not run in Evreos's process.** The FR-039c closed enumeration must carry
Evreos-process crash reasons **and** engine-process failure kinds mapped
one-to-one from WebView2's own closed enumeration, with an engine-process
failure report carrying a reason code and an empty frame list. FR-039c scopes
symbolisation to "Evreos's own debug information", and Evreos has debug
information for nobody else's binaries, so system-runtime and OS frames carry a
module name and nothing more. More consequentially, Microsoft's documentation
states that WebView2 runs browser, renderer, GPU and utility processes as
separate OS processes from the host app, so a page crash is not an Evreos crash
and produces no Evreos stack at all. What Evreos can observe is the
`ProcessFailed` event and its `CoreWebView2ProcessFailedKind` — already a closed
enumeration (BrowserProcessExited, RenderProcessExited,
RenderProcessUnresponsive, FrameRenderProcessExited, GpuProcessExited,
UtilityProcessExited and others), with a companion
`CoreWebView2ProcessFailedReason` — and mapping those into the committed
enumeration is what makes the most common crash a member actually experiences
produce any diagnostic at all; without it, crash reporting covers only the shell
and misses the failure mode that matters most. FR-039d's counter key tolerates
the empty symbol list, since the key *is* the symbol list. *Rejected*: read
WebView2's own Crashpad minidumps from the user data folder (rejected outright —
a renderer minidump contains page memory, hence URLs and page content, which
FR-039c bans capturing and FR-007a bans transmitting; those dumps must be
deleted, never parsed, §2.6); ignore engine-process failures (it makes the
feature blind to the failure mode that matters most). *Established on tier 1;
the macOS analogue — `webViewWebContentProcessDidTerminate:` as a reason source
— is indicative, Apple's documentation being unreachable from the investigating
session (N11) · MicrosoftDocs/edge-developer `process-related-events.md`;
WebView2Feedback `diagnostics/crash.md`; FR-039c, FR-039d.*

**8.11 At crash time capture only a return-address list, the module set and the
reason code; never a minidump.** Symbolise, deduplicate against the FR-039e cap,
and send on the next launch. FR-039c bans "strings read from the heap or the
stack" and bans full process memory; a minidump, including the small kinds,
includes thread stack memory, which is where stack-resident string fragments and
pointers live, and there is no minidump flag whose output is provably free of
them — FR-039c's own reasoning for banning full memory, that forbidding URLs
inside a memory image is not implementable, applies to a partial image too. A
return-address list is provably free of them by construction: it is a list of
code addresses. Symbolising at crash time is also the wrong moment, since it
allocates and reads a large table inside a process that has just faulted, and
deferring to next launch is additionally what makes FR-039e's once-per-install
cap enforceable at all, because that cap is keyed on the symbolised stack, which
does not exist until symbolisation has run. *Rejected*: Crashpad, Breakpad or
the `minidump-writer` crate (rejected on the content ban despite being the
mature and well-trodden path — **the cost is that Evreos writes and proves its
own capture on both tiers, which must be scheduled as real work rather than
assumed cheap**); symbolise in-process at crash time (robustness). *Indicative ·
FR-039c, FR-039e.*

**8.12 Coarsen the operating-system version in the report and in the counter
key. [GAP]** Use the marketing or build level rather than the full patch
quadruple, and commit that granularity in the same file as the reason-code
enumeration. Privacy and utility point the same way here, which is rare enough
to act on: a full version string such as `10.0.26100.4652` is close to
identifying in a cohort of a few thousand, and it fragments the crash counter so
finely that no key ever reaches FR-039e's 50 — which would make every crash
permanently unpublishable while the counters are still held. FR-039d permits
"operating-system version" without fixing a granularity, so this is an
implementation decision that has to be recorded rather than inferred, and
placing it beside the reason codes makes widening it as visible as adding a
code. *Rejected*: the full version including the update-build revision (rejected
on both grounds above); major version only (it cannot distinguish a Windows 11
24H2-specific regression, which is the diagnostic case the feature exists for).
*Indicative · FR-039d, FR-039e.*

**8.13 The receiving service has no report store.** Receipt, counter increment
and acknowledgement are one transaction against a counter table, and the report
exists only as a request body in memory, so FR-039d's "discarded by the end of
the following calendar day" is met by construction. A retention deadline
enforced by a deletion job is a deadline that fails silently when the job fails,
and FR-039d exists precisely because retained reports are the risk; a design in
which retention is structurally impossible is auditable by a third party reading
the published schema — the proof is the absence of any table a report could sit
in — which fits SC-013's publication discipline and matches this repository's
habit of making a rule executable rather than asserted, as `check-budgets.py`
does for Principle II. It also composes with the acknowledge-after-commit rule:
one transaction, one durable increment, then the encapsulated acknowledgement.
*Rejected*: a queue with a nightly purge (a queue is a report store, and a
stalled consumer is an undetected retention breach); a write-ahead or request
log (the same, and FR-039b independently bans a receipt timestamp finer than the
day, which most logging defaults violate). *Indicative · FR-039b, FR-039d.*

**8.14 The FR-039e contribution cap is a local, append-only key set, and
publication is a committed script.** The cap is a local, append-only set of
(symbolised stack, release, OS version, reason code) keys held in the same state
file, never cleared, consulted before any crash report is emitted; publication
is a script in this repository that takes the counter export and emits the
published figures, implementing FR-039e's disclosure units, the
enrolments-less-withdrawals gate, decade banding, whole-percentage rounding, the
"not computable" case for a non-positive denominator, and the
withheld-and-stated case, and refusing to emit anything drawn from a held unit.
FR-039e requires life-of-install rather than per-day and gives its own reason:
one machine in a crash loop reaches 50 over 50 days under a daily cap, which is
exactly the single member on a single code path the threshold exists to
suppress. The set is small — bounded by the number of distinct crash keys one
install actually hits — and because the key carries the release, its effect
resets naturally at each release without the set ever being cleared, which is
the property that makes "50 reports under one key are 50 distinct installs"
hold. On publication: FR-039e is a rule with at least six branches, and the
branch easiest to get wrong is the one that *publishes* — a qualifying week is
always publishable, so the script must also refuse to over-withhold, which
FR-039e explicitly forecloses. Making the disclosure rule executable is what
makes SC-013's reproducibility claim true of the privacy figures rather than
only the performance ones. *Rejected*: server-side deduplication (impossible
without an identifier, which FR-039b forbids); a per-day cap (rejected by
FR-039e's own text); adding a client credential so the endpoint could reject
fabricated streams (forbidden by FR-039b, and FR-039a already states what that
leaves the published figure worth — the plan must not try to fix it); a
documented checklist followed at publication time (rejected on exactly the
reasoning Principle II gives for budgets). *Established for the cap; indicative
for the script's shape · FR-039a, FR-039b, FR-039e, SC-013.*

**8.15 FR-039f is a property of five things, and the fifth is the one that gets
missed** — relay ingress, gateway, counter store, publication pipeline, **and
the gateway's own operational instrumentation**. The gateway ships with no
third-party observability; its only outputs are the counter table and a coarse
liveness signal carrying no per-request record. A gateway instrumented with a
US-hosted APM or error-tracking service exports request-level records —
including receipt timestamps finer than the day, which FR-039b bans
independently of where they are hosted — outside the EU, and FR-039f binds
"every counter and figure derived from them", which an APM trace of a counter
increment is. Choosing an EU-region vendor fixes the hosting half and leaves the
timestamp half broken, so the right answer is no per-request instrumentation at
all rather than a compliant vendor. *Rejected*: EU-region APM or error tracking
(rejected on the FR-039b timestamp ban, not on hosting); sampled request logs (a
sampled per-request record is still a per-request record). *Indicative ·
FR-039b, FR-039f.*

---

## 9. Performance measurement (SC-001 – SC-006, SC-013, FR-038, FR-043)

**9.1 The engine seam is already the instrument for SC-002.** Build the shell
twice against the existing seam — once linked to `evreos-engine-headless`, once
to the system-webview backend — on the same commit and the same pinned runner;
the difference is the runtime's contribution, and the headless figure is the
shell's floor. **Both builds must reach the identical, shell-emitted
"interactive" event, which means the headless configuration must drive a real
window** — an M0 requirement on the shell rather than on the harness, since the
harness cannot invent an event the shell does not emit. SC-002 states that "a
large share of each is the engine's own initialisation rather than Evreos's
code" and holds all four entries provisional for that reason, and ADR-0001
expects the same; a difference measurement needs two builds sharing every line
of shell code and differing only at the seam, which is exactly what Principle
III already forced into existence — `crates/evreos-shell/src/main.rs` is generic
over `Engine`, and `evreos-engine-headless` renders nothing, so it contributes
~0 ms of engine initialisation by construction. The seam built to prove a
principle is the measuring instrument for the budget. *Rejected*: a sampling
profiler attributing startup time to modules (SC-013 requires a third party to
obtain the same figures, and a profiler-derived attribution is not reproducible
without the same profiler, the same symbols and the same judgement about which
frames are "the engine"); phase markers alone (kept, but they cannot answer
"what would this cost with no engine at all"); a standalone WebView2/WKWebView
microbenchmark (it does not share the shell's code path, so the difference is
not attributable to the shell). *Established · `feat/engine-seam`; SC-002,
SC-013; ADR-0001 benchmark honesty.*

**9.2 A plain monotonic-clock phase log is the published record** —
process-creation time as t₀, then marks at main entry, engine construction call,
engine-ready callback, window shown, first frame presented, interactive; emitted
by the shell, published verbatim under SC-013, with phase names identical across
tiers so the two tiers' records are comparable line for line. ETW and
`os_signpost` are an optional local-diagnosis overlay, never the published
record: SC-013's reproduction requirement decides the format, because a CSV of
timestamps can be re-derived by anyone with the published binary and script,
while an ETW trace or an Instruments recording cannot. *Rejected*: ETW or
signpost as the primary record (SC-013 reproducibility); wall-clock timestamps
(not monotonic, and the quantities are tens of milliseconds). *Indicative; the
per-platform process-start-time call on tier 2 is unverified (N12) · SC-013,
FR-038.*

**9.3 Define SC-002's endpoint by SC-006's instrument. [GAP]** "An interactive
window appears" is the first presented frame at which an injected address-field
keystroke is accepted and produces a visible response, and the harness proves it
by injecting at that instant rather than asserting it. The specification states
the figure and never defines interactivity, and no definition was located in the
specification, the constitution or ADR-0001; an undefined endpoint cannot be
reproduced by a third party, so SC-013 fails on SC-002 however carefully the
milliseconds are measured. Binding the endpoint to SC-006's injector also means
the two criteria share one instrument and one definition of "visible response",
so they cannot drift apart. *Rejected*: "window shown" (first
`ShowWindow`/`orderFront` — a window can be on screen and not yet accept input,
which is the exact defect this criterion exists to catch); "first frame
presented" (the same objection, one step later); a human judging it (not
reproducible). *Unestablished; a founder decision recorded where SC-002 is
stated (§12.3) · SC-002, SC-006, SC-013.*

**9.4 Fix the machine cache state explicitly in each cold-start entry's
`condition`** — a reboot before each cold trial, with the tier-1 prefetch and
SysMain state and the tier-2 equivalent stated rather than left to the operator,
and the preparation script published. SC-002 defines cold start as "the first
launch after installation, on a fresh profile, with no Evreos process running
and no cached profile state on the machine": every clause is about Evreos's own
state, and none is about the operating system's file cache, prefetch database or
standby list, so two labs following the text exactly will differ in whether they
reboot — and start-up time is precisely the quantity that difference moves,
while SC-013's declared tolerance is capped at 5%, smaller than the plausible
size of the effect (N7). *Rejected*: leave it to the operator (that is the
SC-013 failure); mandate `purge` on tier 2 (it requires root, so it is not a
step a third party can be assumed to take, whereas a reboot is universally
available and is the same instruction on both tiers). *Established · SC-002,
SC-013.*

**9.5 Discharge SC-005's "no periodic timer outside the enumeration may exist"
at build time for Evreos's own code. [GAP]** One timer facility in the shell, as
a module rather than a crate so its byte cost stays nil; arming a timer requires
an identifier, and a `build.rs` step parses `budgets.toml`'s wake enumeration
and generates the permitted identifier set, so an unenumerated wake is a
**compile error** — reinforced by a `clippy.toml` `disallowed-methods` deny-list
over every other sleep or timer entry point, and by the
`#![forbid(unsafe_code)]` the shipped crates already carry, which forecloses raw
FFI timer calls. The engine's residual wakes are measured separately and
reported, since they are not enumerable. SC-005 supplies the argument itself:
the enumeration is "verified by design review and by instrumentation of
scheduled work rather than by observation, since no finite window can falsify a
timer with a longer period". No sampling window discharges that; a constraint at
construction does, because it holds over the whole program rather than over the
window that was watched — and since the budget file is already the enumeration's
home, making the build read it is what keeps the file and the binary from
diverging: a wake added in code without a budget-file entry does not compile,
which is stronger than "states its cost under FR-043". *Rejected*: `powercfg
/energy` timer-request stacks and ETW timer-set events (retained as a
cross-check for timers we did not author, rejected as the primary mechanism
because they are observation); design review alone (the specification names
instrumentation alongside it); a runtime registry checked at start-up (weaker —
it fails on the machine rather than in CI, and only for code paths the run
reached). *Established for the requirement; the mechanism is a **[GAP]** ·
SC-005, FR-043.*

**9.6 Measure the engine's idle floor per tier before SC-005 is treated as
achievable (N1).** A 60-minute idle measurement of a bare system-webview window
with one suspended tab, on each pinned runner, scheduled alongside the
cold-start spike rather than after the harness is built; instrumented with
per-process user and kernel time at 1 Hz plus a lifecycle source that catches
processes living less than one sample — a Windows Job Object's
`JOBOBJECT_BASIC_ACCOUNTING_INFORMATION`, and on macOS a higher-rate
process-list poll — and reporting the kernel's own wake counters beside the CPU
figures (`task_power_info`'s interrupt, platform-idle and timer wakeups,
available per process through `proc_pid_rusage`). This is the largest exposure
in the measurement work. SC-005 bounds processor use over "the same processes
SC-004 counts", which includes the WebView2 browser, renderer, network and GPU
processes on tier 1 and WebKit's WebContent, Networking and GPU processes on
tier 2 — whose idle timers Evreos does not author, cannot enumerate and cannot
remove. SC-005 is **ratified**, so under the preamble it is tighten-only and can
be relaxed only by an amendment to the specification recording the founder
decision, the measured evidence, and what discipline replaces the budget
removed; SC-002 was deliberately held provisional for structurally the same
reason, and ADR-0001 records no idle-CPU floor anywhere. If the engine's own
idle floor exceeds 5 ms of processor time in some 1-second sample on an
8th-generation i3, SC-005 is unmeetable and the remedy is a specification
amendment rather than a code change — learning that early costs one afternoon,
learning it after the harness is built costs the harness's assumptions too.
Polling per-process times at 1 Hz alone silently misses a process that starts
and exits inside a sample, which is precisely the shape of a scheduled wake that
spawns a helper, and the Windows job object's `TotalUserTime`/`TotalKernelTime`
are documented to cover terminated processes, so the job is the only Windows
instrument that cannot lose that work. *Rejected*: assume the engine idles at
zero (the assertion the constitution's measurement discipline exists to prevent,
and ADR-0001 gives no evidence either way); scope SC-005 to the shell process
only (not available — SC-005 names SC-004's process set explicitly, precisely so
work cannot be hidden by relocating it into a runtime process);
`dtrace`/`ktrace` on macOS (SIP-restricted, and a benchmark a third party must
disable SIP to run is not reproducible under SC-013). *Established that the
floor is unmeasured and that SC-005's scope includes it · SC-002, SC-004,
SC-005, Success Criteria preamble; Microsoft Learn; xnu headers.*

**9.7 SC-004: enumerate processes by API on tier 1 and by construction on tier
2; read the counters with `PrivateUsage` and `ri_phys_footprint`; supply the
shared-section term from a declared inventory; and validate the macOS
cross-check quantity before declaring any margin.** Tier 1 uses
`ICoreWebView2Environment6::GetProcessInfos` plus the `ProcessInfosChanged`
event, cross-checked against a Windows Job Object process-id list containing the
shell and every descendant: WebView2 documents the process collection and its
kinds — Browser, Renderer, Utility, SandboxHelper, GPU, PpapiPlugin, PpapiBroker
— which covers SC-004's "host, content, network and GPU processes" without
matching it term for term, since the enumeration carries no network kind of its
own and carries kinds SC-004 does not name, so the mapping from process kind to
SC-004's set is a statement the published sampling script has to make rather
than an identity that can be asserted. The job object adds what the WebView2 API
cannot see, namely any runtime host Evreos spawns itself, which SC-004 also
counts. Tier 2 runs the soak under a dedicated user account on the pinned runner
with no other WebKit client present, enumerating the whole process list and
filtering by WebKit executable path, because there is no public API attributing
a `com.apple.WebKit.WebContent` process to its host application — Apple's own
developer support calls this the responsibility problem and says there is
virtually no API surface for it outside Endpoint Security — so the boundary
holds by construction and the limitation is published under SC-013 rather than
hidden. The counters are `GetProcessMemoryInfo` →
`PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`, documented as "Same as PagefileUsage"
and therefore the private commit charge SC-004 names, needing only
`PROCESS_QUERY_LIMITED_INFORMATION` for a same-user process with no elevation;
and `proc_pid_rusage(pid, RUSAGE_INFO_V4).ri_phys_footprint`, the same quantity
as `task_vm_info.phys_footprint`, readable for same-user processes with no
entitlement and what WebKit itself uses to report its own footprint. SC-004
requires memory the shell places in shared sections counted once, but a
page-file-backed shared section is by definition not private and appears in no
process's `PrivateUsage`, so summing drops it entirely and there is no way to
recover it from the per-process counters: it must come from an inventory the
shell publishes about itself, added once by the sampler, with the whole-machine
cross-check being what fails when a section exists but was not declared — and
the published sampling script should say so, because that is the cross-check's
actual job. That cross-check is `GetPerformanceInfo().CommitTotal × PageSize` on
Windows, a system-wide commit charge commensurable with the summed per-process
commit charge; macOS has no "committed memory", and the nearest system-wide
figures from `host_statistics64(HOST_VM_INFO64)` are not obviously the same
quantity as summed `phys_footprint`, so the quantity must be defined in the
published script and validated against a known allocation before any tier-2
margin is declared (N12) — SC-004 requires the margin "declared and justified
exactly as a tolerance is", and a margin declared between two incommensurable
quantities is a number with no content. *Rejected*: `task_for_pid` plus
`task_info` for other processes (restricted to development tools with a
debugging entitlement, and a benchmark that needs one is not a benchmark a third
party can rerun); `responsibility_get_pid_responsible_for_pid` (private SPI,
acceptable in a dev-only harness but it can disappear without notice and cannot
be reproduced by a third party reading only public documentation); Endpoint
Security with `responsible_audit_token` (an Apple-granted entitlement for a
benchmark harness — disproportionate and itself unreproducible); process-tree
walking on macOS (WebKit's helpers are XPC-launched and are not children of the
app); a general-purpose crate such as `sysinfo` (resident-set-shaped figures,
which SC-004 rejects by name); proportional apportionment of shared pages
(explicitly ruled out by SC-004 — a Linux `smaps` construct, and the nearest
Windows equivalent caps its share count at 7); using resident or physical memory
for the macOS cross-check (the same ground SC-004 rejects resident-set counters
on); skipping the cross-check on tier 2 (SC-004 requires it on both entries, and
an undeclared margin is zero). *Established, except the macOS cross-check
quantity, which is unestablished (N12) · WebView2Feedback
`specs/ProcessInfo.md`; Apple Developer Forums 739414 and 769021; Microsoft
Learn; WebKit `ProcessMemoryFootprint.h`; xnu `resource.h`; SC-004, SC-013;
ADR-0001 risk 9.*

**9.8 Two page sets, not one: a pinned gating corpus and a live, non-blocking
observatory.** The gating ten-tab set is ten pages archived on a recorded date,
content-addressed, served from loopback by the harness and published under
SC-013, and it includes at least one first-party app surface; a second, live set
drawn from ADR-0001 risk 2's German site matrix runs on a schedule as a
non-blocking observatory. **The founder names the ten** (§12.3); what is
proposed here is the rule for choosing them — the cohort's daily surfaces, not a
synthetic benchmark. SC-013 requires a third party to obtain the same figures,
and a live page's payload changes between two runs by more than a memory
regression does, varies by the runner's country, and drags network variance
inside a memory gate, so a live set cannot gate; but a corpus that stops
resembling the web stops measuring anything, and ADR-0001 risk 2 already names
the sites whose behaviour actually decides whether this cohort keeps the
browser. Running both, with only one blocking, keeps the gate reproducible and
the corpus honest. An app surface belongs in the gating set because FR-016 and
FR-019 apps render inside the shell and their memory is Evreos's memory under
SC-004's boundary. *Rejected*: live sites in the gate (above); a standard suite
such as Speedometer or JetStream (ADR-0001's benchmark-honesty cost holds that
"the public benchmark measures only what is ours", and those suites measure the
engine, which on tier 1 is Chromium). *Unestablished as to the ten pages; the
rule is a proposal · SC-004, SC-013; ADR-0001 risk 2.*

**9.9 SC-006: injected-input timestamp to compositor present-to-display, with
the proxy's bias published, and the chrome decision taken first.** On Windows,
inject with a recorded `QueryPerformanceCounter` timestamp and read the display
time with PresentMon, whose `MsUntilDisplayed` is the time between the Present
call and when the frame was displayed and which since 2.3.1 no longer requires
administrator rights; on macOS, post a synthesised event and take the
display-link callback time for the frame carrying the response. Characterise the
proxy once against a photodiode and publish the bias; do not present the proxy
as photon-out. **Order the plan so the chrome-rendering decision precedes SC-006
instrumentation** — the injector and the present-timeline reader can exist at
M0, but the shell-side marker lands with the chrome. "A visible response" is
only unambiguous at the photon, and a photodiode rig is the one instrument a
third party can rebuild from a published design, but it cannot run per-commit in
CI, so the gate needs a software proxy and present-to-display is the last
boundary the machine can observe; publishing the one-time delta between the two
is what keeps the proxy honest rather than merely convenient, and SC-006's hard
per-trial maximum of 16 ms means a systematic bias of even 2 ms decides pass or
fail. On ordering, ADR-0001 states that what renders the chrome is spike S4's
output, so until that is decided there is no shell-side instrumentation point to
put a timestamp on and no way to know whether the response is presented by our
compositor path or the engine's — and measuring tab switch and keystroke through
different paths on the two tiers would mean the figure does not mean the same
thing twice. Two further conditions: fix the **local profile corpus** as a
stated measurement condition on SC-006's address-field entry — a generated
history and bookmark set of published size and shape, with a second, larger
corpus as a non-blocking scaling check — because FR-003 combines search, history
and bookmarks in one field and FR-007a requires suggestions produced only from
data already on the machine, so keystroke response time is a function of how
much local data there is, and the budget file states no profile condition today;
and make the **discard ledger** durable state indexed by head commit SHA,
appended to the published run record and read by the gate before it decides,
because SC-006 requires the discard budget "counted cumulatively across every
run of the gate on that commit". *Rejected*: measuring to the application's own
"I painted" callback (it excludes composition and scan-out, which is most of the
perceptible delay, and would let the gate pass a build that visibly drops
frames); measuring photon-out per commit (not runnable in CI);
`DwmGetCompositionTimingInfo` alone (narrower than PresentMon, and it does not
attribute frames to a process); measuring on a fresh profile and saying nothing
(the current implicit position, and it fails SC-013); capturing a live
member-shaped profile once (it would contain browsing history, which FR-007a
governs and which cannot be published under SC-013); keeping the discard count
in the CI run's own state (it does not survive a re-run, which is the case the
rule is about) or in a file committed to the repository (a commit changes the
head SHA and so resets the budget it was meant to constrain). *Indicative;
PresentMon's attribution of a DirectComposition-hosted WebView2 window is
unestablished (N7) · GameTechDev/PresentMon; ADR-0001 rationale 1; FR-003,
FR-007a, SC-006, SC-013.*

**9.10 Do not assume the per-entry tolerance is a valid acceptance band for
SC-013 (N7).** Measure the same commit on two or three further machines of each
reference class and compare the observed spread against the declared tolerances
before publishing any figure as reproducible. The preamble requires each
tolerance to be "justified by measured run-to-run variation on the pinned
runner" and caps it at 5% of that entry's recorded baseline — one machine
repeating itself — while SC-013 then reuses that band as the acceptance window
for "a third party rerunning them on a machine of the reference class", which is
a different machine, and "reference class" is a rule that admits many distinct
models. Machine-to-machine variation within a class is a different and normally
larger quantity than run-to-run variation on one machine; if it exceeds the
declared tolerance, SC-013 is unsatisfiable as written with an honestly declared
tolerance, and one of the two rules needs an amendment. *Rejected*: declare
tolerances against cross-machine spread instead (it breaches the preamble's
stated justification rule and the 5% cap); assume the two variations are close
(the assertion Principle II exists to forbid). *Established as a structural
mismatch; the magnitude unestablished · Success Criteria preamble, SC-013,
Assumptions.*

**9.11 `budgets.toml` and its gate are roughly a quarter built; land the missing
schema before any measurement lands.** Verified in this repository against
`feat/budget-gate`. (a) The file carries only SC-001's four entries, while the
preamble's closed list is nine per platform and eighteen in all — missing are
SC-002 warm and cold, SC-004 ten-tab, SC-005 window and wake-free sample, and
SC-006 tab switch and keystroke, on each tier. (b) `check_budget_file` iterates
only over declared entries and never compares them against the closed list, so
its docstring's "fails when an entry a criterion states is missing" does not
happen. (c) There is no check that an entry recorded `ratified` names a founder
decision, which the preamble requires the gate to fail on. (d) There is no
`cross_check_margin` field or check for SC-004, where an undeclared margin is
zero. (e) There is no SC-005 wake enumeration and no check that each wake
carries a period, a processor-time bound and a justifying requirement — again a
stated budget-file-gate failure condition. (f) The schema is
`figure_mb`/`baseline_mb` throughout, but SC-002 and SC-006 are milliseconds and
SC-005 is a percentage of one core plus two processor-time bounds, and there is
no unit. (g) There is no spike-exemption field, and nothing implements the
preamble's requirement that the release job refuse an artefact built from a
commit whose budget file records an unretired exemption. (h) The runner blocks
carry no display refresh rate, which SC-006's 60 Hz condition needs. (i) **A
confirmed defect**: `measure_download_size()` reads
`target/release/evreos-shell`, a Linux ELF built on `ubuntu-latest`, and
`run_gates` keys measurements on `(criterion, name)` with no platform, so that
single Linux number is compared against **both** the `windows` and the `macos`
download-size entries — and Linux is the deferred platform, so neither entry's
stated condition ("the installer artefact CI publishes") is met by it. (j) Every
SC-001 entry carries `baseline_mb = 0.0`, and the script skips the regression
comparison when `baseline > 0` is false, so the regression half of the gate on
those entries is inert until the first real measurement writes a baseline —
**the commit that first measures must also set it, or the gate stays inert
indefinitely.** *Rejected*: add entries as each measurement lands (the
budget-file gate is unconditional from M0 and is specifically what bounds the
advisory period on the measuring gates, so an incomplete file is a gate that
cannot fail); keep `figure_mb` and encode milliseconds in it (it makes the file
unreadable and the tolerance arithmetic silently wrong across units).
*Established, read directly · `feat/budget-gate:budgets.toml`,
`:scripts/check-budgets.py`, `:.github/workflows/build.yml`; Success Criteria
preamble, FR-043.*

**9.12 Start runner procurement now, buy a cold spare per tier, restrict the
hardware-dependent jobs to non-fork pull requests, record the engine runtime
version, and build the SC-013 publication set before the first figure is
recorded.** The preamble disqualifies fungible hosted machines, so both
measuring gates need self-hosted machines that do not exist yet, and the
budget-file gate is presently suppressed for exactly this by
`--allow-unpinned-runners` in the build workflow; procurement is therefore the
longest-lead item in this strand and gates SC-002's spike, SC-004's soak,
SC-005's window, SC-006's trials, S4's decision and every tier-specific
measurement above — which the specification already states (Q-E9a: "procurement
is a release prerequisite and is the only thing standing between those gates and
blocking"). `Nomos-N4s/evreos` is public, and GitHub's own guidance is that
self-hosted runners should not be used with public repositories, because anyone
who can fork and open a pull request can execute code on the runner — and a
compromised benchmark runner is also the machine holding the project's baseline
series. The spare matters because the budget file records "a durable machine
identifier": swapping in a different machine changes that identifier and,
honestly applied, restarts every baseline series on that tier, so the swap
procedure has to be written down rather than improvised the week a laptop dies.
Record the **engine runtime version** in the run record as a measurement
condition on every hardware-dependent entry, on both tiers, and do not freeze
it: ADR-0001 commits tier 1 to the evergreen WebView2 runtime and tier 2 to an
OS-shipped WebKit, so either can move SC-002, SC-004, SC-005 and SC-006 with no
change to Evreos, the regression gate will fire, and the preamble's only route
upward is a recorded founder decision — which the run record must carry the
evidence for; treat a runtime-driven baseline move as a named cause for such a
reset rather than as a code regression, and write that down as expected
behaviour before it happens rather than debugging it as a mystery. The **SC-013
publication set** is what makes reproduction possible and is the format every
earlier step writes into, so it must exist before the first figure is recorded:
pinned runner identity per tier (model, CPU, RAM, OS version and build, display
refresh and resolution, storage type, power profile, durable machine
identifier); commit SHA, toolchain version and target triple; the engine runtime
version for that run; the machine-preparation script including reboot policy,
prefetch and indexing state, power plan and network state; the measurement
scripts themselves (sampler, injector, present-timeline reader, phase-marker
reader); the ten-tab corpus as content-addressed archives with digests plus the
loopback server; **raw per-sample and per-trial data** rather than only
summaries (5,760 samples per process for an 8-hour soak, every one of the ≥1000
trials per interaction); the run record with each entry's verdict against figure
and baseline, the observed cross-check delta, and every discarded gate
invocation with its cause and its commit; the SC-014 traffic capture, script and
analysis; and a licence permitting rerun and republication, without which "a
third party rerunning them" is legally ambiguous. *Rejected*: hosted runners for
the measuring gates (excluded by the preamble); one machine per tier with no
spare (it leaves the entire regression history of a tier on a single 2017 or
2018 laptop); pinning the runtime version on the runner (it makes the gate
stable and the figure meaningless, since no member runs a frozen runtime);
widening the tolerance to absorb runtime drift (the preamble requires tolerance
justified by measured run-to-run variation on the runner, so absorbing a third
party's release into it is a misuse of that field, and it is capped at 5%
anyway). *Established · Success Criteria preamble, Q-E9a, Assumptions, SC-013,
SC-014, FR-038; `feat/budget-gate`; ADR-0001 platform tiering.*

---

## 10. Update, distribution and onboarding

**10.1 FR-014's staged rollout carries no per-install value off the machine.
[GAP]** The update service publishes a signed manifest containing a rollout
fraction; the client draws one random value locally at install, keeps it
locally, and decides its own inclusion; and the update artefact is verified
against a key pinned in the shipped binary **in addition to** the platform's own
code signature. A conventional staged rollout buckets clients server-side, which
means transmitting a stable per-install value on every update check, and FR-036a
forbids deriving a device or member correlator while being explicit that
lifetime is not the test; Principle VI's posture and FR-039b's identifier-free
design point the same way. A locally-evaluated fraction gives the same
operational control — hold at 5%, widen to 50% — with nothing per-install
transmitted, so the rollout mechanism never becomes the one place an identifier
lives. On the pinned key, FR-019a's reasoning transfers directly ("a compromised
host serves its own root alongside its own modified surface and the client
verifies both"), and an update is a larger prize than an app surface. SC-005
enumerates the update check as one of three permitted idle wakes, so it must be
coalesced with the platform's own scheduler, must not wake the machine from
sleep, and must complete within 50 ms of processor time. **Note:** SC-011's
signed-out opt-in rate is estimated against "an aggregate count of active
installs derived from the update channel, **once a requirement governs what that
channel may send and retain**" — issue #28 records that gap, the plan must not
outrun it, and until such a requirement exists the rate is unknown and must be
stated as unknown. *Rejected*: server-side bucketing on an install id (FR-036a,
and on posture — it hands the project a per-install identifier it would then
have to defend); rollout by version-pinned download URLs only (workable but
coarse, and it cannot hold a percentage); relying on the platform code signature
alone (it verifies the publisher, not the artefact's place in the release train,
and gives no downgrade protection). *Indicative; the mechanism is a **[GAP]** ·
FR-014, FR-019a, FR-036a, SC-005, SC-011.*

**10.2 FR-013 is registration plus guidance on tier 1, and SC-007 is a
measurement with a designed hand-off.** On Windows: register under
`RegisteredApplications` and the browser registration keys with ProgIDs for
`http`, `https`, `.htm` and `.html` so that Evreos appears in the Settings list
at all, then deep-link `ms-settings:defaultapps`. On macOS a documented API call
exists but is unverified here (N12). Treat SC-007's "without assistance" as a
moderated usability measurement on cohort-representative participants, never as
an implementation claim. There is no programmatic default-browser API for a
third party on Windows — Mozilla's platform-tilt tracker states it plainly, that
Windows does not support anything like it for third-party browsers and that
browsers are forced to deep link into the Windows settings UI, with Windows 11
offering a "Set default" button once there — so on tier 1 the last step of
SC-007's path is owned by an OS surface Evreos does not control and cannot
restyle, in a cohort of 40+ non-technical members arriving from a shop counter.
*Rejected*: registry manipulation to set the association directly (an
unsupported workaround of the SetUserFTA class; Windows guards the association
hash, and a browser that fights the OS on this is the wrong posture for a
product whose pitch is trustworthiness); skipping registration and only opening
Settings (without the `RegisteredApplications` entry Evreos does not appear in
the list the member is sent to, which is the worst possible version of this
flow). *Established on Windows; unestablished on macOS, Apple's documentation
being unreachable from the investigating session · mozilla/platform-tilt issue
10; FR-013, SC-007.*

**10.3 One installer artefact is served to everyone, and the claim code arrives
only by a deliberate act.** No per-partner or per-campaign build; the download
URL's parameter set is a closed allowlist of language and place, checked in the
FR-041 pre-release verification that already runs; the claim code reaches the
client only through the QR the member scans or the code they type, delivered
into the claim flow by a registered deep link (N12); and the installer hash is
published beside the download so the served artefact can be checked against the
CI artefact. FR-033 requires partner-referral attribution to come from a code
the member deliberately scans or types and forbids inferring it from the
installation, and Principle VI names install-referrer tricks alongside
fingerprinting as prohibited, with the Permanent Prohibition on silent affiliate
attribution standing behind both: a per-partner installer, or a campaign
identifier carried in the download URL and read back by the installer, is the
install-referrer trick in its most ordinary form, because it attributes without
the deliberate act. FR-041 already requires the distribution page's text,
download links and their parameters to keep language and place as two separate
values and to be verified before each release, so the allowlist check has a home
in a verification that already runs; FR-032's "opens the claim flow directly
after installation" then rests on protocol-handler registration at install time
on Windows and a URL type on macOS — a packaging task rather than a runtime one,
and one that must be tested per tier before it is claimed. *Rejected*:
per-partner installers with the campaign baked in (better conversion, and
prohibited); a one-time post-install fetch of a referrer token keyed to the
download session (the same prohibition at one remove). *Established · FR-032,
FR-033, FR-041; Principle VI, Permanent Prohibitions.*

---

## 11. Gaps — what this design needs that the specification does not require

Each is a proposal, not a requirement. To become binding, each needs either a
recorded founder decision in the plan or a specification amendment where the
affected requirement is stated.

| # | Gap | Serves | What would make it binding |
|---|---|---|---|
| G1 | `evreos-net` egress chokepoint with a closed `Purpose` enum and a dependency-graph CI assertion | FR-007a (shell half) | Plan decision; the enum's history-bearing variants must equal FR-007a's four entries exactly |
| G2 | Two-level signing keys: offline root pinned in the shell, online publishing key under a root-signed delegation | FR-019a | Founder decision on key custody, recorded as an ADR (§12.4) |
| G3 | A per-app **capability ceiling** in the shipped registry, intersected with the manifest | FR-017, FR-018 | Plan decision; it makes widening an app's ceiling a browser release |
| G4 | A **shipped roster** of first-party app identities the home surface may present | FR-016a | Plan decision; must state that it does not bind the root holder |
| G5 | `not_after` on the surface signature, and an anti-downgrade floor stored outside the FR-020 cache | FR-019a | Plan decision; expiry scoped to acceptance, never to rendering |
| G6 | Two pinned OHTTP key configurations per release, rotated by promotion | FR-039b | Plan decision; state the absence of forward secrecy honestly |
| G7 | Fixed-length padding across all four report kinds, one report per connection | FR-039b | Plan decision; state the byte cost under FR-043 |
| G8 | Coarsened OS-version granularity in the crash report and the counter key | FR-039d | Recorded in the same file as the reason-code enumeration |
| G9 | Local rollout-fraction evaluation, and an update key pinned in the binary | FR-014, FR-036a | Plan decision |
| G10 | A committed WCAG2ICT reading of FR-034/SC-008, with EN 301 549 named as the target | FR-034, SC-008 | Founder decision; adopting WCAG 2.2 instead would be an amendment |
| G11 | A 24×24 minimum pointer target and no hover-only affordances | Principle X | Founder decision — WCAG 2.1 AA does not require it (2.5.5 is AAA at 44×44) |
| G12 | `build.rs` compile-time enforcement of SC-005's wake enumeration | SC-005 | Plan decision; SC-005 requires instrumentation, not this instrument |
| G13 | A definition of SC-002's "an interactive window appears" | SC-002, SC-013 | **Specification amendment** — without it SC-013 cannot be met on SC-002 |
| G14 | Restore-as-suspended for FR-001 sessions | FR-001, FR-002, SC-002, SC-004 | Plan decision |
| G15 | Deny-all request gating on app-surface webviews, confined to the shell's custom scheme | FR-007a, FR-023, FR-040 | Plan decision; tier-2 feasibility rides on Q-E12 |
| G16 | A committed closed list of permitted **non-history** destinations that SC-014's classifier reads | SC-014 | **Specification amendment** — §4.2 |

---

## 12. Open questions

### 12.1 Mapped onto the spikes the specification already carries

- **Q-E10 — does affiliate attribution survive tracking prevention on tier 2?**
  Unchanged, and ADR-0001 risk 1 already makes it an ordering constraint: it
  must be answered before the wallet is designed around cookies. Two additions
  from this research. Run the measurement **twice on each tier, with the shipped
  blocking configuration enabled and disabled**, since Evreos's own lists may
  break the click-out redirect chain and whether they match this pilot's
  affiliate network is unverified. And schedule it **after** the tier-2 delegate
  replacement (§2.2), because the instrumentation point —
  `didReceiveServerRedirectForProvisionalNavigation:` — sits in the delegate
  that work writes, so the spike costs less afterwards. *Settled by*: a real
  affiliate redirect chain driven end to end on each tier's pinned runner, with
  the full signal recorded.
- **Q-E11 — does PlayReady reach the Win32 WebView2 host, does it cover any
  streaming service members use, and at which security level?** Unchanged from
  ADR-0001 risk 8. One addition: any content-protection path also provisions
  against a per-device identifier, which FR-036a and Principle VI make a
  **founder decision** rather than a spike output — as ADR-0001 risk 8 already
  records. *Settled by*: risk 8's measurement on the tier-1 runner. FR-041
  forbids asserting either a capability or an exclusion until it is taken.
- **Q-E11b — does a third-party WKWebView host reach FairPlay through EME at
  all, and which services members use depend on it?** Unchanged; the spike must
  establish the demand as well as the capability. *Settled by*: risk 8's
  measurement on the tier-2 runner.
- **Q-E12 — tier-2 blocking parity at the macOS 13 floor.** The *route* is now
  established (§3.2) and needs no fork, no new binding crate and no macOS-14
  proxy. What remains open is **parity at the 150,000-emitted-rule ceiling on
  real German sites, and the cost of the whole tier-2 binding route**. Scope the
  spike to the route rather than to blocking alone, since it also carries
  find-in-page and `pageZoom` (§2.3), FR-008's per-site-control mechanism (§3.5,
  N4), and app-surface network confinement (§6.4, G15). *Settled by*: on the
  tier-2 pinned runner, compile EasyList, EasyPrivacy and EasyList Germany
  through `adblock`'s content-blocking conversion, partition exception-closed
  under the ceiling, load through `WKContentRuleListStore` and
  `-addContentRuleList:`, and compare blocked-request sets against the tier-1
  native-engine run over the same site list. Outputs: parity delta per site,
  compile wall time at first run, store size on disk, RSS delta at ten tabs, and
  which per-site-control candidate is viable.
- **The cold-start spike (SC-002, no identifier).** Unchanged, with a method
  (§9.1) and an endpoint definition it needs first (G13). *Settled by*: two
  builds differing only at the seam, on each pinned runner, rebooted per §9.4.
- **The macOS-memory-at-ten-tabs spike (SC-004 tier 2; ADR-0001 risk 9).** Now
  has a concrete mechanism to test: does a **shared `WKWebViewConfiguration`**
  share a web content process on macOS 13, or only a data store and scheme
  handlers? Only the latter is established (§1.4). *Settled by*: process count
  and RSS at ten tabs on the tier-2 runner, shared configuration versus per-view
  configuration. This is the evidence SC-004's provisional tier-2 entry waits
  on.
- **S4 (ADR-0001; not one of the four specification spikes).** The windowing
  crate and what renders the chrome. §5.2 gives the criteria and §5.3 the
  accessibility exposure. *Settled by*: a measured candidate comparison on the
  tier-1 runner against SC-006 and SC-004, plus the screen-reader, dead-key and
  200% passes.

### 12.2 New measurements this research opens

Each is **new**: none is in the specification's Spikes section today, so they
belong in the plan and its tasks, and where an outcome would change a
specification statement that lands as an amendment there.

- **N1 — does the engine's own idle floor fit inside SC-005's 5 ms wake-free
  1-second sample and 18 s 60-minute window on each tier's reference machine?**
  The highest-consequence open item in this document, because SC-005 is ratified
  and tighten-only. *Settled by*: §9.6's 60-minute bare-window measurement per
  tier, run **before** SC-005 is treated as achievable. A failure is a
  specification amendment, not a bug.
- **N2 — does the tier-1 `WebResourceRequested` route see every request class
  the SDK enumerates?** Open reports assert it is not raised for service-worker
  fetches, link prefetch, WebSockets or virtual-host mappings, while the current
  SDK enumerates a WEBSOCKET resource context and SERVICE_WORKER/SHARED_WORKER
  source kinds — the reports may simply be older than the API. *Settled by*: a
  coverage fixture on the tier-1 runner, one page emitting a request in each
  enumerated context and source kind with
  `AddWebResourceRequestedFilterWithRequestSourceKinds(*, ALL, ALL)` registered,
  asserting one event per request and cross-checked against the SC-014 packet
  capture. Any class the handler does not see is a hole in tier-1 blocking and
  must be recorded as such rather than assumed covered.
- **N3 — is there any combination of platform signals that distinguishes an
  intercepting network from a successful load, without an outbound probe?** On
  Windows: (IsSuccess, WebErrorStatus, HttpStatusCode, final URI). On macOS:
  (which delegate callback fired, NSError domain and code, final URL, whether
  `didReceiveServerRedirectForProvisionalNavigation` fired). *Settled by*:
  driving each tier's backend through a real captive portal on that tier's
  reference runner and recording the full signal tuple. If nothing distinguishes
  it, the question becomes a **founder decision** under Principle VI and FR-007a
  — whether an outbound probe request is permissible — and, separately, whether
  `Intercepted` remains in the closed enum or FR-015 is amended. It is not a
  question a backend implementation may settle by guessing.
- **N4 — on tier 2, does `removeAllContentRuleLists`/`addContentRuleList:` take
  effect on the page already loaded, or only from the next navigation; and does
  wry's `with_navigation_handler` fire early enough on macOS to swap rule lists
  before a new top-level document's subresources begin loading?** If the former
  is "next navigation only", implementing FR-008's per-site control may require
  a reload the member did not ask for. *Settled by*: a direct test on the tier-2
  runner, instrumented for ordering. It decides between per-site-control
  candidates (A) and (B) and folds into Q-E12.
- **N5 — what actually throttles a hidden background tab on each shipping
  tier**, given that wry's `with_background_throttling` is documented
  Unsupported on Windows and Supported only since macOS 14.0, so neither tier
  gets it at its floor while FR-002 requires background tabs suspended and
  SC-005 sets the idle figure? *Settled by*: ten tabs on each tier's runner with
  nine hidden via `set_visible(false)`, CPU sampled against SC-005's window; and
  separately whether `WebViewExtWindows::set_memory_usage_level(Low)` moves
  SC-004's number on tier 1. If neither lever suffices, FR-002's suspension has
  no mechanism on that tier and that is a finding for the plan, not a bug to be
  found later.
- **N6 — does an accessibility tree published by an AccessKit host compose
  coherently with an embedded WebView2 or WKWebView tree** — one reading order,
  one focus order, no orphaned subtree — under Narrator, NVDA and VoiceOver?
  *Settled by*: a spike building a minimal AccessKit chrome with one embedded
  webview on each tier's runner, driven by each platform's own assistive
  technology, with the resulting tree captured from Accessibility Insights (UIA)
  and Accessibility Inspector (NSAccessibility) and committed. AccessKit's
  merged multiple-tree support explicitly does not cover native webview trees,
  so nothing published answers this. It gates the drawn-chrome candidate in S4.
- **N7 — SC-006 harness bring-up, in three parts.** Does PresentMon observe a
  WebView2-hosted window's frames — presented through DirectComposition by the
  browser process — as the host application's, or does it attribute them
  elsewhere or miss them? How large is the tier-1 cold-start difference between
  a rebooted machine and one with a warm file cache and prefetch database, and
  does it exceed the entry's tolerance? Is the machine-to-machine spread across
  each reference class within each entry's declared tolerance, as SC-013
  assumes? *Settled by*: a bring-up capture on the tier-1 runner against a
  minimal wry window with a known animation, comparing PresentMon's attribution
  against the frames the shell knows it submitted; paired cold-start trials,
  rebooted versus not, on one commit; and the same commit measured on two or
  three further machines of each reference class.
- **N8 — does a custom-scheme app surface get a distinct per-app origin, a
  secure context, and storage and cookie partitioning** against other apps and
  against the shell's chrome, on Windows 11 and on macOS 13? *Settled by*:
  measuring `location.origin`, `window.isSecureContext`, and cross-app
  `localStorage` and `document.cookie` visibility in a surface served from
  `evapp://<app-id>/` on macOS and `https://evapp.<app-id>/` on Windows, on each
  tier's pinned runner. The wry URL formats predict distinct origins; nothing
  measured confirms that partitioning follows.
- **N9 — what does a WKWebView on macOS 13 actually do when a page calls
  `navigator.geolocation.getCurrentPosition` or
  `Notification.requestPermission()` in a third-party host** — silent denial,
  silent grant, or an unhandled request that hangs? *Settled by*: running both
  on the tier-2 reference machine at the floor and committing the result, as
  FR-015a already requires for site-credential autofill. The header evidence
  establishes that no delegate exists to prompt; it does not establish what the
  page sees, and FR-041 forbids stating either presence or absence until it is
  measured.
- **N10 — cost questions that are budget measurements rather than preferences.**
  What does `fluent-bundle` plus its plural and formatting dependencies cost in
  download and installed bytes against SC-001, and is that justified over a
  plain keyed table for three languages? What does the SQLite dependency FR-012
  implies cost, and do Chrome, Firefox and Edge profile stores read reliably
  while those browsers are running (copy-then-read versus direct read, WAL
  present)? What are the byte and millisecond costs of `evreos-engine-webview`
  plus the chosen windowing crate on the tier-1 runner — the first change in the
  repository to add a non-trivial third-party tree? *Settled by*: building with
  each option and reading the SC-001 gate, as FR-043 requires the pull request
  to state anyway. For the backend, an A/B under the existing gate — two
  commits, backend absent then present, each measured by
  `scripts/check-budgets.py` — and the measuring commit must also write the
  baseline (§9.11(j)).
- **N11 — tier-2 crash capture and symbol-table sizing.** On tier 2, what is the
  crash-capture route, is a web-content-process-termination callback available
  and usable as a reason code, and does symbolisation from the release's DWARF
  or dSYM fit the tier-2 size budget? Separately, does the fixed padded
  ciphertext size — and the bounded frame count it forces — still leave crash
  stacks deep enough to diagnose a crash, and what happens when the causing
  frame lies below the truncation point? *Settled by*: a tier-2 spike on the
  pinned runner once procurement completes, with the result committed to this
  repository in the manner FR-015a requires for the autofill test; plus
  symbolising a corpus of deliberately induced shell crashes at several
  truncation depths, recording the depth at which the causing frame is retained,
  and stating the resulting fixed payload size against `budgets.toml` under
  FR-043.
- **N12 — a cluster of small platform unknowns, each of which blocks a published
  figure or a claimed capability.** What system-wide quantity implements
  SC-004's whole-machine cross-check on macOS, and is it commensurable with
  summed `phys_footprint` such that a declared margin means anything? Is
  `mmap(MAP_SHARED)` memory dropped from `phys_footprint` the way a
  page-file-backed section is dropped from `PrivateUsage`, so that SC-004's
  shared-section term is needed identically on both tiers? Does macOS offer any
  analogue to a Job Object's accounting — CPU time for processes that started
  and exited between two samples — that needs neither an entitlement nor SIP
  disabled? Which WebView2 API returns the runtime version string the run record
  must carry, and what is the equivalent identifier on tier 2? Can a protocol
  handler for the claim deep link be registered at install time on each tier, so
  FR-032's "directly after installation" holds on the first scan? Do the two
  engines' built-in PDF viewers meet a single stated behaviour set — open,
  scroll, zoom, text search, print, save, keyboard operation, screen-reader
  reading order — closely enough for FR-009's "consistently across supported
  platforms"? *Settled by*: targeted checks on each runner — allocate and touch
  a known N MB in a child process and confirm that both the summed per-process
  footprint and the candidate machine-wide delta move by approximately N,
  repeated for a shared mapping; a review of libproc and Endpoint Security
  options against the lifecycle requirement, publishing the residual work the
  harness can still miss if none qualifies; reading the WebView2 API reference;
  a packaging test (install, scan, confirm the claim flow opens without a prior
  launch); and an FR-009 conformance checklist written first, then run against
  both viewers with the same fixture documents including a screen-reader pass.

Two items are **not** new spikes and must not be planned as though they were.
The **site-credential autofill test** FR-015a requires on each tier is already a
specification-mandated, committed test and a **release blocker** — a tier must
not be released until its result is committed. And the **tier-1 runtime egress
inventory** — what WebView2 transmits on its own beyond SmartScreen, including
whether its non-optional "required" diagnostic data carries visited addresses —
is not a separate spike but an early, mandatory run of SC-014's own capture on
the tier-1 runner (§4.1, §2.6). Schedule it early rather than at acceptance: a
bad answer there is architectural news about tier 1, and FR-007a makes such a
transmission Evreos's own with no available remedy.

### 12.3 Founder decisions (not measurements)

- **Which reading of SC-014's "every URL-bearing payload" governs** — the
  literal one, which a conforming build fails on its own update check, or the
  history-bearing one that matches FR-007a's scope. It lands as a specification
  amendment that either restates SC-014's criterion in terms of history-bearing
  payloads or adds the committed closed list of permitted non-history
  destinations the capture's classifier reads (G16). It changes what the
  criterion means, so it is not an implementer's call.
- **The definition of SC-002's "an interactive window appears"** (G13).
- **Which ten pages compose the SC-004 gating corpus** (§9.8), against the rule
  proposed there, with ADR-0001 risk 2's German site matrix carried separately
  as the live, non-blocking observatory.
- **Whether the merchant catalogue is a delivered signed surface or
  shell-native** (§7.1), costed under FR-043.
- **Whether FR-039c's frame-contents rule is a ceiling or a floor** — whether
  "MUST carry only the module name, the symbol name, and the source file and
  line" requires line tables to ship, which is a measurable download-size cost
  against a 20 MB budget. Take the reading with the byte cost in hand (N11).
- **How FR-036a is enforced against first-party app surfaces**, given that an
  app surface is ordinary web content and receives `screen`, font metrics and
  `performance.now()` from the platform with no capability declaration involved,
  so the manifest gate FR-017 and FR-018 provide cannot see the breach. Either
  the capability catalogue gains explicitly-refused categories and app surfaces
  are reviewed at signing under FR-019a, or the SC-014 capture is extended to a
  signed-in session with each first-party app opened and app egress classified
  the same way. The second is testable; the first is not.
- **Where the root signing key lives, who holds it, and the recorded procedure
  for signing a delegation** (G2) — not derivable from the specification, and
  the two-level key design is unimplementable without it. Record it as an ADR.
- **Whether an outbound connectivity probe is permissible** if N3 finds that
  nothing distinguishes an intercepting network — a Principle VI and FR-007a
  question.
- **Whether to adopt EN 301 549 with the WCAG2ICT mapping** (G10), and the 24×24
  target-size project rule (G11).

### 12.4 Dependencies outside this repository, and legal questions

- **The Apivo API contract**, on four points that decide whether the wallet is
  buildable as specified. Does it report, per state, a **total the service
  itself computed**, and a payable amount, rather than only individual entries —
  if it returns entries only, FR-026's ban on client aggregation makes the
  wallet unbuildable as specified, and either the API gains total fields or
  FR-026 is amended? Is the FR-027 pending-reason set a **closed enumeration
  with stable codes**, with text keyed by BCP-47 primary language subtag and
  place as a separate parameter — a closed code set lets the explanation ship in
  the shell's FR-035 catalogues and stay available in the offline and stale
  states, while a free-form server string does not? Can a member's wallet hold
  **more than one currency** — if so, no single total exists that the client
  could show even were it permitted to compute one, which makes per-currency
  service-supplied totals mandatory rather than merely preferable? And does the
  service **issue a withdrawal token** before submission, so exactly-once stays
  wholly server-side and the client never needs an idempotency key of its own
  (§7.3)? *Settled by*: reading the wallet endpoint's contract or capturing a
  real response, or a founder decision to add the endpoint. If a token is
  unavailable, the plan must record which reading of FR-026a it takes and why.
- **Q-E11a** — whether the existing service is confirmed to hold campaign
  records and accept a redemption. Until it is, FR-029 ships disabled by build
  constant and **SC-010 must not be scheduled as an acceptance gate**.
- **Q-E14** is accepted rather than assumed: the client-type field and the
  EU-hosted retention computation are changes to a service outside this
  repository, and SC-011's signed-in figure rests on them.
- **A named OHTTP relay operator**, incorporated in a stated jurisdiction,
  running EU-only ingress, contracted to the three obligations FR-039b
  enumerates — no retention of source addresses, no transport metadata, no
  inbound-to-outbound correlation log, and forwarding the acknowledgement on the
  same connection without retaining it — with a signed contract and effective
  date that FR-039b also requires to appear in the pre-consent disclosure.
  **This is release-gating procurement, not an engineering task** (§8.8).
- **Counsel or a data-protection officer, carried with the DPIA**, on three
  questions this document deliberately does not answer: whether a
  US-incorporated relay operator running EU-only ingress satisfies FR-039b's
  naming requirement and FR-039f's hosting requirement; what the lawful basis is
  for transmitting the withdrawal report after the member has turned diagnostics
  off, since disclosure is not a basis; and whether the European Accessibility
  Act covers a desktop web browser distributed free of charge in Germany and
  Greece, or reaches Evreos only through the Apivo money surfaces.
- **Two structural limits on the signed-out retention figure that are not
  measurable in production**, because measuring them would need the identifier
  the design excludes: the acknowledgement-loss rate, which biases the figure
  downward (§8.3), and reinstallation, which inflates the enrolment denominator
  since a reinstall is a new install and enrols again. Both can be bounded only
  by a pre-release lab trial with a known client population on a controlled
  network, and both must be published beside the figure alongside the
  "unverified" and "self-selected" labels FR-039a and SC-011 already require.
- **Does the harness CA needed to read SC-014 payloads change what the capture
  observes** — HSTS, certificate pinning, or the runtime declining the
  interception — such that some traffic escapes the capture unrecorded? *Settled
  by*: running the capture with and without termination and reconciling the
  connection counts; any connection present in the unterminated run and absent
  in the terminated one is traffic the analysis cannot see and must be named as
  such in the published analysis SC-013 covers.

---

## 13. What this research did not decide, and what follows for the plan

**Deliberately not decided.** The windowing crate and the chrome renderer (S4).
Whether the tier-2 floor stays at macOS 13 or moves to 14 (Q-E12, and with it
the `WKWebsiteDataStore::dataStoreForIdentifier` question that rides along — at
a 13 floor only the default and non-persistent stores exist, so whether v1 needs
more than one persistent profile on tier 2 is a scoping answer to take before
any measurement). Any content-protection capability (Q-E11, Q-E11b). Whether
affiliate attribution survives tracking prevention (Q-E10). And every figure the
spikes exist to measure: none is predicted anywhere above.

**Ordering, and it inverts the obvious order: fix the seam before writing the
backend.** §§1.1–1.6 say the merged `Engine` trait cannot be implemented over
either shipping backend without a nested message loop SC-006 forbids, cannot
represent navigation the shell did not initiate, cannot express an in-flight
load SC-009 requires to be testable, has no seam where the shared platform
context lives that SC-004 depends on, and is single-surface where FR-001,
FR-002, FR-007 and FR-016 each need many. ADR-0001's own revisit-trigger list
records that the trait's swap intention "is untested until a second real backend
exists beside the headless one"; this research is that test arriving early, and
its answer is that the seam needs those changes before the backend is written.
Concretely: **(T1)** reshape the trait — asynchronous start returning a
navigation id, the closed event enum carrying `LoadError` unchanged inside
`Failed`, the host/factory type owning the shared context, the addressable
surface handle, the blocking policy surface, and the four §1.7 additions —
moving the headless implementation in the same commits, as FR-044 requires.
**(T2)** Add the conformance battery as a feature-gated module in
`evreos-engine` and make the headless engine pass it — still no unsafe anywhere.
**(T3)** Land `evreos-engine-webview` Windows-only with the lint carve-out, and
pass the battery plus SC-009's four causes on the tier-1 runner. **(T4)** The
macOS delegate replacement, then the same battery on the tier-2 runner. **(T5)**
Q-E12's whole tier-2 route rides in the crate T4 created.

**Three things run in parallel from Phase 1, because they are longer-lead than
any code.** Runner procurement (§9.12), which gates SC-002's spike, SC-004's
soak, SC-005's window, SC-006's trials, S4's decision and every tier-specific
measurement above. The OHTTP relay contract and the DPIA (§8.8) — no operator,
no signal, and the milestone that ships diagnostics must be able to ship with
the feature dark and unofferable. And two cheap bring-up measurements taken
before harness architecture is fixed, because a bad result from either is a
specification amendment rather than a code change: N1's engine idle floor
against SC-005's ratified bound, and the cold-start engine-initialisation floor
SC-002's four provisional entries already wait on.

**Three further things are cheap now and expensive later.** The budget-file
schema and its confirmed defect (§9.11): the budget-file gate blocks from M0 and
is specifically what bounds the advisory period on the measuring gates, so an
incomplete file is a gate that cannot fail. The unsafe-boundary decision (§2.9),
which must land with the first FFI line rather than after, because retrofitting
it means arguing about an `allow` that is already in the tree. And the SC-013
publication format (§9.12), which is not a final step but the shape every
earlier figure is written into.

**Two workstreams can land early with no platform risk.** The app verification
core — preimage format document, verifier, registry, catalogue, capability
intersection, version floor, cache, and the FR-019b artefact gate — is
exercisable entirely against the headless engine and can land before any
system-webview backend exists: least platform risk, most review surface. The
blocking corpus and conversion work is likewise platform-free and produces the
failure-taxonomy gate and the rule-count gate on ordinary CI, which is what
makes Q-E12's site parity a short measurement rather than an open-ended one.

**Risk, ranked by how much it can cost.** Highest: skipping T1 because the seam
already exists. Second: SC-005 is ratified and tighten-only, and its scope
includes engine processes whose idle floor has never been measured (N1) — the
same argument that kept SC-002 deliberately provisional, and if the floor
exceeds the bound the remedy is a specification amendment. Third: SC-001's
ratified 20 MB meeting a real dependency tree for the first time at T3, on a
gate that blocks from M0 and is not hardware-dependent, with baselines currently
at zero so the regression half is inert until that commit writes them. Fourth:
the tier-2 delegate replacement, the only place either shipping tier needs more
than an additive wrapper — bounded at seven named behaviours, all of which the
shell wants to own anyway, but all of them unsafe objc2 code with no second
implementation to check it against until T2's battery exists, which is the
argument for T2 preceding T4. Fifth: chrome accessibility is genuinely unsolved
for the drawn-chrome candidate (N6), against a release-blocking principle, and
belongs in the plan's risk register rather than folded into "build the UI".
Sixth: the partitioner's correctness hazard (§3.4), whose failure lands on
exactly the bank and government sites the specification names as abandonment
triggers — a property to prove by test on the emitted JSON, not to hope for.
Seventh, and outside this repository's control: FR-006 on tier 2 has no
host-side prompt mechanism for two of its four permissions at any floor this
product could declare (§2.4), and FR-002's suspension has no verified mechanism
on either tier at its floor (N5).

**Two claims risks, distinct from build risks.** FR-036a is a prohibition on
what Evreos does, not a defence for the member against sites, and the FR-041
distribution page must not read it the other way (§4.3). And ADR-0001's sketch
of the wallet using initialization scripts in merchant pages is unavailable
under FR-018a and FR-018b, so offer detection must work from the address alone
against a locally held merchant list; if that proves insufficient for the
pilot's merchants there is no compliant fallback, and the offer feature is
re-scoped rather than injected (§6.5).