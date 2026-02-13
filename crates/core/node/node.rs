use std::any::Any;

use crate::color::Color;
use crate::edit::Edit;
use crate::events::{CustomEvent, EventKind};
use crate::parameter::ParamValue;
use crate::process_ctx::ProcessCtx;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeUuid(pub Uuid);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeclId(pub String);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SemanticsHint {
    pub intent: Option<String>,
    pub unit: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PresentationHint {
    pub color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    pub id: NodeId,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub meta: NodeMeta,
}

impl NodeData {
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

// To contain other meta, like tags
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeMeta {
    pub uuid: NodeUuid,
    pub decl_id: DeclId,
    pub short_name: String,
    pub enabled: bool,
    pub can_be_disabled: bool,
    pub label: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub semantics: SemanticsHint,
    pub presentation: PresentationHint,
}

impl NodeMeta {
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

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_semantics(mut self, semantics: SemanticsHint) -> Self {
        self.semantics = semantics;
        self
    }

    pub fn with_presentation(mut self, presentation: PresentationHint) -> Self {
        self.presentation = presentation;
        self
    }

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

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeMetaPatch {
    // For now, we can just replace the whole meta of a node. In the future, we can add more fine-grained patches. pub new_meta: NodeMeta, }
}

// Behaviour of a node, which determines how it reacts to events and updates
pub trait Node: Send + Any {
    fn node_data(&self) -> &NodeData;
    fn node_data_mut(&mut self) -> &mut NodeData;

    fn get_type(&self) -> &str;

    fn set_param_value(&mut self, _value: ParamValue) -> Option<ParamValue> {
        None
    }

    fn from_boxed_node(node: Box<dyn Node>) -> Option<Self>
    where
        Self: Sized,
    {
        let any: Box<dyn Any> = node;
        any.downcast::<Self>().ok().map(|node| *node)
    }

    fn id(&self) -> NodeId {
        self.node_data().id
    }

    fn is(&self, id: NodeId) -> bool {
        self.id() == id
    }

    // Lifecycle methods
    fn init(&mut self, _ctx: &mut ProcessCtx) {}
    fn update(&mut self, _ctx: &mut ProcessCtx) {} // called at this node's desired update rate
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {}
    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
    }

    fn add_child_boxed(&mut self, ctx: &mut ProcessCtx, child: Box<dyn Node>, after: Option<NodeId>) {
        ctx.add_child_boxed(self.id(), child, after);
    }

    fn add_child<N>(&mut self, ctx: &mut ProcessCtx, child: N, after: Option<NodeId>)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.add_child_boxed(ctx, Box::new(child), after);
    }
    fn remove_child(&mut self, ctx: &mut ProcessCtx, child: NodeId) {
        ctx.edits.push(Edit::RemoveNode { node: child });
    }
    fn move_child(&mut self, ctx: &mut ProcessCtx, child: NodeId, new_parent: NodeId, after: Option<NodeId>) {
        ctx.edits.push(Edit::MoveNode {
            node: child,
            new_parent,
            new_prev_sibling: after,
        });
    }
    fn replace_child_boxed(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: Box<dyn Node>) {
        ctx.replace_node_boxed(old, new_node);
    }
    fn replace_child<N>(&mut self, ctx: &mut ProcessCtx, old: NodeId, new_node: N)
    where
        Self: Sized,
        N: Node + 'static,
    {
        self.replace_child_boxed(ctx, old, Box::new(new_node));
    }

    // DEFAULT IMPLEMENTATIONS FOR EVENT HANDLERS

    // Dispatch events from the inbox to the appropriate handlers
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

    // Default handlers for events, can be overridden by nodes to implement custom behaviour

    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {}
    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_child_replaced(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {}
    fn on_child_moved(&mut self, _ctx: &mut ProcessCtx, _child: NodeId, _old_parent: NodeId, _new_parent: NodeId) {}
    fn on_child_reordered(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}
    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}
    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {}
    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {}
}

//Implement default node, which is a basic container / folder
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Container {
    node_data: NodeData,
}

impl Container {
    pub fn new(label: String) -> Self {
        Self { node_data: NodeData::new(label) }
    }
}

impl Node for Container {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "container"
    }
}

// Manager is an internal container-like node used as a curated user-extensible root.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manager {
    node_data: NodeData,
}

impl Manager {
    pub fn new(label: String) -> Self {
        Self { node_data: NodeData::new(label) }
    }
}

impl Node for Manager {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "manager"
    }
}
