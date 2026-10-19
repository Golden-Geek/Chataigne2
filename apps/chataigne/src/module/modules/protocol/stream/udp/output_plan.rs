use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision},
};

use super::{
    outbound::{OutputPlanBuilder, PreparedOutputPlan, UdpOutputTarget, UDP_HOST_BYTES},
    UDP_OUTPUT_NODE_TYPE,
};

use super::outbound::{UdpOutboundSend, UDP_PLAN_BYTES};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Debug)]
pub(super) struct PlanFailure {
    pub message: String,
    pub retryable: bool,
}

impl PlanFailure {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }
}

#[derive(Debug)]
struct PlanState {
    result: OnceLock<Result<PreparedOutputPlan, PlanFailure>>,
    limit: usize,
}

pub(super) const TICKET_HEAP_BYTES: usize = std::mem::size_of::<PlanState>() + 2 * std::mem::size_of::<usize>();

#[derive(Clone, Debug)]
pub(super) struct PlanTicket(Arc<PlanState>);

impl PlanTicket {
    fn pending(limit: usize) -> Self {
        Self(Arc::new(PlanState {
            result: OnceLock::new(),
            limit,
        }))
    }
    #[cfg(test)]
    pub(super) fn ready(plan: PreparedOutputPlan) -> Self {
        let ticket = Self::pending(UDP_PLAN_BYTES);
        ticket.complete(Ok(plan));
        ticket
    }
    pub(super) fn result(&self) -> Option<&Result<PreparedOutputPlan, PlanFailure>> {
        self.0.result.get()
    }
    pub(super) fn limit(&self) -> usize {
        self.0.limit
    }
    pub(super) fn complete(&self, result: Result<PreparedOutputPlan, PlanFailure>) {
        let _ = self.0.result.set(result);
    }
    pub(super) fn fail(&self, message: String) {
        self.complete(Err(PlanFailure::unavailable(message)));
    }
}

#[derive(Clone)]
pub(super) struct OutputPlanCache {
    root: NodeId,
    revision: SubtreeRevision,
    pub(super) ticket: PlanTicket,
}

/// Build a candidate only. The module publishes it to its cache after the single native
/// send admission succeeds, so a rejected command cannot strand a pending plan ticket.
pub(super) fn command(
    cache: &mut Option<OutputPlanCache>,
    snapshot: &Arc<ProcessTreeSnapshot>,
    root: NodeId,
    bytes: Vec<u8>,
    log: bool,
) -> Result<(UdpOutboundSend, OutputPlanCache), String> {
    let revision = snapshot
        .subtree_revision(root)
        .ok_or("UDP output folder is unavailable")?;
    if let Some(cached) = cache {
        let retry = matches!(cached.ticket.result(), Some(Err(error)) if error.retryable);
        if cached.root == root && cached.revision == revision && !retry {
            let command = UdpOutboundSend::with_ticket(bytes, cached.ticket.clone(), None, log)?;
            return Ok((command, cached.clone()));
        }
    }
    *cache = None;
    // The worker enforces this upper bound before sealing the model. Small views need not
    // reserve a whole maximum plan; large graphs still receive the explicit byte limit.
    let ticket = PlanTicket::pending(snapshot.retained_bytes().clamp(4096, UDP_PLAN_BYTES));
    let command = UdpOutboundSend::with_ticket(bytes, ticket.clone(), Some((snapshot, root)), log)?;
    Ok((command, OutputPlanCache { root, revision, ticket }))
}

pub(super) const PREPARATION_EDGES_PER_TURN: usize = 64;

#[derive(Debug)]
enum Walk {
    Visit(NodeId),
    Advance(NodeId),
    Done,
}

#[derive(Debug)]
struct OutputCursor {
    root: NodeId,
    state: Walk,
    remaining: usize,
}

impl OutputCursor {
    fn new(snapshot: &ProcessTreeSnapshot, root: NodeId) -> Self {
        let state = snapshot.node(root).and_then(|node| node.first_child).map_or(Walk::Done, Walk::Visit);
        Self { root, state, remaining: snapshot.node_count().saturating_mul(2) }
    }

