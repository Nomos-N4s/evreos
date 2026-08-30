# ADR-0001: Host web content in operating-system webviews

- **Status**: Accepted
- **Date**: 2026-08-29
- **Deciders**: Founder

## Context

Evreos is a featherweight, privacy-first browser and super-app shell for desktop,
built by a solo founder. Its identity is a set of hard budgets — download size,
installed size, cold start, shell memory overhead, idle CPU, chrome input latency
— and a trust posture that lets it carry money surfaces. Its first cohort is
German, over 40, non-technical, arriving through a partner shop rather than
through technical enthusiasm.

The rendering engine is the decision every other decision hangs from. It sets the
floor for size and memory, decides what accessibility and international text input
cost, and determines who ships a security fix when the engine has a hole.

## Decision

**Host web content in operating-system webviews through the `wry` crate** —
WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux. The shell talks to
the engine only through an `Engine` trait, beside a headless test implementation
kept working from day one, so the seam is proved by a second implementation rather
than asserted.

The windowing crate is a free variable, decided as an output of spike S4. `wry`
no longer depends on `tao` — `tao` is a dev-dependency only — so "wry/tao" is not
a package deal.

### Platform tiering

This departs from the equal treatment of three platforms in the specification
input, and the departure is deliberate:

| Platform | Tier | Meaning |
| --- | --- | --- |
| Windows | 1 | Release criteria apply. WebView2 is evergreen Chromium, security-patched by Microsoft. |
| macOS | 2 | Ships at a minimum OS version, **not yet declared** — deferred to clarify. WKWebView is frozen to the user's OS, so this number sets the engine floor and every capability statement that depends on it. |
| Linux | Separate decision | Removed from "cross-platform". Its own budget row, its own go/no-go. |

The tiering rests on a market assumption that is **not yet verified**: that the
first cohort's platform mix is heavily Windows, and that a non-technical cohort
arriving through a retail counter skews further that way. If true, the platform
where this architecture is strongest is also most of the market. That figure has
not been measured for this cohort and appears in the risks below; measure it
before treating the tiering as settled.

## Rationale, in evidence order

1. **Windowed hosting hands input, IME and the accessibility tree to the OS
   engine — verified on Windows.** `wry` uses `CreateCoreWebView2Controller`,
   the windowed hosting mode Microsoft documents as having the platform handle
   input, IME and accessibility. For *page content* on that platform, keyboard
   operation, scaling, German dead keys and Greek input therefore come from the
   engine rather than from one person implementing them.

   Three limits on this argument, stated because the constitution makes
   accessibility a release blocker and an overstated claim here is expensive.
   It is **evidenced on Windows only**; the equivalent guarantees for WKWebView
   and WebKitGTK are assumed, not verified, and are a risk below. It covers
   **page content, not the shell's own chrome** — the tab strip, address field
   and app surfaces carry their own accessibility obligation, and what renders
   them is an output of spike S4, not settled here. And WCAG 2.1 AA is a
   property of the whole product, so no engine choice delivers it by itself.
   This remains the strongest argument for the decision; it is not a guarantee
   of compliance.
2. **It is the only route to the size budgets.** Bundled Chromium adds 80–120 MB
   before any product code; Microsoft's own documentation puts the fixed-version
   WebView2 runtime at "over 250 MB". An empty Electron application boots at
   150–200 MB RSS, which is the entire shell-overhead budget at zero tabs. Engine
   choice is downstream of the size budget, not an independent decision.
3. **It is the only maintained Rust native-webview abstraction covering all
   three platforms.** Non-Rust abstractions over the same three backends exist
   and are maintained — `webview/webview`, Photino and pywebview among them — so
   the claim is about the Rust ecosystem, not the field. Rust-adjacent
   alternatives fail on Linux specifically: Qt WebView falls back to bundled
   QtWebEngine, which is the thing permanently rejected below.

Rust follows from `wry` rather than the other way round. What rides on Rust is
access to `wry` and `adblock-rust`, and memory safety in the layer that parses
URLs, session files, IPC messages and signed manifests — not the safety of web
content, which belongs to the engine.

