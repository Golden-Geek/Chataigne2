use std::any::Any;

use crate::color::Color;
use crate::define_node_type;
use crate::edit::Edit;
use crate::engine::NodeExecutionRule;
use crate::events::{CustomEvent, Event, EventKind};
use crate::parameter::ParamValue;
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Stable engine identifier for a node stored in [`crate::engine::node_store::NodeStore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

/// Persistent UUID assigned to node metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeUuid(pub Uuid);

/// Declaration identifier used to refer to node definitions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeclId(pub String);

/// Semantic hints used for tooling, UX, and interpretation.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticsHint {
    /// Optional high-level intent of the node.
    pub intent: Option<String>,
    /// Optional unit for value-oriented nodes.
    pub unit: Option<String>,
}

/// Presentation hints used for editor rendering.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationHint {
    /// Preferred UI color.
    pub color: Option<Color>,
}

/// Runtime node links and metadata.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    /// Stable node id assigned by the store.
    pub id: NodeId,
    /// Parent node id (`None` for root or detached nodes).
    pub parent: Option<NodeId>,
    /// First child in sibling chain.
    pub first_child: Option<NodeId>,
    /// Last child in sibling chain.
    pub last_child: Option<NodeId>,
    /// Previous sibling in parent chain.
    pub prev_sibling: Option<NodeId>,
    /// Next sibling in parent chain.
    pub next_sibling: Option<NodeId>,
    /// User-facing metadata.
    pub meta: NodeMeta,
}

impl NodeData {
    /// Creates detached node data initialized with default metadata.
    pub fn new(label: String) -> Self {
        println!("New node data, label: {}", label);
        let meta = NodeMeta::new(label);

        Self {
            id: NodeId(0),
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
            meta,
        }
    }
}

/// Metadata associated with a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeMeta {
    /// Persistent unique id for this node instance.
    pub uuid: NodeUuid,
    /// Declaration id used by schema and tools.
    pub decl_id: DeclId,
    /// Generated short name.
    pub short_name: String,
    /// Whether the node is currently enabled.
    pub enabled: bool,
    /// Whether the enabled flag may be toggled.
    pub can_be_disabled: bool,
    /// User-visible label.
    pub label: String,
    /// Optional free-form description.
    pub description: Option<String>,
    /// Arbitrary classification tags.
    pub tags: Vec<String>,
    /// Semantic hints.
    pub semantics: SemanticsHint,
    /// Presentation hints.
    pub presentation: PresentationHint,
}

impl NodeMeta {
    /// Creates default metadata from a label.
    pub fn new(label: String) -> Self {
        let short_name = Self::generate_short_name(&label);

        Self {
            uuid: NodeUuid(Uuid::new_v4()),
            decl_id: DeclId(short_name.clone()),
            short_name,
            enabled: true,
            can_be_disabled: true,
            label,
            description: None,
            tags: vec![],
            semantics: SemanticsHint::default(),
            presentation: PresentationHint::default(),
        }
    }

    /// Sets a description.
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Replaces tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Replaces semantic hints.
    pub fn with_semantics(mut self, semantics: SemanticsHint) -> Self {
        self.semantics = semantics;
        self
    }

    /// Replaces presentation hints.
    pub fn with_presentation(mut self, presentation: PresentationHint) -> Self {
        self.presentation = presentation;
        self
    }

    /// Sets enablement metadata.
    pub fn with_enabled(mut self, enabled: bool, can_be_disabled: bool) -> Self {
        self.enabled = enabled;
        self.can_be_disabled = can_be_disabled;
        self
    }

    fn generate_short_name(label: &String) -> String {
        //from label to lowerCamelCase, "+" and "-" are replaced by "Plus" and "Minus", other are removed
        let mut short_name = String::new();
        let mut capitalize_next = false;
        for c in label.chars() {
            if c.is_alphanumeric() {
                if capitalize_next {
                    short_name.push(c.to_ascii_uppercase());
                    capitalize_next = false;
                } else {
                    short_name.push(c.to_ascii_lowercase());
                }
            } else if c == '+' {
                short_name.push_str(if capitalize_next { "Plus" } else { "plus" });
                capitalize_next = false;
            } else if c == '-' {
                short_name.push_str(if capitalize_next { "Minus" } else { "minus" });
                capitalize_next = false;
            } else {
                capitalize_next = true;
            }
        }
        short_name
    }
}

/// Patch placeholder for metadata updates.
///
/// This currently carries no fields and acts as a forward-compatible marker.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeMetaPatch {}

/// Controls whether an event reaching a node should notify, pass through, or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPropagation {
    /// Notify this node when it is interested and continue bubbling.
    Notify,
    /// Do not notify this node, but still allow bubbling to continue.
    PassOn,
    /// Notify this node when it is interested and stop bubbling.
    Stop,
}

/// Runtime listener subscription targeting a node and optional subtree depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventSubscription {
    /// Subscription root node id.
    pub node: NodeId,
    /// Maximum descendant depth from `node` that should match.
    ///
    /// `0` matches only `node`, `1` includes direct children, and so on.
    pub max_depth: u32,
}

impl EventSubscription {
    /// Creates a subscription matching events originating exactly from `node`.
    pub fn node(node: NodeId) -> Self {
        Self { node, max_depth: 0 }
    }

    /// Creates a subscription matching `node` and descendants up to `max_depth`.
    pub fn subtree(node: NodeId, max_depth: u32) -> Self {
        Self { node, max_depth }
    }
}

/// Behavior contract implemented by all node types.
pub trait Node: Send + Any {
    /// Returns immutable runtime node data.
    fn node_data(&self) -> &NodeData;
    /// Returns mutable runtime node data.
    fn node_data_mut(&mut self) -> &mut NodeData;

    /// Returns the node type identifier.
    fn get_type(&self) -> &str;

    /// Applies a parameter value to the node and returns the previous value when supported.
    fn set_param_value(&mut self, _value: ParamValue) -> Option<ParamValue> {
        None
    }

    /// Attempts to downcast a boxed trait object into `Self`.
    fn from_boxed_node(node: Box<dyn Node>) -> Option<Self>
    where
        Self: Sized,
    {
        let any: Box<dyn Any> = node;
        any.downcast::<Self>().ok().map(|node| *node)
    }

    /// Returns this node id.
    fn id(&self) -> NodeId {
        self.node_data().id
    }

    /// Returns `true` when this node id matches `id`.
    fn is(&self, id: NodeId) -> bool {
        self.id() == id
    }

    /// Called when the node is initialized.
    fn init(&mut self, _ctx: &mut ProcessCtx) {}
    /// Called at this node's update rate.
    fn update(&mut self, _ctx: &mut ProcessCtx) {} // called at this node's desired update rate
    /// Called before node destruction.
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {}

    /// Returns the runtime execution rule used by the engine scheduler.
    ///
    /// The default rule is passive: no dependencies and no update rate.
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::default()
    }

    /// Returns how many descendant levels of events this node subscribes to.
    ///
    /// `0` means this node does not subscribe to descendant events, `1` means it subscribes to direct child events, etc.
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        1
    }
    /// Returns how many additional ancestor hops this node grants to a received event.
    ///
    /// `0` means no additional bubbling, `1` allows bubbling to the direct parent by default.
    fn bubble_event_depth(&self, _event: &Event) -> u32 {
        1
    }
    /// Returns propagation behavior for an event that has reached this node.
    ///
    /// `depth` is the ancestor distance from event origin (`0` for the origin node).
    fn event_propagation(&self, _event: &Event, _depth: u32) -> EventPropagation {
        EventPropagation::Notify
    }
    /// Dispatches inbox events to per-event handlers.
    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
    }

    /// Queues insertion of a boxed child node.
    fn add_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_child_boxed(self.id(), child, after);
    }

    /// Queues insertion of a typed child node.
    fn add_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_child_boxed(ctx, Box::new(child), after);
    }
    /// Queues removal of an existing child.
    fn remove_child(&mut self, ctx: &mut ProcessCtx, child: NodeId) {
        ctx.edits.push(Edit::RemoveNode { node: child });
    }
    /// Queues move of an existing child.
    fn move_child(&mut self, ctx: &mut ProcessCtx, child: NodeId, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode { node: child, new_parent, new_prev_sibling: after });
    }
    /// Subscribes this node to direct events originating from `target`.
    fn add_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.add_event_listener(self.id(), target);
    }
    /// Subscribes this node to events from `target` and descendants up to `max_depth`.
    fn add_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.add_event_listener_subtree(self.id(), target, max_depth);
    }
    /// Removes this node's direct listener subscription to `target`.
    fn remove_listener(&mut self, ctx: &mut ProcessCtx, target: NodeId) {
        ctx.remove_event_listener(self.id(), target);
    }
    /// Removes this node's subtree listener subscription to `target`.
    fn remove_listener_subtree(&mut self, ctx: &mut ProcessCtx, target: NodeId, max_depth: u32) {
        ctx.remove_event_listener_subtree(self.id(), target, max_depth);
    }
    /// Queues replacement of a child by a boxed node.
    fn replace_child_boxed(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(old, new_node);
    }
    /// Queues replacement of a child by a typed node.
    fn replace_child<N>(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: N)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.replace_child_boxed(ctx, old, Box::new(new_node));
    }

    /// Dispatches all events in the process context to typed handlers.
    fn dispatch_inbox(&mut self, ctx: &mut ProcessCtx) {
        for event in ctx.events.clone() {
            match event.kind {
                EventKind::ParamChanged { param, old_value } => {
                    self.on_param_change(ctx, param, old_value);
                }
                EventKind::ChildAdded { parent, child } => {
                    self.on_child_added(ctx, parent, child);
                }
                EventKind::ChildRemoved { parent, child } => {
                    self.on_child_removed(ctx, parent, child);
                }
                EventKind::ChildReplaced { parent, old, new } => {
                    self.on_child_replaced(ctx, parent, old, new);
                }
                EventKind::ChildMoved { child, old_parent, new_parent } => {
                    self.on_child_moved(ctx, child, old_parent, new_parent);
                }
                EventKind::ChildReordered { parent, child } => {
                    self.on_child_reordered(ctx, parent, child);
                }
                EventKind::NodeCreated { node } => {
                    self.on_node_created(ctx, node);
                }
                EventKind::NodeDeleted { node } => {
                    self.on_node_deleted(ctx, node);
                }
                EventKind::MetaChanged { node, patch } => {
                    self.on_meta_changed(ctx, node, patch);
                }
                EventKind::Custom(event) => {
                    self.on_custom_event(ctx, event);
                }
            }
        }
    }

    /// Called when a parameter has changed.
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {}
    /// Called when a child is added.
    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a child is removed.
    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a child is replaced.
    fn on_child_replaced(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {}
    /// Called when a child is moved to another parent.
    fn on_child_moved(&mut self, _ctx: &mut ProcessCtx, _child: NodeId, _old_parent: NodeId, _new_parent: NodeId) {}
    /// Called when a child is reordered under the same parent.
    fn on_child_reordered(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    /// Called when a node is created.
    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    /// Called when a node is deleted.
    fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    /// Called when node metadata changes.
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {}
    /// Called when a custom event is emitted.
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {}
}

define_node_type!(
    /// Internal Folder-like node used as empty organizational structure and root for user content without process or bubbling.
    pub struct Folder {
    }
    type_name: "folder",
    node_impl {
        fn init(&mut self, _ctx: &mut ProcessCtx) {
            println!("Folder init");
        }

        fn destroy(&mut self, _ctx: &mut ProcessCtx) {
            println!("Folder destroy");
        }

        fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
            EventPropagation::PassOn
        }
    }
);

define_node_type!(
    /// Internal Folder-like node used as a curated user-extensible root.
    pub struct Manager {
    }
    type_name: "manager",
    node_impl {
        fn init(&mut self, _ctx: &mut ProcessCtx) {
            println!("Manager init");
        }

        fn destroy(&mut self, _ctx: &mut ProcessCtx) {
            println!("Manager destroy");
        }
    }
);
