//! The Evreos browser chrome crate.
//!
//! Owns the browser's own chrome surfaces (omnibox, tab strip, window controls,
//! navigation controls, cashback offer controls). Built against the windowing
//! crate `winit` 0.30 and `accesskit` selected by `docs/adr/0002-chrome-renderer.md`.
//!
//! Under T038, this crate lands no member-facing surface: the omnibox, tabs,
//! error states and every other chrome surface belong to the tasks that own
//! them and are built against the window and event dispatch this crate and
//! `evreos-shell` provide.

#![forbid(unsafe_code)]

use evreos_engine::NavigationEvent;
use std::thread::{self, ThreadId};
use winit::event::WindowEvent;

/// Representation of input events dispatched to chrome surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum ChromeInput {
    /// A window event passed from the platform event loop.
    Window(WindowEvent),
}

impl From<WindowEvent> for ChromeInput {
    fn from(event: WindowEvent) -> Self {
        Self::Window(event)
    }
}

/// Trait implemented by chrome surface renderers and handlers.
///
/// Per ADR-0002:
/// - Event loop dispatch occurs strictly on the platform UI thread.
/// - Chrome surfaces receive input events and engine navigation events.
pub trait ChromeSurface: 'static {
    /// Handle an input event delivered from the UI event loop.
    fn on_input(&mut self, event: &WindowEvent);

    /// Handle a navigation event forwarded from the engine seam.
    fn on_navigation(&mut self, event: &NavigationEvent);

    /// Downcast support for test inspection and concrete implementations.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable downcast support.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Default minimal chrome implementation for windows before specific chrome components land.
#[derive(Debug, Default)]
pub struct DefaultChrome {
    input_events: Vec<WindowEvent>,
    navigation_events: Vec<NavigationEvent>,
    last_input_thread: Option<ThreadId>,
    last_navigation_thread: Option<ThreadId>,
}

impl DefaultChrome {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn input_count(&self) -> usize {
        self.input_events.len()
    }

    pub fn navigation_count(&self) -> usize {
        self.navigation_events.len()
    }

    pub fn last_input_thread(&self) -> Option<ThreadId> {
        self.last_input_thread
    }

    pub fn last_navigation_thread(&self) -> Option<ThreadId> {
        self.last_navigation_thread
    }

    pub fn received_inputs(&self) -> &[WindowEvent] {
        &self.input_events
    }

    pub fn received_navigations(&self) -> &[NavigationEvent] {
        &self.navigation_events
    }
}

impl ChromeSurface for DefaultChrome {
    fn on_input(&mut self, event: &WindowEvent) {
        self.last_input_thread = Some(thread::current().id());
        self.input_events.push(event.clone());
    }

    fn on_navigation(&mut self, event: &NavigationEvent) {
        self.last_navigation_thread = Some(thread::current().id());
        self.navigation_events.push(event.clone());
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_chrome_records_input_and_navigation_on_current_thread() {
        let mut chrome = DefaultChrome::new();
        assert_eq!(chrome.input_count(), 0);
        assert_eq!(chrome.navigation_count(), 0);

        let input_event = WindowEvent::Focused(true);
        chrome.on_input(&input_event);
        assert_eq!(chrome.input_count(), 1);
        assert_eq!(chrome.last_input_thread(), Some(thread::current().id()));

        let nav_event = NavigationEvent::Started {
            id: evreos_engine::NavigationId::FIRST,
            address: "https://example.invalid/".into(),
        };
        chrome.on_navigation(&nav_event);
        assert_eq!(chrome.navigation_count(), 1);
        assert_eq!(
            chrome.last_navigation_thread(),
            Some(thread::current().id())
        );
    }
}
