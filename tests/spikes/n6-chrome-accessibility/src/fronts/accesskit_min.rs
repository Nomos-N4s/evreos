//! The minimal AccessKit front: a three-node chrome tree — window root, a
//! labelled address text input, a button — published through
//! `accesskit_winit` beside the host's content webview, with Tab traversal
//! wired so focus order can be exercised.
//!
//! Everything the front attempts is logged to stderr with a `[n6]` prefix,
//! because the spike exists to be watched: the cross-tree labelled-by
//! attempt is logged with the structural reason it cannot be expressed, and
//! focus traversal is logged as it happens.

use accesskit::{
    Action, ActionRequest, Affine, Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate,
};
use accesskit_winit::{Adapter, WindowEvent as AccessKitWindowEvent};
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::keyboard::{Key, NamedKey};
use winit::window::Window;
use wry::WebView;

use crate::host::{CHROME_HEIGHT, ChromeFront};

const WINDOW_ID: NodeId = NodeId(0);
const ADDRESS_ID: NodeId = NodeId(1);
const GO_ID: NodeId = NodeId(2);

// Logical bounds inside the chrome strip the host reserves.
const ADDRESS_RECT: Rect = Rect {
    x0: 16.0,
    y0: 24.0,
    x1: 616.0,
    y1: 64.0,
};
const GO_RECT: Rect = Rect {
    x0: 632.0,
    y0: 24.0,
    x1: 712.0,
    y1: 64.0,
};

pub struct AccessKitMinFront {
    adapter: Option<Adapter>,
    /// Which chrome node holds the chrome tree's focus. Once focus is handed
    /// across to the webview this stays on the last chrome node, and the
    /// window's own focus loss is what tells the platform where focus went.
    focus: NodeId,
}

impl AccessKitMinFront {
    pub fn new() -> Self {
        Self {
            adapter: None,
            focus: ADDRESS_ID,
        }
    }

    fn build_address(&self) -> Node {
        let mut node = Node::new(Role::TextInput);
        node.set_bounds(ADDRESS_RECT);
        node.set_label("Address");
        node.set_value("about:spike");
        node.add_action(Action::Focus);
        // The join T015 measures would be expressed here, if it could be
        // expressed at all; log_cross_tree_attempt says why it cannot, and
        // the local label above is the fallback.
        node
    }

    fn build_go_button(&self) -> Node {
        let mut node = Node::new(Role::Button);
        node.set_bounds(GO_RECT);
        node.set_label("Go");
        node.add_action(Action::Focus);
        node.add_action(Action::Click);
        node
    }

    fn build_root(&self, window: &Window) -> Node {
        let size = window.inner_size().to_logical::<f64>(window.scale_factor());
        let mut node = Node::new(Role::Window);
        node.set_bounds(Rect {
            x0: 0.0,
            y0: 0.0,
            x1: size.width,
            y1: size.height.min(CHROME_HEIGHT),
        });
        node.set_transform(Affine::scale(window.scale_factor()));
        node.set_children(vec![ADDRESS_ID, GO_ID]);
        node.set_label("Evreos N6 spike chrome");
        node
    }

    fn build_initial_tree(&self, window: &Window) -> TreeUpdate {
        TreeUpdate {
            nodes: vec![
                (WINDOW_ID, self.build_root(window)),
                (ADDRESS_ID, self.build_address()),
                (GO_ID, self.build_go_button()),
            ],
            tree: Some(TreeInfo::new(WINDOW_ID)),
            tree_id: TreeId::ROOT,
            focus: self.focus,
        }
    }

    fn set_focus(&mut self, focus: NodeId, why: &str) {
        self.focus = focus;
        if let Some(adapter) = self.adapter.as_mut() {
            adapter.update_if_active(|| TreeUpdate {
                nodes: vec![],
                tree: None,
                tree_id: TreeId::ROOT,
                focus,
            });
        }
        eprintln!(
            "[n6] focus: chrome tree focus set to {} ({why})",
            name_of(focus)
        );
    }

