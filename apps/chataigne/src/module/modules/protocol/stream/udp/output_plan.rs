use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessTreeSnapshot, SubtreeRevision},
};

use super::{
    outbound::{OutputPlanBuilder, PreparedOutputPlan, UdpOutputTarget, UDP_HOST_BYTES},
    UDP_OUTPUT_NODE_TYPE,
};

pub(super) struct OutputPlanCache {
    root: NodeId,
    revision: SubtreeRevision,
    plan: PreparedOutputPlan,
}

pub(super) fn cached_plan(
    cache: &mut Option<OutputPlanCache>,
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
) -> Result<PreparedOutputPlan, String> {
    let revision = snapshot
        .subtree_revision(root)
        .ok_or("UDP output folder is unavailable")?;
    if let Some(cached) = cache {
        if cached.root == root && cached.revision == revision {
            return Ok(cached.plan.clone());
        }
    }
    // Admission failures can be transient (the shared payload pool may be full).
    // Cache only successful preparation so a later call can recover unchanged settings.
    *cache = None;
    let plan = prepare(snapshot, root)?;
    *cache = Some(OutputPlanCache {
        root,
        revision,
        plan: plan.clone(),
    });
    Ok(plan)
}

fn prepare(snapshot: &ProcessTreeSnapshot, root: NodeId) -> Result<PreparedOutputPlan, String> {
    let mut builder = OutputPlanBuilder::default();
    for output in OutputNodes::new(snapshot, root) {
        let output = output?;
        if !snapshot.node(output).is_some_and(|node| node.enabled) {
            continue;
        }
        let host = snapshot
            .find_child_by_decl_id(output, "remote_host")
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref());
        let Some(ParamValue::Str(host)) = host else {
            continue;
        };
        // Validate the borrowed value before trimming or copying arbitrarily large input.
        if host.len() > UDP_HOST_BYTES {
            return Err("UDP destination host exceeds the 1024-byte limit; command was not admitted".into());
        }
        if host.trim().is_empty() {
            continue;
        }
        let port =
            crate::app::module::common::streaming::module_helpers::child_int_param(snapshot, output, "remote_port")
                .and_then(|port| u16::try_from(port).ok());
        if let Some(remote_port) = port {
            builder.push_target(UdpOutputTarget {
                remote_host: host.clone(),
                remote_port,
            })?;
        }
    }
    builder.finish()
}

/// Constant-space traversal through folder links. Destination parameters are not traversed.
/// Cold preparation is still synchronous; the cache removes this scan from recurring sends.
pub(super) struct OutputNodes<'a> {
    snapshot: &'a ProcessTreeSnapshot,
    root: NodeId,
    next: Option<NodeId>,
    remaining: usize,
}

impl<'a> OutputNodes<'a> {
    pub(super) fn new(snapshot: &'a ProcessTreeSnapshot, root: NodeId) -> Self {
        Self {
            snapshot,
            root,
            next: snapshot.node(root).and_then(|node| node.first_child),
            remaining: snapshot.node_count().saturating_mul(2),
        }
    }

    fn advance(&mut self, mut id: NodeId) -> Result<Option<NodeId>, String> {
        loop {
            if self.remaining == 0 {
                return Err("UDP output tree traversal exceeded its node bound".into());
            }
            self.remaining -= 1;
            let node = self.snapshot.node(id).ok_or("UDP output tree has a missing node")?;
            if let Some(sibling) = node.next_sibling {
                return Ok(Some(sibling));
            }
            match node.parent {
                Some(parent) if parent != self.root => id = parent,
                Some(_) => return Ok(None),
                None => return Err("UDP output tree has a detached node".into()),
            }
        }
    }
}

impl Iterator for OutputNodes<'_> {
    type Item = Result<NodeId, String>;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(id) = self.next.take() {
            if self.remaining == 0 {
                return Some(Err("UDP output tree traversal exceeded its node bound".into()));
            }
            self.remaining -= 1;
            let Some(node) = self.snapshot.node(id) else {
                return Some(Err("UDP output tree has a missing node".into()));
            };
            if node.node_type == "folder" && node.first_child.is_some() {
                self.next = node.first_child;
            } else {
                match self.advance(id) {
                    Ok(next) => self.next = next,
                    Err(error) => return Some(Err(error)),
                }
            }
            if node.node_type == UDP_OUTPUT_NODE_TYPE {
                return Some(Ok(id));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
