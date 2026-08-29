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
| macOS | 2 | Ships at a declared minimum OS version. WKWebView is frozen to the user's OS. |
| Linux | Separate decision | Removed from "cross-platform". Its own budget row, its own go/no-go. |

The alignment is fortunate rather than clever: Germany is Windows-dominant, and a
non-technical cohort arriving through a retail counter skews further that way. The
platform where this architecture is strongest is most of the market.

## Rationale, in evidence order

1. **Windowed hosting hands input, IME and the accessibility tree to the OS
   engine.** `wry` uses `CreateCoreWebView2Controller` — windowed hosting — where
   Microsoft documents that the platform handles input, IME and accessibility.
   WCAG 2.1 AA, full keyboard operation, 200% scaling, German dead keys and Greek
   input therefore arrive by construction rather than by one person implementing
   them. This is the strongest argument and it is not the one about bytes.
2. **It is the only route to the size budgets.** Bundled Chromium adds 80–120 MB
   before any product code; Microsoft's own documentation puts the fixed-version
   WebView2 runtime at "over 250 MB". An empty Electron application boots at
   150–200 MB RSS, which is the entire shell-overhead budget at zero tabs. Engine
   choice is downstream of the size budget, not an independent decision.
3. **It is the only maintained cross-platform native-webview abstraction that
   includes Linux.** Alternatives fail there specifically: Qt WebView falls back to
   bundled QtWebEngine, which is the thing permanently rejected below.

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
  landing visible, interactive text selection in June 2026, against a tracker
  issue open since December 2014; it still embeds SpiderMonkey for JavaScript.
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

- **No DRM on any engine.** Netflix, Disney+, Spotify web, ARD/ZDF DRM streams
  fail. Note this is **not** something a Chromium fork would have fixed: the
  Widevine CDM is not in open-source Chromium, Brave and Vivaldi ship it under
  agreements, and Google has refused open-source projects. DRM is a licensing wall
  in both architectures. Requires a documented hand-off to the system browser.
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

- **A `wry` fork is required, not optional.** `PageLoadEvent` is `{ Started,
  Finished }` with no failure variant, and there is no certificate or
  authentication-challenge handling in the desktop paths. Today a mistyped URL, an
  expired certificate, a captive portal or an HTTP-auth prompt each produce a
  blank page and a spinner that never stops. That is not a missing feature; it is
  the browser not existing. The fix cannot be applied from outside, because
  supplying a navigation delegate breaks `wry`'s IPC and custom protocols. Budget
  the fork and upstream the load-failure and TLS hooks.
- **Three engines is three QA surfaces** for the whole web, not for our own pages.
  A support ticket may not reproduce across platforms.
- **A shared `CoreWebView2Environment` and `WKProcessPool` must be an explicit
  requirement of the `Engine` trait.** `wry` creates a fresh environment per
  webview unless one is passed in, and does not cache.
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
3. **The `wry` fork spike** — add a load-failure event and a TLS hook on WebView2
   and WKWebView. The cheapest way to learn what maintaining that fork feels like.
4. **Linux go/no-go** — ten live tabs on Wayland, and an AppImage on the scale.
   Note `wry`'s `build_as_child` tab model is X11-only, and Wayland is the default
   on current Ubuntu, Fedora and Plasma.
5. **Blocking parity** — compile EasyList, EasyPrivacy and German regional lists
   against the 150,000-rule ceiling, and measure parity on real German sites as a
   CI gate.

## Evidence status

Claims about `wry`'s API surface, WebKit's rule ceiling, Microsoft's runtime size
and documented WebView2 behaviour, Servo's and Ladybird's status, and Thorium's
maintenance load were verified against primary sources — source code, official
documentation, release notes and project statements. Claims about ITP versus
affiliate attribution, German platform-share figures, and Chrome-extension
behaviour on macOS 15.4 in practice are **unverified** and appear above as risks
to retire rather than as findings.