## Rejected options

- **Bundled Chromium, CEF or Electron.** The budgets die at the first byte, and it
  is the thing Evreos exists not to be. Permanent.
- **A Chromium fork (Brave, Vivaldi, Arc model).** Not merely undesirable —
  arithmetically unavailable under a 20 MB download budget. Independently, the
  maintenance load is not payable by one person: Thorium, the closest thing to a
  small-team fork, is three people, roughly eight hours per rebase, a 100 GB
  checkout, permanently a major version behind. Chrome moves to a two-week major
  release cadence with Chrome 153 on 8 September 2026, doubling that treadmill.
- **An own engine.** Servo began at Mozilla Research in 2012 and only started
  landing visible, interactive text selection in June 2026 — against a tracker
  issue raised in December 2014 and closed in August 2026, roughly eleven and a
  half years later, with initial support still carrying documented gaps. It
  embeds SpiderMonkey for JavaScript, so it is not independent throughout.
  Ladybird, the only credible from-scratch effort, is a US public charity funded
  by donations, whose co-founder and anchor donor resigned in July 2025. Neither
  is authorable by a solo founder. **Revisit trigger:** either becomes
  daily-drivable, at which point the `Engine` trait means adopting it is a backend
  swap rather than a rewrite. Servo's embedding API states it is not yet ready for
  general use.
- **Servo or Ladybird as the v1 default.** Same evidence. Tracked as a future
  experimental third backend, to **adopt rather than author**.

## Capability floor

What this architecture cannot deliver. Stated here so it is not rediscovered as a
surprise, and so no marketing claim outruns it.

