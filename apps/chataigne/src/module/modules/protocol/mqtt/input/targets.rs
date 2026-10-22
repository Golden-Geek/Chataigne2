use super::{ProcessTreeSnapshot, MAX_PATH};
use crate::app::module::common::received_values::service::VALUE_WORK_UNITS;
use golden_core::{
    edit::{Edit, EditPreconditions, EditRequest, NodeReadGuard},
    node::NodeId,
};
use std::collections::HashSet;

// Cursor path work and pruning share VALUE_WORK_UNITS. Pruning can add a common
// path above its individual removed children; native input bounds that path too.
const MAX_READS: usize = VALUE_WORK_UNITS + MAX_PATH + 2;
const READS_PER_CHECK: usize = 32;
const MAX_CHECKS: usize = 1 + MAX_READS.div_ceil(READS_PER_CHECK);

// Reserve queued target/source guards before handing an input to the actor.
// The extra read group covers the source's fixed read set. Token-vector spare
// capacity and queue wrappers remain charged even if the module is destroyed.
pub(super) const QUEUED_READ_BYTES: usize = (MAX_READS + READS_PER_CHECK) * size_of::<NodeReadGuard>()
    + MAX_CHECKS * (size_of::<EditRequest>() + size_of::<EditPreconditions>() + size_of::<[usize; 8]>());

pub(super) fn capture(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    requests: &[EditRequest],
) -> Result<Vec<EditPreconditions>, String> {
    if requests.len() > 2 * VALUE_WORK_UNITS {
        return Err("MQTT received-value prefix exceeds its edit budget".into());
    }
    let mut seen = HashSet::with_capacity(MAX_READS);
    seen.insert(root); // The source check already fences the Values root.
    let mut reads = Vec::with_capacity(MAX_READS);
    let mut path = Vec::with_capacity(MAX_PATH + 2);
    for request in requests {
        let mut target = match &request.edit {
            Edit::SetParam { node, .. } | Edit::ReplaceNode { node, .. } | Edit::RemoveNode { node } => *node,
            Edit::AddNodeTree { parent, .. } => *parent,
            Edit::EmitCustomEvent { .. } => continue,
            _ => return Err("MQTT received-value planner produced an unsupported edit".into()),
        };
        let mut depth = 0;
        while !seen.contains(&target) {
            if depth > MAX_PATH + 1 || reads.len() == MAX_READS {
                return Err("MQTT received-value target reads exceed their path/work budget".into());
            }
            let node = snapshot.node(target).ok_or("MQTT received-value target disappeared")?;
            reads.push(NodeReadGuard::capture_structure(snapshot, target).unwrap());
            path.push(target);
            target = node.parent.ok_or("MQTT received-value target is outside Values")?;
            depth += 1;
        }
        // Only reuse paths already proven to end at Values. A malformed cycle
        // cannot make a second visit look like a valid shared ancestor.
        seen.extend(path.drain(..));
    }
    Ok(reads
        .chunks(READS_PER_CHECK)
        .map(|chunk| EditPreconditions::new(chunk.to_vec(), Vec::new()).expect("bounded public target read group"))
        .collect())
}