    // One call visits or advances one edge, including deep ancestor unwinding.
    fn step(&mut self, snapshot: &ProcessTreeSnapshot) -> Result<Option<Option<NodeId>>, String> {
        if matches!(self.state, Walk::Done) { return Ok(None); }
        if self.remaining == 0 { return Err("UDP output traversal exceeded its node bound".into()); }
        self.remaining -= 1;
        match self.state {
            Walk::Visit(id) => {
                let node = snapshot.node(id).ok_or("UDP output tree has a missing node")?;
                self.state = if node.node_type == "folder" {
                    node.first_child.map_or(Walk::Advance(id), Walk::Visit)
                } else { Walk::Advance(id) };
                Ok(Some((node.node_type == UDP_OUTPUT_NODE_TYPE).then_some(id)))
            }
            Walk::Advance(id) => {
                let node = snapshot.node(id).ok_or("UDP output tree has a missing node")?;
                self.state = if let Some(sibling) = node.next_sibling { Walk::Visit(sibling) }
                    else { match node.parent {
                        Some(parent) if parent != self.root => Walk::Advance(parent),
                        Some(_) => Walk::Done,
                        None => return Err("UDP output tree has a detached node".into()),
                    } };
                Ok(Some(None))
            }
            Walk::Done => unreachable!(),
        }
    }
}

/// Only empty, fixed-size state is created by the producer. Once admitted, this object
/// and every mutable builder allocation are owned by the native worker until completion.
#[derive(Debug)]
pub(super) struct OutputPreparation {
    snapshot: golden_io::RetainedPayload<ProcessTreeSnapshot>,
    cursor: OutputCursor,
    builder: Option<OutputPlanBuilder>,
}

impl OutputPreparation {
    pub(super) fn new(snapshot: golden_io::RetainedPayload<ProcessTreeSnapshot>, root: NodeId, limit: usize) -> Self {
        let cursor = OutputCursor::new(&snapshot, root);
        Self { snapshot, cursor, builder: Some(OutputPlanBuilder::with_limit(limit)) }
    }

    pub(super) fn service(&mut self, cancelled: &impl Fn() -> bool) -> Option<Result<PreparedOutputPlan, PlanFailure>> {
        for _ in 0..PREPARATION_EDGES_PER_TURN {
            if cancelled() { return Some(Err(PlanFailure::unavailable("UDP output preparation cancelled"))); }
            match self.cursor.step(&self.snapshot) {
                Err(error) => return Some(Err(PlanFailure::invalid(error))),
                Ok(None) => return Some(self.builder.take().expect("active output builder").finish().map_err(PlanFailure::unavailable)),
                Ok(Some(None)) => {},
                Ok(Some(Some(output))) => {
                    if let Err(error) = append_target(&self.snapshot, output, self.builder.as_mut().expect("active output builder")) {
                        return Some(Err(error));
                    }
                }
            }
        }
        None
    }
}

fn append_target(snapshot: &ProcessTreeSnapshot, output: NodeId, builder: &mut OutputPlanBuilder) -> Result<(), PlanFailure> {
    if !snapshot.node(output).is_some_and(|node| node.enabled) { return Ok(()); }
    let host = snapshot.find_child_by_decl_id(output, "remote_host")
        .and_then(|id| snapshot.node(id)).and_then(|node| node.param_value.as_ref());
    let Some(ParamValue::Str(host)) = host else { return Ok(()); };
    if host.len() > UDP_HOST_BYTES {
        return Err(PlanFailure::invalid(format!("UDP output {output:?} host exceeds the 1024-byte limit; no output was sent")));
    }
    if host.trim().is_empty() { return Ok(()); }
    let port = crate::app::module::common::streaming::module_helpers::child_int_param(snapshot, output, "remote_port")
        .and_then(|port| u16::try_from(port).ok());
    if let Some(remote_port) = port {
        builder.push_target(UdpOutputTarget { remote_host: host.clone(), remote_port }).map_err(PlanFailure::invalid)?;
    }
    Ok(())
}

#[cfg(test)]
fn prepare(snapshot: &ProcessTreeSnapshot, root: NodeId, limit: usize, cancelled: impl Fn() -> bool) -> Result<PreparedOutputPlan, PlanFailure> {
    let snapshot = Arc::new(snapshot.clone());
    let lease = snapshot.try_lease().map_err(|error| PlanFailure::unavailable(error.to_string()))?;
    let mut preparation = OutputPreparation::new(lease, root, limit);
    loop { if let Some(result) = preparation.service(&cancelled) { return result; } }
}

#[cfg(test)]
mod tests;