    /// The cross-tree labelled-by attempt, logged rather than performed,
    /// because the API gives it no way to be performed.
    ///
    /// What T015 wants to express: the chrome address input, labelled by the
    /// content heading (`#content-heading`) inside the webview. What the API
    /// offers: `Node::set_labelled_by` takes `accesskit::NodeId` values, and
    /// a `NodeId` is a bare 64-bit identifier whose meaning is scoped to the
    /// AccessKit tree the enclosing `TreeUpdate` names — here the adapter's
    /// own tree, `TreeId::ROOT`. The webview's content tree is not an
    /// AccessKit tree at all: it is the platform accessibility tree the
    /// system webview publishes itself, with its own runtime identifiers, and
    /// it is never registered with this adapter. So no `NodeId` in the chrome
    /// tree's namespace denotes a content node, and accesskit 0.25.0 carries
    /// no node property that references or embeds a foreign platform tree —
    /// `TreeId` reaches only trees pushed through AccessKit itself. The
    /// reference is structurally inexpressible, not merely unimplemented
    /// here.
    fn log_cross_tree_attempt(&self) {
        eprintln!(
            "[n6] cross-tree: attempting the labelled-by join T015 measures: \
             chrome address input labelled by the content heading in the webview"
        );
        eprintln!(
            "[n6] cross-tree: not expressible: Node::set_labelled_by takes accesskit::NodeId \
             values, resolved inside the tree this adapter publishes (TreeId::ROOT)"
        );
        eprintln!(
            "[n6] cross-tree: the webview's content tree is the platform tree the system \
             webview publishes itself, with its own runtime identifiers; it is not an \
             AccessKit tree, so no NodeId here denotes a node there"
        );
        eprintln!(
            "[n6] cross-tree: accesskit 0.25.0 has no node property that references or embeds \
             a foreign platform tree; falling back to the local label 'Address'"
        );
    }
}

fn name_of(id: NodeId) -> &'static str {
    match id {
        WINDOW_ID => "the chrome window root",
        ADDRESS_ID => "the address input",
        GO_ID => "the go button",
        _ => "an unknown node",
    }
}

impl ChromeFront for AccessKitMinFront {
    fn attach(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: &Window,
        proxy: EventLoopProxy<accesskit_winit::Event>,
    ) {
        self.adapter = Some(Adapter::with_event_loop_proxy(event_loop, window, proxy));
        eprintln!(
            "[n6] front: accesskit adapter created; the chrome tree is published on demand \
             (window root, address input, go button)"
        );
        self.log_cross_tree_attempt();
    }

    fn on_window_event(&mut self, window: &Window, webview: &WebView, event: &WindowEvent) {
        if let Some(adapter) = self.adapter.as_mut() {
            adapter.process_event(window, event);
        }
        match event {
            WindowEvent::Focused(focused) => {
                eprintln!(
                    "[n6] focus: the host window {} platform focus",
                    if *focused { "gained" } else { "lost" }
                );
            }
            WindowEvent::Resized(_) => {
                let update = self.build_initial_tree(window);
                if let Some(adapter) = self.adapter.as_mut() {
                    adapter.update_if_active(move || update);
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Tab),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                if self.focus == ADDRESS_ID {
                    self.set_focus(GO_ID, "Tab from the address input");
                } else {
                    eprintln!(
                        "[n6] focus: Tab from the go button hands focus across the tree \
                         boundary to the content webview; traversal inside it is the \
                         system webview's own"
                    );
                    if let Err(error) = webview.focus() {
                        eprintln!("[n6] focus: the webview declined focus: {error}");
                    }
                }
            }
            _ => {}
        }
    }

    fn on_accesskit_event(
        &mut self,
        window: &Window,
        _webview: &WebView,
        event: &AccessKitWindowEvent,
    ) {
        match event {
            AccessKitWindowEvent::InitialTreeRequested => {
                eprintln!(
                    "[n6] accesskit: initial chrome tree requested — an assistive technology \
                     is reading it"
                );
                let update = self.build_initial_tree(window);
                if let Some(adapter) = self.adapter.as_mut() {
                    adapter.update_if_active(move || update);
                }
            }
            AccessKitWindowEvent::ActionRequested(ActionRequest {
                action,
                target_node,
                ..
            }) => {
                eprintln!(
                    "[n6] accesskit: action {action:?} requested on {}",
                    name_of(*target_node)
                );
                match action {
                    Action::Focus if *target_node == ADDRESS_ID || *target_node == GO_ID => {
                        self.set_focus(*target_node, "assistive-technology focus action");
                    }
                    Action::Click if *target_node == GO_ID => {
                        eprintln!(
                            "[n6] accesskit: the go button was clicked; the spike acts on nothing"
                        );
                    }
                    _ => {}
                }
            }
            AccessKitWindowEvent::AccessibilityDeactivated => {
                eprintln!("[n6] accesskit: accessibility deactivated");
            }
        }
    }
}
