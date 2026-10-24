use super::*;
use crate::app::module_command::CapturedInvocation;
use golden_core::{
    engine::{EngineTime, NodeWaker},
    node::NodeUuid,
    process_ctx::SubtreeRevision,
    script::ScriptArguments,
};
use golden_io::{PayloadPool, RetainedPayload, RevisionGuard};
use std::sync::{Arc, OnceLock};

mod decode;
mod preparation;
mod service;
#[cfg(test)]
mod tests;
use preparation::Preparation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DmxCommandRequest {
    SetChannel { channel: u16, value: u8 },
    SetChannels { channel: u16, slots: Vec<u8> },
    SendFrame { slots: Vec<u8> },
    Blackout,
}

const WEIGHT: usize = 16 * 1024;
const LIMIT: usize = 64;
pub(super) type FrameRevision = (NodeId, SubtreeRevision);

#[derive(Clone, Debug, PartialEq, Eq)]
struct Key {
    uuid: NodeUuid,
    enabled: bool,
    configuration: [Option<FrameRevision>; 5],
}
impl Key {
    fn capture(snapshot: &ProcessTreeSnapshot, module: NodeId) -> Option<Self> {
        let node = snapshot.node(module)?;
        let connection = snapshot.find_child_by_decl_id(module, "connection")?;
        let parameters = snapshot.find_child_by_decl_id(module, "parameters")?;
        let mut configuration = [None; 5];
        for (index, (parent, name)) in [
            (connection, "network_interface"),
            (connection, "receive_enabled"),
            (connection, "listen_port"),
            (connection, "destination"),
            (parameters, "universe"),
        ]
        .into_iter()
        .enumerate()
        {
            configuration[index] = snapshot
                .find_child_by_decl_id(parent, name)
                .and_then(|id| snapshot.subtree_revision(id).map(|revision| (id, revision)));
        }
        Some(Self {
            uuid: node.uuid,
            enabled: node.enabled,
            configuration,
        })
    }
}

enum Input {
    Command(CapturedInvocation),
    Script {
        method: &'static str,
        args: ScriptArguments,
    },
    Property(Vec<u8>),
    Priority {
        value: u8,
        previous: u8,
    },
}
struct Source {
    input: Input,
    snapshot: RetainedPayload<ProcessTreeSnapshot>,
    producer: NodeWaker,
    session: RevisionGuard,
    module: NodeId,
    key: Key,
    time: EngineTime,
    output: FrameRevision,
    universe: u16,
    priority: u8,
}
impl Source {
    fn is_live(&self) -> bool {
        !self.producer.is_closed() && self.session.is_current()
    }
    fn rejection(&self, snapshot: &ProcessTreeSnapshot) -> Option<&'static str> {
        if self.producer.is_closed() {
            return Some("DMX producer retired; request was not sent");
        }
        if !self.session.is_current() {
            return Some("DMX session retired; request was not sent");
        }
        // Resolve the consumed paths, not the entire module subtree: command
        // tester authoring must not invalidate unrelated captured frame inputs.
        if Key::capture(snapshot, self.module).as_ref() != Some(&self.key) {
            return Some("DMX configuration changed; request was not sent");
        }
        None
    }
}

pub(super) struct Prepared {
    source: RetainedPayload<Source>,
    operation: Operation,
    baseline: Option<Vec<u8>>,
}
enum Operation {
    Frame(DmxCommandRequest),
    Priority(u8),
}
impl Prepared {
    pub(super) fn is_live(&self) -> bool {
        self.source.is_live()
    }
}
struct Pending {
    source: RetainedPayload<Source>,
    preparation: Option<Preparation>,
    ready: Option<RetainedPayload<Prepared>>,
}
#[derive(Default)]
pub(super) struct Outgoing {
    pending: VecDeque<Pending>,
    authored: Option<FrameRevision>,
    projection: Option<Vec<u8>>,
    reflect: Option<RetainedPayload<Prepared>>,
    retry_at: Option<Instant>,
}

fn pool(prepared: bool) -> Result<PayloadPool, String> {
    static SOURCE: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    static PREPARED: OnceLock<Result<PayloadPool, String>> = OnceLock::new();
    (if prepared { &PREPARED } else { &SOURCE })
        .get_or_init(|| {
            PayloadPool::new(
                if prepared { "dmx-prepared" } else { "dmx-invocations" },
                512,
                if prepared { 8 } else { 16 } * 1024 * 1024,
            )
            .map_err(|error| error.to_string())
        })
        .clone()
}

pub(crate) fn dispatch(ctx: &mut ProcessCtx, invocation: CapturedInvocation) -> Result<(), String> {
    let producer = ctx.node_waker().map_err(|error| error.to_string())?;
    let time = ctx.current_event().map_or(ctx.time, |event| event.time);
    ctx.call_node_mutation_without_snapshot(invocation.module, move |node, ctx| {
        let module = match node.get_type() {
            "artnet_module" => &mut node.as_any_mut().downcast_mut::<ArtNetModule>().unwrap().base,
            "sacn_module" => &mut node.as_any_mut().downcast_mut::<SacnModule>().unwrap().base,
            _ => return Err("DMX command target is unavailable".into()),
        };
        let snapshot = invocation.snapshot.clone();
        if let Err(error) = module.capture_output(ctx, Input::Command(invocation), snapshot, producer, time) {
            golden_core::logerror!(format!("DMX command was not admitted: {error}"));
            module.set_transport_warning(ctx, &error);
        }
        Ok(())
    });
    Ok(())
}
