//! The reusable spike scaffold: one winit window, one wry webview held as a
//! child with bounds the host manages, and the seam a chrome front plugs
//! into.
//!
//! The split is deliberate groundwork for the later real window work. The
//! host owns what any chrome renderer would need — the event loop, the
//! window, the embedded content webview and its geometry — and knows nothing
//! about how the chrome presents itself. A front owns exactly that
//! presentation: for T015 it is a minimal AccessKit tree, and a later
//! renderer candidate is another [`ChromeFront`] behind the same three calls.
//!
//! Threading: winit delivers every callback on the thread that entered
//! [`run`], and every UI object here is confined to it. Debug builds assert
//! that on each callback, so a violation says where it happened rather than
//! failing somewhere inside a platform call.

use std::error::Error;
use std::thread::{self, ThreadId};

use accesskit_winit::Event as AccessKitEvent;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};
use wry::{Rect, WebView, WebViewBuilder};

/// Logical height of the strip across the top that belongs to the chrome
/// front. The webview gets everything below it.
pub const CHROME_HEIGHT: f64 = 88.0;

const WINDOW_TITLE: &str = "Evreos spike: N6 chrome accessibility";

/// The content the webview renders, inline so the spike touches no network:
/// a heading, a labelled text input and a button — enough for the content
/// accessibility tree to hold nodes a chrome node would want to join
/// against.
const CONTENT_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>N6 content page</title>
</head>
<body>
<h1 id="content-heading">Content region</h1>
<p>This page exists so the content accessibility tree has nodes.</p>
<label for="content-input">Content input</label>
<input id="content-input" type="text">
<button id="content-button" type="button">Content button</button>
</body>
</html>
"#;

/// The seam a chrome renderer plugs into.
///
/// The host calls [`attach`](ChromeFront::attach) once, then forwards every
/// window event and every AccessKit adapter event. All three calls arrive on
/// the event-loop thread.
pub trait ChromeFront {
    /// Called once, after the window and the child webview exist and before
    /// the window is first shown — an AccessKit adapter must be created
    /// before the window becomes visible, and this is the front's chance to
    /// create one.
    fn attach(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        proxy: EventLoopProxy<AccessKitEvent>,
    );

    /// Every winit window event for the host's window, before the host acts
    /// on it.
    fn on_window_event(&mut self, window: &Window, webview: &WebView, event: &WindowEvent);

    /// Every AccessKit adapter event the host's event loop carries for the
    /// host's window.
    fn on_accesskit_event(
        &mut self,
        window: &Window,
        webview: &WebView,
        event: &accesskit_winit::WindowEvent,
    );
}

struct WindowState {
    // Field order is drop order: the webview must go before the window that
    // parents it.
    webview: WebView,
    window: Window,
}

struct Host {
    ui_thread: ThreadId,
    proxy: EventLoopProxy<AccessKitEvent>,
    front: Box<dyn ChromeFront>,
    state: Option<WindowState>,
}

impl Host {
    /// Debug assertion that the caller is on the event-loop thread. Every
    /// winit, wry and AccessKit call here must be; this names the violation
    /// at the callback that made it.
    fn assert_ui_thread(&self) {
        debug_assert_eq!(
            thread::current().id(),
            self.ui_thread,
            "UI call off the event-loop thread"
        );
    }

    /// The bounds the content webview keeps: the window, minus the chrome
    /// strip.
    fn webview_bounds(window: &Window) -> Rect {
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        Rect {
            position: LogicalPosition::new(0.0, CHROME_HEIGHT).into(),
            size: LogicalSize::new(size.width, (size.height - CHROME_HEIGHT).max(0.0)).into(),
        }
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<WindowState, Box<dyn Error>> {
        let attributes = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(900.0, 640.0))
            .with_visible(false);
        let window = event_loop.create_window(attributes)?;
        let webview = WebViewBuilder::new()
            .with_html(CONTENT_HTML)
            .with_bounds(Self::webview_bounds(&window))
            .build_as_child(&window)?;
        // Attach the front while the window is still hidden: an AccessKit
        // adapter created after the window first shows can miss the platform's
        // first accessibility query.
        self.front.attach(event_loop, &window, self.proxy.clone());
        window.set_visible(true);
        eprintln!("[n6] host: window and child webview are up; the chrome front is attached");
        Ok(WindowState { webview, window })
    }
}

impl ApplicationHandler<AccessKitEvent> for Host {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.assert_ui_thread();
        if self.state.is_some() {
            return;
        }
        match self.create_window(event_loop) {
            Ok(state) => self.state = Some(state),
            Err(error) => {
                eprintln!("[n6] host: cannot create the window and webview: {error}");
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.assert_ui_thread();
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        self.front
            .on_window_event(&state.window, &state.webview, &event);
        match event {
            WindowEvent::CloseRequested => {
                self.state = None;
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Err(error) = state
                    .webview
                    .set_bounds(Self::webview_bounds(&state.window))
                {
                    eprintln!("[n6] host: cannot resize the webview with the window: {error}");
                }
            }
            _ => {}
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AccessKitEvent) {
        self.assert_ui_thread();
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if event.window_id != state.window.id() {
            return;
        }
        self.front
            .on_accesskit_event(&state.window, &state.webview, &event.window_event);
    }
}

/// Run the host with the given chrome front until its window closes.
pub fn run(front: Box<dyn ChromeFront>) -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::<AccessKitEvent>::with_user_event().build()?;
    let proxy = event_loop.create_proxy();
    let mut host = Host {
        ui_thread: thread::current().id(),
        proxy,
        front,
        state: None,
    };
    event_loop.run_app(&mut host).map_err(Into::into)
}