- **Content protection is a scoping question, not a settled exclusion. The scope
  is narrower than first recorded, and the German public broadcasters are not in
  it at all.** No exclusion is described to anyone until risk 8 is retired. The
  hand-off is built regardless, because it is cheap insurance.

  **Verified — ARD and ZDF Mediathek are not DRM-protected.** Streamlink's ARD
  plugin plays them with plain HLS and progressive HTTP and contains no DRM
  handling; yt-dlp's ARD and ZDF extractors contain none either, and yt-dlp
  additionally detects DRM generically from HLS, DASH and Smooth manifests
  (`HlsFD._has_drm`, DASH `ContentProtection`, ISM `Protection`), so a protected
  adaptive stream would surface even if the extractor said nothing about it —
  which covers the adaptive path these extractors use, though not formats served
  as direct URLs; and the ARD add-on in Kodi's official repository
  (`plugin.video.ardmediathek_de`) declares no `inputstream.adaptive` dependency
  at all, so it neither decrypts nor even demuxes adaptive streams. That last
  point is indirect — `inputstream.adaptive` is a DASH/HLS/Smooth demuxer whose
  DRM support is an added capability rather than its purpose — so it corroborates
  the streamlink and yt-dlp evidence rather than standing alone. ARD's
  restrictions are geo-blocking and FSK
  broadcast-time windows, not encryption. An earlier version of this record
  named these two as the paradigm DRM-dependent services; that was wrong, and
  it matters, because they are heavily used by this cohort.

  **Verified — Joyn uses DRM, and is multi-DRM rather than Widevine-only.** The
  unofficial Kodi add-on `Maven85/plugin.video.joyn` requests playback with the
  literals `platform='browser'` and `protectionSystem='widevine'`, but those are
  the add-on's own hardcoded parameters, not Joyn's only option: the add-on
  branches on a `drm` value Joyn returns and sets a license type of either
  `com.widevine.alpha` or `com.microsoft.playready`, and ships a "Force PlayReady
  DRM" setting that sends a Windows Edge user agent. So Joyn is protected, and
  which system it serves depends on the client. RTL+ is **not characterised
  here**: an earlier draft said it shows FairPlay asset names on Apple platforms,
  citing no source, so the claim cannot be re-verified and is withdrawn.

  **Verified — PlayReady is reachable in WebView2 through EME, at the software
  security level.** A 2024 WebView2 bug report names `playready.hardware` and
  `playready.recommendation` (SL3000) as producing a black screen while
  `playready.software` plays correctly (WebView2Feedback#4935, closed as
  completed). An open Widevine feature request opens with "Webview2 support
  playready already" (WebView2Feedback#4828). Microsoft's own WebView2
  documentation says nothing about content protection at all.

  **Verified — Widevine is not an officially supported WebView2 key system.**
  The feature request above is open and unanswered, and a separate report
  describes Widevine authentication failing inconsistently where it loads
  (WebView2Feedback#2021).

  **Widevine itself** is not in open-source Chromium; it is licensed per vendor,
  and Google has refused at least one independent open-source browser outright.
  Brave and Vivaldi carry it under commercial agreements — and both are Chromium
  forks, so the wall is commercial rather than architectural. Forking Chromium
  does not clear it by itself, and hosting an OS webview does not make it worse.
  Whether a solo-founder project could obtain such an agreement is untested.

  **Still unestablished:** whether PlayReady at the software security level is
  sufficient for the services members actually use, since commercial streamers
  commonly require a hardware tier for higher resolutions; whether a third-party
  `WKWebView` host gets FairPlay through EME at all on tier 2; and what EME
  WebKitGTK exposes on the deferred platform. Netflix's stated requirements were
  unreachable during investigation. Provide the hand-off regardless — it is cheap
  insurance — and see risk 8.

  The negative claims rest on convergent client evidence: independent players
  that succeed with no DRM capability at all. The positive ones rest on a
  third-party add-on's request parameters and on vendor issue trackers, which
  show what a service offers a particular client rather than what it requires of
  every client. The negatives are strong; the positives are leads for the spike.
- **No single cross-platform content-blocking primitive.** `wry` exposes no
  `WKContentRuleList` or `WebKitUserContentFilterStore` binding; what exists is
  top-level navigation gating, a macOS-14+ proxy config behind a feature flag,
  Chrome extensions on Windows and web-process extensions on Linux. Blocking is
  two or three pipelines behind the `Engine` trait. WebKit's compiled rule lists
  cap at 150,000 rules each, so multi-list splitting is required from day one.
- **Hosting third-party Chrome extensions is not a promise Evreos can make.**
  WebView2 has genuine Chrome-extension hosting, but UI-less and sideload-only;
  macOS gained `WKWebExtension` only in 15.4, excluding pre-2018 Macs; Linux has
  nothing and will not. Note the trap: `wry`'s Linux `with_extensions_path` shares
  a name with the Windows method but loads a shared object, not a `manifest.json`.
  **The cashback wallet is therefore built once, natively in the shell**, using
  navigation gating, initialization scripts and the cross-platform cookie API.
- **Passkeys are uncertain.** Entitlement-gated on macOS; absent from WebKitGTK
  release notes; undocumented for WebView2. German banking is moving onto
  WebAuthn.
- **No engine security fix of our own on macOS or Linux.** The engine is the OS's.
  Patch latency belongs to Apple or a distribution, and is to be published rather
  than hidden. Never market those builds on engine security updates.
- **Per-tab process isolation cannot be truthfully claimed** on macOS or Linux.
- **On Windows, WebView2 is Chromium.** This architecture escapes the maintenance
  bill, not the Chromium monoculture. Do not market it as the latter.

## Accepted costs

- **Navigation failure is not surfaced, and the remedy differs by platform.**
  `PageLoadEvent` is `{ Started, Finished }` with no failure variant, and there
  is no certificate or authentication-challenge handling in `wry`'s desktop
  paths. The consequence is that the shell cannot distinguish a failed load from
  a successful one — which is browser table stakes missing, and must be fixed.
  The symptom differs: on Windows the load terminates but reports success,
  because `Finished` is raised from `NavigationCompleted` with the success and
  error-status arguments discarded; on Linux the finished event always follows a
  failure; only on macOS does the load never terminate at all.

  The remedy also differs, and a fork is **not** required everywhere. IPC is
  attached to the user-content controller and custom protocols are URL scheme
  handlers on the configuration, so neither depends on the navigation delegate.
  On Windows and Linux there is no replaceable delegate: event registrations are
  additive and `wry` exposes the raw native handles, so the shell can add its own
  navigation-completed, server-certificate-error and basic-authentication
  handlers, or the GTK load-failed, load-failed-with-tls-errors and authenticate
  signals, without forking. On macOS replacing the navigation delegate is
  necessary and costs `wry`'s page-load, download, navigation-gating and
  deferred init-script callbacks. Budget a wrapper over exposed handles on
  Windows and Linux, a fork or upstream contribution on macOS, and upstream the
  load-failure and TLS hooks regardless.
- **Three engines is three QA surfaces** for the whole web, not for our own pages.
  A support ticket may not reproduce across platforms.
- **Environment sharing must be an explicit requirement of the `Engine`
  trait, on Windows.** `wry` creates a fresh `CoreWebView2Environment` per
  webview unless one is passed in, and does not cache, so sharing must be
  designed in rather than assumed. The macOS equivalent originally recorded here
  — sharing a process pool — is withdrawn: that interface has been a documented
  no-op for several OS versions and has no binding in `wry`. What actually
  governs macOS memory at ten tabs is unestablished and belongs to the spikes.
- **Benchmark honesty.** Page rendering belongs to the OS engine, so the public
  benchmark measures only what is ours: download and installed size, cold start,
  shell overhead, idle behaviour, chrome latency. The "≥40% below Chrome at 10
  tabs" target is withdrawn: on Windows the engine *is* Chromium, and the
  published ordering flips between USS and PSS on the same machine.

## Risks to retire, in priority order

1. **Affiliate attribution against WebKit's Intelligent Tracking Prevention.** ITP
   targets bounce-tracking redirect chains, which describes affiliate attribution.
   Unverified end to end, and it is a business risk wearing a browser-feature
   costume. Test a real redirect chain on all three engines before the wallet is
   designed around cookies.
2. **The German site matrix** — Sparkasse, Volksbank, ING, DKB, ELSTER, the
   electronic Personalausweis flow, DHL, the partner shop's own checkout, Google
   sign-in (blocked in embedded webviews per Microsoft's documentation), and
   public-broadcaster streaming. A cohort member whose bank login breaks does not
   file a bug; they uninstall and tell the shop.
3. **The navigation-failure spike.** Add a load-failure event and a TLS hook on
   each engine, by the route that engine requires: a wrapper over the exposed
   native handles on Windows and on Linux, where no fork is needed; a fork or
   upstream contribution on macOS, where the navigation delegate must be
   replaced. Only the macOS half carries fork cost, and it is the cheapest way
   to learn what maintaining that fork feels like. Upstream the load-failure and
   TLS hooks regardless of which route each platform takes.
4. **Linux go/no-go** — ten live tabs on Wayland, and an AppImage on the scale.
   Note `wry`'s `build_as_child` tab model is X11-only, and Wayland is the default
   on current Ubuntu, Fedora and Plasma.
5. **Blocking parity** — compile EasyList, EasyPrivacy and German regional lists
   against the 150,000-rule ceiling, and measure parity on real German sites as a
   CI gate.
6. **The cohort's platform mix.** The tiering above assumes it. Instrument the
   partner's landing page before committing to the tier order.
7. **Accessibility on the tier-2 and deferred platforms.** The rationale is
   evidenced on Windows only. Drive each shell surface with the platform's own
   assistive technology before claiming WCAG 2.1 AA anywhere else.
8. **Content protection on the tier-1 platform.** Establish what WebView2 can
   actually play: whether a Widevine module can be loaded, and whether PlayReady
   is reachable through the platform media stack. The public broadcasters are
   already known not to need either. Test against the services that do — Joyn,
   RTL+, and the commercial streamers — before the exclusion is treated as
   settled or described to anyone.
9. **What governs macOS memory at ten tabs.** The process-pool requirement
   originally recorded here was withdrawn as a no-op, which left this
   untracked. Establish what actually shares state between webviews on that
   platform, or the shell-overhead budget has no mechanism behind it there.
10. **Browser-extension behaviour on recent macOS in practice.** Cited in the
    capability floor above and listed as unverified below; nothing has been run
    to confirm it. It bounds nothing in v1, since hosting third-party extensions
    is a non-goal, but the claim should not stand unmeasured while being used.
11. **Passkey support on the tier-2 and deferred platforms.** The capability
    floor calls it uncertain and nothing tracked it. German banking is moving
    onto WebAuthn, so this bears directly on whether the cohort can sign in.

## Revisit triggers

The decision itself, not only its rejected alternatives, should be reopened if:

- Accessibility on a tier-2 or deferred platform cannot be brought to WCAG 2.1
  AA within the shell, since the constitution makes that a release blocker;
- the cohort's measured platform mix contradicts the tiering assumption above;
- the size budgets are formally relaxed, since the engine choice is downstream
  of them and a different answer becomes available if 20 MB is no longer binding;
- an independent engine becomes daily-drivable, in which case the `Engine` trait
  is intended to make adoption a backend swap — an intention that is untested
  until a second real backend exists beside the headless one.

## Evidence status

**Verified against primary sources** — source code, official documentation,
release notes and project statements: `wry`'s API surface and its per-platform
navigation behaviour; WebKit's compiled-rule-list ceiling; Microsoft's documented
runtime size and WebView2 feature list; Servo's and Ladybird's status and
funding; Thorium's maintenance load; the licensing position on the content
protection module discussed above.

**Unverified, and each appears in the risks to retire above** rather than as a
finding: tracking prevention versus affiliate attribution (risk 1); the cohort's
platform mix, which the tiering assumes (risk 6); accessibility guarantees on the
tier-2 and deferred platforms (risk 7); what WebView2 can actually play under
content protection (risk 8); what governs macOS memory at ten tabs (risk 9);
browser-extension behaviour on recent macOS in practice (risk 10); and passkey
support on the tier-2 and deferred platforms (risk 11), which the capability
floor names and which nothing had tracked.

**Size and memory figures** quoted in the rationale come from vendor
documentation and published project statements rather than from measurement on
this project's own targets. Treat them as sound enough to eliminate options by
an order of magnitude, and not as budgets; the budgets are measured in the
spikes.

## Corrections

This record was amended after an adversarial review confirmed defects in its
supporting claims. The decision is unchanged; the corrections concern accuracy.

A second amendment followed a review of the first. Corrected then: a risk that
still budgeted a fork on Windows, contradicting the accepted cost this same
document had just corrected, and which also omitted Linux while claiming to name
the route each platform requires; an unsourced claim about how a
content-protection system is packaged, stated as fact; a media exclusion that
named no system or service and so could not be acted on or re-verified; and
unverified items the evidence section claimed were in the risks list when they
were not — browser-extension behaviour on recent macOS, passkey support, and
what governs macOS memory at ten tabs, the last created by withdrawing the
process-pool requirement without putting anything in its place.

The media exclusion was then rewritten a third time, from investigation rather
than from synthesis, after review found the second version's examples false. The
substantive correction: ARD and ZDF Mediathek are **not** DRM-protected, and had
been named as the paradigm services that fail. What WebView2 can play under
content protection was left explicitly unestablished rather than guessed, which
is what the two previous versions did wrong.

Corrected in the first amendment: the assertion that a `wry` fork was required on
every platform, which was false on Windows and Linux; the description of navigation failure as an
endless loading indicator, which is accurate only on macOS; the claim to be the
only maintained cross-platform native-webview abstraction including Linux, which
several non-Rust projects refute; a process-pool sharing requirement naming an
interface that is a documented no-op; a media exclusion argued from one
content-protection system while another is native to the tier-1 platform; a
stale tracker-issue state; the accessibility rationale, which was evidenced on
one platform and asserted for three; and an unverified platform-share figure
that was stated as fact while the evidence section called it unverified.
