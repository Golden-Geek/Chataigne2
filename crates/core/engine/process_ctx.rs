use crate::edit::{Edit, EditQueue};
use crate::engine::EngineTime;
use crate::events::{CustomEvent, Event, EventKind};
use crate::node::{Node, NodeId};
use crate::parameter::ParamValue;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPhase {
    EngineTick,
    EndOfTickStabilization,
}

pub struct ProcessCtx {
    pub phase: ExecutionPhase,
    pub inbox: Vec<Event>,
    pub edits: EditQueue,
    pub time: EngineTime,
}

impl ProcessCtx {
    pub fn new(phase: ExecutionPhase, time: EngineTime) -> Self {
        Self {
            phase,
            inbox: Vec::new(),
            edits: EditQueue::new(),
            time,
        }
    }

    pub fn set_param(&mut self, node: NodeId, value: ParamValue) {
        self.edits.push(Edit::SetParam { node, value });
    }

    pub fn add_child<N: Node + 'static>(&mut self, parent: NodeId, child: N, after: Option<NodeId>) {
        self.add_child_boxed(parent, Box::new(child), after);
    }

    pub fn add_child_boxed(&mut self, parent: NodeId, child: Box<dyn Node>, after: Option<NodeId>) {
        self.edits.push(Edit::AddNode {
            parent,
            prev_sibling: after,
            node: child,
        });
    }

    pub fn replace_node<N: Node + 'static>(&mut self, node: NodeId, new_node: N) {
        self.replace_node_boxed(node, Box::new(new_node));
    }

    pub fn replace_node_boxed(&mut self, node: NodeId, new_node: Box<dyn Node>) {
        self.edits.push(Edit::ReplaceNode {
            node,
            new_node,
        });
    }

    pub fn emit_custom_event(&mut self, event: CustomEvent) {
        self.inbox.push(Event { time: self.time, kind: EventKind::Custom(event) });
    }

    pub fn emit_custom_payload<U: Serialize>(&mut self, topic:  impl Into<String>, origin: Option<NodeId>, payload: &U) -> serde_json::Result<()> {
        let event = CustomEvent::from_payload(topic, origin, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }
}
