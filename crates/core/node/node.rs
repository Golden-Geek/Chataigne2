use crate::engine::ProcessCtx;
use crate::schema::{NodeId, NodeTypeId};
use crate::parameter::ParameterData;

pub struct Node {
    pub id: NodeId,
    pub node_type: NodeTypeId,
    pub execution: NodeExecution,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
    pub meta: NodeMeta,
    pub data: NodeData,
    pub behaviour: Option<Box<dyn NodeBehaviour>>,
}

impl Node {

}

// To Contain other data, like parameters
pub enum NodeData {
    None,
    Parameter(ParameterData),
}

// To contain other meta, like tags
#[derive(Default)]
pub struct NodeMeta {
    pub nodes: std::collections::HashMap<NodeId, Node>,
}

// Handle for mutably accessing a node's data and behaviour
pub struct NodeHandleMut<'a> {
    pub id: NodeId,
    pub meta: &'a mut NodeMeta,
    pub data: &'a mut NodeData,
    pub behaviour: Option<&'a mut Box<dyn NodeBehaviour>>,
}

impl NodeHandleMut<'_> {
    pub fn get_children(&self) -> Vec<NodeId> {
        let mut children = Vec::new();
        let mut current = self
            .meta
            .nodes
            .get(&self.id)
            .and_then(|node| node.first_child);
        while let Some(child_id) = current {
            children.push(child_id);
            current = self
                .meta
                .nodes
                .get(&child_id)
                .and_then(|node| node.next_sibling);
        }
        children
    }

    // pub fn call_on_children(&self, ctx: &mut ProcessCtx, f: impl FnMut(&NodeHandleMut)) {}

    // pub fn create_child(
    //     &mut self,
    //     ctx: &mut ProcessCtx,
    //     node_type: NodeTypeId,
    //     execution: NodeExecution,
    // ) -> NodeId {
    //     let new_id = ctx.create_node(node_type, execution);
    //     ctx.add_child(self.id, new_id);
    //     new_id
    // }

    // pub fn add_child(&mut self, ctx: &mut ProcessCtx, child_id: NodeId) {
    //     ctx.add_child(self.id, child_id);
    // }

    // pub fn remove_child(&mut self, ctx: &mut ProcessCtx, child_id: NodeId) {
    //     ctx.remove_child(self.id, child_id);
    // }

    // pub fn move_child(&mut self, ctx: &mut ProcessCtx, child_id: NodeId, new_parent_id: NodeId) {
    //     ctx.move_child(child_id, new_parent_id);
    // }

    // pub fn replace_child(
    //     &mut self,
    //     ctx: &mut ProcessCtx,
    //     old_child_id: NodeId,
    //     new_child_id: NodeId,
    // ) {
    //     ctx.replace_child(self.id, old_child_id, new_child_id);
    // }

    // pub fn delete_child(&mut self, ctx: &mut ProcessCtx, child_id: NodeId) {
    //     ctx.remove_child(self.id, child_id);
    //     ctx.delete_node(child_id);
    // }
} // Handle for immutably accessing a node's data and behaviour pub struct NodeHandle<'a> { pub id: NodeId, pub meta: &'a NodeMeta, pub data: &'a NodeData, pub behaviour: Option<&'a Box<dyn NodeBehaviour>>,

// Execution mode of a node, which determines when its process function is called
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeExecution {
    Passive,
    Reactive,
    Continuous,
}

// Behaviour of a node, which determines how it reacts to events and updates
pub trait NodeBehaviour: Send {
    fn init(&mut self, _ctx: &mut ProcessCtx) {}
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {}
}

pub trait NodeReactive {
    fn process(&mut self, ctx: &mut ProcessCtx) {
        self.dispatch_inbox(ctx);
    }

    fn dispatch_inbox(&mut self, ctx: &mut ProcessCtx) {
        // let inbox_events = ctx.inbox.clone();
        // for event in inbox_events {
            //         match event.kind {
            //             core::schema::EventKind::ParamChanged { param, value } => {
            //                 self.on_param_change(ctx, param, value);
            //             }
            //             core::schema::EventKind::ChildAdded { parent, child } => {
            //                 self.on_child_added(ctx, parent, child);
            //             }
            //             core::schema::EventKind::ChildRemoved { parent, child } => {
            //                 self.on_child_removed(ctx, parent, child);
            //             }
            //             core::schema::EventKind::ChildReplaced { parent, old, new } => {
            //                 self.on_child_replaced(ctx, parent, old, new);
            //             }
            //             core::schema::EventKind::ChildMoved {
            //                 child,
            //                 old_parent,
            //                 new_parent,
            //             } => {
            //                 self.on_child_moved(ctx, child, old_parent, new_parent);
            //             }
            //             core::schema::EventKind::ChildReordered { parent, child } => {
            //                 self.on_child_reordered(ctx, parent, child);
            //             }
            //             core::schema::EventKind::NodeCreated { node } => {
            //                 self.on_node_created(ctx, node);
            //             }
            //             core::schema::EventKind::NodeDeleted { node } => {
            //                 self.on_node_deleted(ctx, node);
            //             }
            //             core::schema::EventKind::MetaChanged { node, patch } => {
            //                 self.on_meta_changed(ctx, node, patch);
            //             }
            //         }
            //     }
            // }

            // fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _value: Value) {}

            // fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}

            // fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}

            // fn on_child_replaced(
            //     &mut self,
            //     _ctx: &mut ProcessCtx,
            //     _parent: NodeId,
            //     _old: NodeId,
            //     _new: NodeId,
            // ) {
            // }

            // fn on_child_moved(
            //     &mut self,
            //     _ctx: &mut ProcessCtx,
            //     _child: NodeId,
            //     _old_parent: NodeId,
            //     _new_parent: NodeId,
            // ) {
            // }

            // fn on_child_reordered(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {}

            // fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}

            // fn on_node_deleted(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {}

            // fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {}
        // }
    }
}

pub trait NodeContinuous: NodeReactive {
    fn update(&mut self, ctx: &mut ProcessCtx);
}
