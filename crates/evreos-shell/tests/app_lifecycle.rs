//! Application lifecycle integration tests for evreos-shell.
//!
//! Under T038:
//! - Asserts that a window opens.
//! - Asserts that the loop dispatches an input event to the chrome on the thread
//!   the platform requires rather than on any other.
//! - Asserts that the loop dispatches a navigation event from the engine seam on
//!   the thread the platform requires rather than on any other.
//! - Asserts that closing the last window ends the process leaving no child view alive.

use std::panic;
use std::thread;

use evreos_engine::{Engine, NavigationEvent, Request};
use evreos_engine_headless::HeadlessEngine;
use evreos_shell::app::App;
use winit::event::WindowEvent;

#[test]
fn a_window_opens_and_allocates_child_view() {
    let engine = HeadlessEngine::new();
    let mut app = App::new(engine);

    assert_eq!(app.window_count(), 0);
    assert_eq!(app.child_views_alive(), 0);
    assert!(!app.has_open_windows());

    let win_id = app.open_window("Test Window");

    assert_eq!(app.window_count(), 1);
    assert_eq!(app.child_views_alive(), 1);
    assert!(app.has_open_windows());

    let window = app.get_window(win_id).expect("window must exist");
    assert_eq!(window.id(), win_id);
    assert_eq!(window.title(), "Test Window");
    assert!(!window.is_closed());
}

#[test]
fn loop_dispatches_input_event_to_chrome_on_platform_ui_thread() {
    let engine = HeadlessEngine::new();
    let mut app = App::new(engine);
    let ui_thread = app.ui_thread_id();
    assert_eq!(ui_thread, thread::current().id());

    let win_id = app.open_window("Input Test Window");

    // Dispatch input on the platform UI thread
    let event = WindowEvent::Focused(true);
    app.dispatch_input(win_id, &event);

    let window = app.get_window(win_id).expect("window must exist");
    let default_chrome = window
        .chrome()
        .as_any()
        .downcast_ref::<evreos_chrome::DefaultChrome>()
        .expect("must be DefaultChrome");
    assert_eq!(default_chrome.input_count(), 1);
    assert_eq!(default_chrome.last_input_thread(), Some(ui_thread));

    // Assert that attempting to dispatch input off the platform UI thread panics with thread violation
    let fake_other_thread_id = thread::spawn(|| thread::current().id()).join().unwrap();
    app.set_ui_thread_for_testing(fake_other_thread_id);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        app.dispatch_input(win_id, &WindowEvent::Focused(false));
    }));
    assert!(
        result.is_err(),
        "dispatching input off the platform UI thread must fail thread assertion"
    );
}

#[test]
fn loop_dispatches_navigation_event_from_engine_seam_on_platform_ui_thread() {
    let mut engine = HeadlessEngine::new().with_page("https://example.invalid/", "Example");
    let _ = engine.start_navigation(&Request::new("https://example.invalid/"));

    let mut app = App::new(engine);
    let ui_thread = app.ui_thread_id();
    assert_eq!(ui_thread, thread::current().id());

    let win_id = app.open_window("Navigation Window");

    // Dispatch navigation events from engine seam on UI thread
    let dispatched = app.dispatch_navigation_events();
    assert!(!dispatched.is_empty(), "engine events must be dispatched");

    let first = &dispatched[0];
    assert!(matches!(first, NavigationEvent::Started { .. }));

    let window = app.get_window(win_id).expect("window must exist");
    let default_chrome = window
        .chrome()
        .as_any()
        .downcast_ref::<evreos_chrome::DefaultChrome>()
        .expect("must be DefaultChrome");
    assert_eq!(default_chrome.navigation_count(), dispatched.len());
    assert_eq!(default_chrome.last_navigation_thread(), Some(ui_thread));

    // Assert that attempting to dispatch navigation events off the platform UI thread panics with thread violation
    let fake_other_thread_id = thread::spawn(|| thread::current().id()).join().unwrap();
    app.set_ui_thread_for_testing(fake_other_thread_id);

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        app.dispatch_navigation_events();
    }));
    assert!(
        result.is_err(),
        "dispatching navigation events off the platform UI thread must fail thread assertion"
    );
}

#[test]
fn closing_last_window_leaves_no_child_view_alive() {
    let engine = HeadlessEngine::new();
    let mut app = App::new(engine);

    let win1 = app.open_window("First Window");
    let win2 = app.open_window("Second Window");

    assert_eq!(app.window_count(), 2);
    assert_eq!(app.child_views_alive(), 2);
    assert!(app.has_open_windows());

    // Close the first window
    let closed1 = app.close_window(win1);
    assert!(closed1);
    assert_eq!(app.window_count(), 1);
    assert_eq!(app.child_views_alive(), 1);
    assert!(app.has_open_windows());

    // Close the last window
    let closed2 = app.close_window(win2);
    assert!(closed2);
    assert_eq!(app.window_count(), 0);
    assert_eq!(
        app.child_views_alive(),
        0,
        "no child view must be left alive"
    );
    assert!(
        !app.has_open_windows(),
        "process run loop ends when no windows remain"
    );

    // Attempting to close again returns false and child views count remains 0
    assert!(!app.close_window(win1));
    assert_eq!(app.child_views_alive(), 0);
}
