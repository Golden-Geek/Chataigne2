use crate::edit::{Edit, EditOrigin, EditQueue, Propagation};
use crate::engine::EngineTime;
use crate::events::{CustomEvent, Event, EventKind};
use crate::node::NodeId;
use crate::parameter::ParamValue;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionPhase {
    EngineTick,
    EndOfTickStabilization,
    FlushImmediate,
}

pub struct ProcessCtx {
    pub phase: ExecutionPhase,
    pub inbox: Vec<Event>,
    pub edits: EditQueue,
    pub time: EngineTime,
}

impl ProcessCtx {
    pub fn set_param(&mut self, node: NodeId, value: ParamValue) {
        self.set_param_with(node, value, Propagation::EndOfTick);
    }

    pub fn set_param_with(&mut self, node: NodeId, value: ParamValue, propagation: Propagation) {
        self.edits.push(Edit::SetParam { node, value }, propagation, EditOrigin::Internal);
    }

    pub fn set_param_immediate(&mut self, node: NodeId, value: ParamValue) {
        self.set_param_with(node, value, Propagation::Immediate);
    }

    pub fn set_param_next_tick(&mut self, node: NodeId, value: ParamValue) {
        self.set_param_with(node, value, Propagation::NextTick);
    }

    pub fn emit_custom_event(&mut self, event: CustomEvent) {
        self.inbox.push(Event { time: self.time, kind: EventKind::Custom(event) });
    }

    pub fn emit_custom_payload<U: Serialize>(&mut self, topic: impl Into<String>, payload: &U) -> serde_json::Result<()> {
        let event = CustomEvent::from_payload(topic, payload)?;
        self.emit_custom_event(event);
        Ok(())
    }
}
