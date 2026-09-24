# Decision 0005: Intercepted navigation classification

- **Status** — Open
- **Date** — 2026-09-24
- **Deciders** — Founder
- **Cite as** — decisions/0005

## Question

FR-015 specifies four causes of navigation failure that an implementation must classify and report: unresolvable host, certificate failure, intercepted navigation (such as a captive portal), and general connection failure.

Platform error code ranges — specifically WebView2's `COREWEBVIEW2_WEB_ERROR_STATUS` (19 enumeration values) on Windows and Apple's `NSURLError` domain/codes on macOS — contain no value denoting interception. This is because when a captive portal or middlebox intercepts a navigation, the network request completes successfully from the platform webview's perspective: an HTTP server responds, returns content (e.g., a captive portal login page), and the navigation succeeds with no platform web error status.

To determine whether platform signals alone can distinguish an intercepted navigation without making an outbound request, measurement N3 is required.

If measurement N3 shows that platform signals do not distinguish interception, two questions are reserved for the founder and MUST NOT be decided silently by an implementer:

1. **Outbound Probe Permissibility**: Is an outbound probe permissible under Principle VI (No Telemetry / Privacy Invariants) and FR-007a's closed and exhaustive list of outbound network requests?
2. **Requirement or Enum Amendment**: If an outbound probe is impermissible or unacceptable, does `LoadError::Intercepted` stay in the `LoadError` enum (reported when shell-classified or headless-simulated) or is requirement FR-015 amended?

This record raises both questions and answers neither.

## Decision

This decision is **Open**. The question is recorded so that downstream tasks (such as T022, T083, and T099) may cite `decisions/0005` prior to founder resolution.

Measurement N3 will be conducted in a later phase on each tier's pinned runner to record the platform signal tuples when encountering a captive portal alongside control loads:

- **Windows (Tier 1)** signal tuple: `(IsSuccess, WebErrorStatus, HttpStatusCode, final URI)`
- **macOS (Tier 2)** signal tuple: `(which delegate callback fired, NSError domain and code, final URL, whether didReceiveServerRedirectForProvisionalNavigation fired)`

The measurement itself is taken in Phase 3 on that tier's pinned runner, not in this decision file.

Until measurement N3 is completed and the founder resolves any open questions:
- No backend may synthesise `LoadError::Intercepted` from a platform error status code.
- `LoadError::Intercepted` is exercised only via shell-supplied classification or the headless engine.

## Evidence

- `specs/001-evreos-v1/spec.md`:
  - FR-015: Navigation failure reporting and requirements for the four failure causes.
  - FR-007a: Closed and exhaustive list of permitted outbound network requests.
  - Principle VI: Privacy, telemetry prohibition, and strict boundary invariants.
  - SC-009: Error state handling.
- WebView2 API Specification: `COREWEBVIEW2_WEB_ERROR_STATUS` enumeration (19 values) contains no status code representing captive portal redirection or interception.
- Apple `Foundation` & `WebKit` Documentation: `NSURLError` domain and `WKNavigationDelegate` callbacks handle errors, but HTTP success / redirect responses from captive portals produce normal navigation completions rather than `NSURLError` codes.
- `specs/001-evreos-v1/tasks.md`: Tasks T021, T022, T083, and T099.

## Serves

- FR-015 (Navigation failure classification)
- SC-009 (Error states)
- Principle VI (Privacy and no unsolicited network telemetry)
- FR-007a (Exhaustive permitted network requests list)
- Tasks T021, T022, T083, T099

## Consequences

- Holds `decisions/0005` in the decision register as an open founder decision.
- Defines the required signal tuples for measurement N3 on Windows and macOS.
- Establishes the constraint that platform error status mapping MUST NOT map platform errors to `LoadError::Intercepted`.
- When measurement N3 completes in Phase 3, if platform signals fail to uniquely identify interception, the two open questions will be submitted to the founder for a recorded decision or specification amendment.

## Corrections

None yet.
