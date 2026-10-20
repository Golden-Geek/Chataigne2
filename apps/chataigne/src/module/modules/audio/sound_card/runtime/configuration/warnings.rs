use std::{collections::BTreeMap, mem::size_of};

use golden_core::{node::NodeId, process_ctx::ProcessTreeSnapshot};

use super::super::RuntimeWarning;

pub(crate) mod delivery;

const MAX_ACTIONS: usize = 262_144;

pub(crate) enum WarningPatch {
    Set(RuntimeWarning),
    Clear { node: NodeId, id: &'static str },
}

pub(crate) struct WarningPlan {
    pub(crate) patches: Vec<WarningPatch>,
    pub(crate) bytes: usize,
}

fn owned_id(id: &str) -> Option<&'static str> {
    match id {
        "sound-card-unresolved-channel" => Some("sound-card-unresolved-channel"),
        "sound-card-unresolved-physical-channel" => Some("sound-card-unresolved-physical-channel"),
        _ => None,
    }
}

impl WarningPlan {
    /// Compare against authored engine state on the preparing worker. There is
    /// no actor-side optimistic cache to lose a failed edit or stale warning text.
    pub(super) fn prepare(snapshot: &ProcessTreeSnapshot, module: NodeId, warnings: &[RuntimeWarning]) -> Option<Self> {
        if warnings.len() > MAX_ACTIONS {
            return None;
        }
        let mut desired = warnings
            .iter()
            .map(|warning| ((warning.node.0, warning.id.as_str()), warning))
            .collect::<BTreeMap<_, _>>();
        let mut patches = Vec::new();
        let mut pending = vec![module];
        let mut inspected = 0usize;
        while let Some(node) = pending.pop() {
            let state = snapshot.node(node)?;
            inspected = inspected.checked_add(1 + state.presentation.warnings.len())?;
            if inspected > MAX_ACTIONS {
                return None;
            }
            for existing in &state.presentation.warnings {
                let Some(id) = owned_id(&existing.id) else { continue };
                match desired.remove(&(node.0, id)) {
                    Some(warning) if existing.message != warning.message || existing.detail != warning.detail => {
                        patches.push(WarningPatch::Set(warning.clone()));
                    }
                    Some(_) => {}
                    None => patches.push(WarningPatch::Clear { node, id }),
                }
            }
            let children = snapshot.child_ids_slice(node);
            if inspected + pending.len() + children.len() > MAX_ACTIONS {
                return None;
            }
            pending.extend_from_slice(children);
        }
        patches.extend(desired.into_values().cloned().map(WarningPatch::Set));
        if patches.len() > MAX_ACTIONS {
            return None;
        }
        let mut bytes =
            size_of::<Self>() + 2 * size_of::<usize>() + patches.capacity().checked_mul(size_of::<WarningPatch>())?;
        for patch in &patches {
            if let WarningPatch::Set(warning) = patch {
                bytes = bytes
                    .checked_add(warning.id.capacity())?
                    .checked_add(warning.message.capacity())?
                    .checked_add(warning.detail.as_ref().map_or(0, String::capacity))?;
            }
        }
        Some(Self { patches, bytes })
    }
}

impl std::fmt::Debug for WarningPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WarningPlan")
            .field("patches", &self.patches.len())
            .field("bytes", &self.bytes)
            .finish()
    }
}

#[cfg(test)]
mod tests;
