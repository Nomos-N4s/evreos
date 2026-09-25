//! Application window and UI event loop for the Evreos shell.
//!
//! Owns the application lifecycle, top-level windowing, chrome attachment,
//! and engine seam event dispatch.
//!
//! # Platform Thread Invariant
//!
//! Per `docs/adr/0002-chrome-renderer.md` and SC-006:
//! - All UI window event handling, chrome input dispatch, and engine navigation
//!   event dispatch MUST execute on the platform's required main UI thread.
//! - Violations are caught and reported as thread assertions naming the fault.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, ThreadId};

use crate::store::WindowKind;
use evreos_chrome::{ChromeSurface, DefaultChrome};
use evreos_engine::{DataStoreSelector, Engine, NavigationEvent, SurfaceId};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId as WinitWindowId};

/// Unique identifier for an application window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AppWindowId(u64);

impl AppWindowId {
    pub const FIRST: AppWindowId = AppWindowId(1);

    pub fn next(self) -> AppWindowId {
        AppWindowId(self.0 + 1)
    }
}

impl fmt::Display for AppWindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "win-{}", self.0)
    }
}

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

pub fn mint_window_id() -> AppWindowId {
    AppWindowId(NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed))
}

/// Represents an application window owning its chrome and engine rendering surface.
pub struct AppWindow {
    id: AppWindowId,
    title: String,
    surface_id: SurfaceId,
    winit_window: Option<Window>,
    chrome: Box<dyn ChromeSurface>,
    is_closed: bool,
    kind: WindowKind,
}

