use crate::schema::{NodeId};
use crate::parameter::{ParamValue};

pub struct Inbox
{
    pub events: Vec<Event>,
}

pub struct Event {
    pub time: EventTime,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventTime {
    /// Monotonic engine tick counter. Increments only on EngineTick.
    pub tick: u64,

    /// Micro-step index within the same tick.
    /// 0 = main tick pass, 1.. = stabilisation rounds or flushImmediate rounds within that same tick.
    pub micro: u32,

    /// Total ordering within the same (tick, micro).
    pub seq: u32,
}

pub enum EventKind
{
    ParamChanged { param: NodeId, old_value: ParamValue },

    // Structure-level
    ChildAdded     { parent: NodeId, child: NodeId },
    ChildRemoved   { parent: NodeId, child: NodeId },
    ChildReplaced  { parent: NodeId, old: NodeId, new: NodeId },
    ChildMoved     { child: NodeId, old_parent: NodeId, new_parent: NodeId },
    ChildReordered { parent: NodeId, child: NodeId },

    // Identity/lifecycle-level
    NodeCreated { node: NodeId },
    NodeDeleted { node: NodeId },

    // Metadata-level
    MetaChanged { node: NodeId },//, patch: NodeMetaPatch },
}

