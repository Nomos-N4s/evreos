# Measurement N12: the default-browser route on macOS

- **Task**: T059 (`specs/001-evreos-v1/tasks.md`) / CAR-116
- **Date**: 2026-09-27
- **Status**: Unmeasured. No Mac was available for this change; the route is
  owed on the tier-2 pinned runner.

## The question

FR-013 requires that the member can make Evreos their default browser from
within it. Research §10.2 establishes the tier-1 route: register, then open the
system's default-apps page. On tier 2 it records only that "a documented API
call exists but is unverified here", and tags it N12. The N12 entry in research
§12.2 does not list it among the platform unknowns it names, so this record
states the question itself. T059 asks that the route be established on the
tier-2 pinned runner rather than assumed. Until it is, `route()` in
`crates/evreos-platform/src/default_browser.rs` reports
`Route::Unestablished` on macOS, and `open_settings` returns `Unsupported`
there.

## What is known, and from where

Everything below is from Apple's developer documentation. None of it was
verified for this record, and none of it is adopted by it.

- **Being listed.** An application declares the URL schemes it handles in its
  `Info.plist` (`CFBundleURLTypes`) and the document types it opens
  (`CFBundleDocumentTypes`). An application declaring `http` and `https` is a
  candidate for the system's default web browser setting.
- **Asking to be the default.** Two calls are documented:
  - `NSWorkspace.setDefaultApplication(at:toOpenURLsWithScheme:completionHandler:)`
    in AppKit, available from macOS 12;
  - `LSSetDefaultHandlerForURLScheme` in Launch Services, deprecated from
    macOS 12.

  The documentation does not settle whether either call changes the default
  outright or asks the member first, nor what either does on the macOS 13
  floor for an application that is signed and notarised but not sandboxed.

## What the pinned runner must settle

On the tier-2 pinned runner, at the macOS 13 floor and on the newest release
the tier supports, with a signed build whose `Info.plist` declares `http`,
`https` and the HTML document types:

1. **Listing.** Does the build appear in the system's default web browser
   setting from its `Info.plist` alone, and from what moment: installation,
   first launch, or a Launch Services registration it must make itself?
2. **The call.** What does `setDefaultApplication(at:toOpenURLsWithScheme:)`
   do for `http` and `https`: change the default, show a system confirmation
   the member answers, or fail? What does its completion handler report in
   each case, including when the member declines?
3. **Conditions.** Does the call need an entitlement, or fail outside the App
   Sandbox or before notarisation?
4. **Fallback.** If there is no call that works, which system settings page
   holds the default web browser setting on each release, and can it be opened
   directly?
5. **Binding and cost.** Through which safe binding the adopted call is
   reached. `objc2-app-kit` is already in the shell's macOS graph: version
   0.2.2, through `evreos-chrome`'s `accesskit_macos` and through `winit`.
   Version 0.3.2 is in the workspace's graph too, but only through `wry`
   0.53.5, which the chrome spike alone uses. Research §2.8 names it among
   `wry` 0.56.1's dependencies, for an engine crate that does not exist yet.
   What does the adopted binding add to a release build, stated against
   `budgets.toml` as the tier-1 bindings are at
   `docs/measurements/default-browser-registration.md`?

## What follows from the answer

The route settled here becomes `Route` for macOS in
`crates/evreos-platform/src/default_browser.rs`, with a test of its own on a
tier-2 runner, and this record is updated with the instrument tuple and the
observed behaviour. If question 2 shows that the call asks the member, that
dialog is an operating-system surface Evreos does not control, as the
default-apps page is on tier 1, and SC-007's hand-off design applies to it too.