impl AppWindow {
    pub fn id(&self) -> AppWindowId {
        self.id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    pub fn chrome(&self) -> &dyn ChromeSurface {
        &*self.chrome
    }

    pub fn chrome_mut(&mut self) -> &mut dyn ChromeSurface {
        &mut *self.chrome
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    pub fn kind(&self) -> WindowKind {
        self.kind
    }

    pub fn is_private(&self) -> bool {
        self.kind == WindowKind::Private
    }

    pub fn data_store(&self) -> DataStoreSelector {
        match self.kind {
            WindowKind::Normal => DataStoreSelector::Persistent,
            WindowKind::Private => DataStoreSelector::NonPersistent,
        }
    }
}

/// Application controller managing windows, UI event loop, and engine seam integration.
pub struct App<E: Engine> {
    engine: E,
    ui_thread: ThreadId,
    windows: HashMap<AppWindowId, AppWindow>,
    winit_map: HashMap<WinitWindowId, AppWindowId>,
    active_child_views: usize,
    default_title: String,
}

impl<E: Engine> App<E> {
    /// Create a new application instance bound to the calling thread as the UI thread.
    pub fn new(engine: E) -> Self {
        Self::with_title(engine, "Evreos")
    }

    /// Create a new application instance with a default window title.
    pub fn with_title(engine: E, default_title: impl Into<String>) -> Self {
        Self {
            engine,
            ui_thread: thread::current().id(),
            windows: HashMap::new(),
            winit_map: HashMap::new(),
            active_child_views: 0,
            default_title: default_title.into(),
        }
    }

    /// Returns the thread ID required for all UI operations.
    pub fn ui_thread_id(&self) -> ThreadId {
        self.ui_thread
    }

    /// Asserts that the caller is executing on the designated platform UI thread.
    ///
    /// # Panics
    ///
    /// Panics if called from any thread other than `ui_thread`.
    pub fn assert_ui_thread(&self) {
        assert_eq!(
            thread::current().id(),
            self.ui_thread,
            "UI call off the event-loop thread"
        );
    }

    /// Override the designated UI thread ID for testing thread-affinity assertions.
    pub fn set_ui_thread_for_testing(&mut self, thread_id: ThreadId) {
        self.ui_thread = thread_id;
    }

    /// Reference to the underlying engine.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Mutable reference to the underlying engine.
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// Number of currently open windows.
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Whether any window remains open.
    pub fn has_open_windows(&self) -> bool {
        !self.windows.is_empty()
    }

    /// Number of child rendering surfaces currently alive.
    pub fn child_views_alive(&self) -> usize {
        self.active_child_views
    }

    /// Open an application window with default chrome.
    pub fn open_window(&mut self, title: impl Into<String>) -> AppWindowId {
        self.open_window_with_chrome(title, Box::new(DefaultChrome::new()))
    }

    /// Open an application window with a custom chrome surface.
    pub fn open_window_with_chrome(
        &mut self,
        title: impl Into<String>,
        chrome: Box<dyn ChromeSurface>,
    ) -> AppWindowId {
        self.assert_ui_thread();
        let id = mint_window_id();
        let surface_id = self.engine.create_surface(DataStoreSelector::Persistent);
        self.active_child_views += 1;

        let window = AppWindow {
            id,
            title: title.into(),
            surface_id,
            winit_window: None,
            chrome,
            is_closed: false,
            kind: WindowKind::Normal,
        };

        self.windows.insert(id, window);
        id
    }

    /// Open a private browsing window with default chrome.
    ///
    /// Under FR-007, selects [`DataStoreSelector::NonPersistent`] on the engine seam.
    pub fn open_private_window(&mut self, title: impl Into<String>) -> AppWindowId {
        self.open_private_window_with_chrome(title, Box::new(DefaultChrome::new()))
    }

    /// Open a private browsing window with a custom chrome surface.
    ///
    /// Under FR-007, selects [`DataStoreSelector::NonPersistent`] on the engine seam.
    pub fn open_private_window_with_chrome(
        &mut self,
        title: impl Into<String>,
        chrome: Box<dyn ChromeSurface>,
    ) -> AppWindowId {
        self.assert_ui_thread();
        let id = mint_window_id();
        let surface_id = self.engine.create_surface(DataStoreSelector::NonPersistent);
        self.active_child_views += 1;

        let window = AppWindow {
            id,
            title: title.into(),
            surface_id,
            winit_window: None,
            chrome,
            is_closed: false,
            kind: WindowKind::Private,
        };

        self.windows.insert(id, window);
        id
    }

    /// Whether the specified window is a private browsing window.
    pub fn is_private_window(&self, id: AppWindowId) -> bool {
        self.windows
            .get(&id)
            .map(|w| w.is_private())
            .unwrap_or(false)
    }

    /// Create an application window backed by a real `winit` window.
    pub fn create_winit_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        title: impl Into<String>,
        chrome: Box<dyn ChromeSurface>,
    ) -> Result<AppWindowId, Box<dyn std::error::Error>> {
        self.assert_ui_thread();
        let title_str = title.into();
        let id = mint_window_id();
        let surface_id = self.engine.create_surface(DataStoreSelector::Persistent);
        self.active_child_views += 1;

        let attributes = WindowAttributes::default()
            .with_title(&title_str)
            .with_inner_size(LogicalSize::new(1024.0, 768.0));

        let winit_win = event_loop.create_window(attributes)?;
        let winit_id = winit_win.id();

        let window = AppWindow {
            id,
            title: title_str,
            surface_id,
            winit_window: Some(winit_win),
            chrome,
            is_closed: false,
            kind: WindowKind::Normal,
        };

        self.winit_map.insert(winit_id, id);
        self.windows.insert(id, window);
        Ok(id)
    }

    /// Close an application window by ID, destroying its child surface and updating state.
    pub fn close_window(&mut self, id: AppWindowId) -> bool {
        self.assert_ui_thread();
        if let Some(mut window) = self.windows.remove(&id) {
            window.is_closed = true;
            // Clean up winit mapping if present
            if let Some(ref winit_win) = window.winit_window {
                self.winit_map.remove(&winit_win.id());
            }
            // Destroy the child surface in the engine
            self.engine.close_surface(window.surface_id);
            if self.active_child_views > 0 {
                self.active_child_views -= 1;
            }
            true
        } else {
            false
        }
    }

    /// Get a reference to a window by ID.
    pub fn get_window(&self, id: AppWindowId) -> Option<&AppWindow> {
        self.windows.get(&id)
    }

    /// Get a mutable reference to a window by ID.
    pub fn get_window_mut(&mut self, id: AppWindowId) -> Option<&mut AppWindow> {
        self.windows.get_mut(&id)
    }

    /// Dispatch an input event to the chrome of the specified window on the UI thread.
    pub fn dispatch_input(&mut self, window_id: AppWindowId, event: &WindowEvent) {
        self.assert_ui_thread();
        if let Some(window) = self.windows.get_mut(&window_id) {
            window.chrome.on_input(event);
        }
    }

    /// Poll navigation events from the engine seam and dispatch them to open window chrome surfaces on the UI thread.
    pub fn dispatch_navigation_events(&mut self) -> Vec<NavigationEvent> {
        self.assert_ui_thread();
        let mut dispatched = Vec::new();
        while let Some(event) = self.engine.poll_event() {
            for window in self.windows.values_mut() {
                window.chrome.on_navigation(&event);
            }
            dispatched.push(event);
        }
        dispatched
    }

    /// Run the application event loop using `winit` until the last window closes.
    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.assert_ui_thread();
        let event_loop = EventLoop::new()?;
        event_loop.run_app(self).map_err(Into::into)
    }
}

impl<E: Engine> ApplicationHandler for App<E> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.assert_ui_thread();
        if self.windows.is_empty() {
            let title = self.default_title.clone();
            if let Err(error) =
                self.create_winit_window(event_loop, title, Box::new(DefaultChrome::new()))
            {
                eprintln!("[evreos-shell] failed to create initial window: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WinitWindowId,
        event: WindowEvent,
    ) {
        self.assert_ui_thread();
        let Some(&app_id) = self.winit_map.get(&window_id) else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                self.close_window(app_id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
            }
            ref input_event => {
                self.dispatch_input(app_id, input_event);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.assert_ui_thread();
        self.dispatch_navigation_events();
        if self.windows.is_empty() {
            event_loop.exit();
        }
    }
}
