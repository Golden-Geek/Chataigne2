use super::*;
use service::{Position, Repair};

pub(super) fn direction_root(
    source: &ProcessTreeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    direction: &str,
    pos: &mut Position,
) -> Option<Repair> {
    let Some(parent) = find_path(snapshot, module, "values/levels") else {
        advance(pos);
        return None;
    };
    let namespace = snapshot.node(module)?.uuid;
    let uuid = derived_uuid(namespace, format!("{direction}-values").as_bytes());
    let expected = if direction == "input" {
        SoundCardInputValues::NODE_TYPE
    } else {
        SoundCardOutputValues::NODE_TYPE
    };
    let used = direction_used(snapshot, module, direction);
    // Include the older deterministic root even if it still lives under Values.
    let source_parent = find_path(source, module, "values/levels");
    let children = source_parent.map(|p| source.child_ids_slice(p)).unwrap_or_default();
    let candidate = children.get(pos.row).copied().or_else(|| {
        (pos.row == children.len())
            .then(|| source.node_id_by_uuid(uuid))
            .flatten()
            .filter(|id| !source_parent.is_some_and(|p| source.node(*id).is_some_and(|n| n.parent == Some(p))))
    });
    if pos.row <= children.len() {
        pos.row += 1;
        let state = candidate.and_then(|id| snapshot.node(id)).filter(|n| {
            source
                .node(n.id)
                .is_some_and(|old| old.uuid == n.uuid && old.parent == n.parent)
        });
        if let Some(state) = state.filter(|n| n.uuid == uuid || n.decl_id == direction) {
            if used && pos.retained.is_none() && state.node_type == expected {
                pos.retained = Some(state.id);
            } else {
                return Some(Repair::new(
                    vec![Edit::RemoveNode { node: state.id }],
                    vec![state.id, parent],
                ));
            }
        }
        return None;
    }
    let retained = pos
        .retained
        .and_then(|id| snapshot.node(id))
        .filter(|n| n.node_type == expected && (n.uuid == uuid || n.decl_id == direction))
        .or_else(|| {
            snapshot
                .node_id_by_uuid(uuid)
                .and_then(|id| snapshot.node(id))
                .filter(|n| n.node_type == expected)
        });
    if used && retained.is_none() && snapshot.node_id_by_uuid(uuid).is_some() {
        // A prior row may have queued that obsolete identity's removal, or an
        // external move changed its scan position. Revisit from a fresh source.
        pos.stage = Stage::Done;
        return None;
    }
    let edits = if !used {
        Vec::new()
    } else if let Some(state) = retained {
        if state.parent != Some(parent) {
            vec![Edit::MoveNode {
                node: state.id,
                new_parent: parent,
                new_prev_sibling: None,
            }]
        } else {
            Vec::new()
        }
    } else {
        let tree = if direction == "input" {
            let mut node = SoundCardInputValues::new();
            set_derived_identity(&mut node, namespace, "input-values", "Input", "input");
            NodeTree::new(node)
        } else {
            let mut node = SoundCardOutputValues::new();
            set_derived_identity(&mut node, namespace, "output-values", "Output", "output");
            NodeTree::new(node)
        };
        vec![Edit::AddNodeTree {
            parent,
            tree,
            prev_sibling: None,
        }]
    };
    let mut reads = vec![parent];
    if let Some(state) = retained {
        reads.push(state.id);
    }
    advance(pos);
    (!edits.is_empty()).then(|| Repair::new(edits, reads))
}

pub(super) fn spectral(
    source: &ProcessTreeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    pos: &mut Position,
) -> Option<Repair> {
    let Some(parent) = find_path(snapshot, module, "parameters/processing") else {
        advance(pos);
        return None;
    };
    let namespace = snapshot.node(module)?.uuid;
    let uuid = derived_uuid(namespace, SPECTRAL_PARAMETERS_UUID_KEY);
    let retained = snapshot.node_id_by_uuid(uuid).and_then(|id| snapshot.node(id));
    if retained.is_none_or(|n| n.node_type != "folder") {
        let mut container = Folder::new("Spectral Analysis");
        let data = container.node_data_mut();
        data.meta.uuid = uuid;
        data.meta.decl_id = DeclId("parameters/processing/spectral_analysis".into());
        data.meta.short_name = "spectral_analysis".into();
        data.meta.enabled = retained
            .and_then(|n| n.param_value.as_ref())
            .and_then(ParamValue::as_bool)
            .unwrap_or(false);
        data.meta.can_be_disabled = true;
        data.meta.user_permissions = NodeUserPermissions::none();
        let mut reads = vec![parent];
        let edit = if let Some(state) = retained {
            reads.push(state.id);
            // Replace before pruning: the legacy bool's enabled intent survives a failed
            // suffix, and the deterministic identity never has an absent interval.
            Edit::ReplaceNode {
                node: state.id,
                new_node: Box::new(container),
            }
        } else {
            Edit::AddNodeTree {
                parent,
                tree: NodeTree::new(container),
                prev_sibling: find_child_by_key(snapshot, parent, "pitch_detection"),
            }
        };
        return Some(Repair {
            edits: vec![edit],
            reads,
            revisit: true,
        });
    }
    let retained = retained.unwrap();
    let children = find_path(source, module, "parameters/processing")
        .map(|p| source.child_ids_slice(p))
        .unwrap_or_default();
    if let Some(&id) = children.get(pos.row) {
        pos.row += 1;
        let state = snapshot
            .node(id)
            .filter(|n| n.parent == Some(parent) && source.node(id).is_some_and(|old| old.uuid == n.uuid));
        if let Some(state) = state.filter(|n| {
            n.id != retained.id
                && (n.decl_id.rsplit('/').next() == Some("spectral_analysis") || n.label == "Spectral Analysis")
        }) {
            let enabled = if state.node_type == "folder" {
                state.enabled
            } else {
                state
                    .param_value
                    .as_ref()
                    .and_then(ParamValue::as_bool)
                    .unwrap_or(false)
            };
            let mut edits = Vec::with_capacity(2);
            // Merge each legacy enabled intent before removing its only copy. Each row
            // fences both owners; no scan-wide aggregate can become stale between turns.
            if enabled && !retained.enabled {
                edits.push(Edit::PatchMeta {
                    node: retained.id,
                    patch: NodeMetaPatch {
                        enabled: Some(true),
                        ..NodeMetaPatch::default()
                    },
                });
            }
            edits.push(Edit::RemoveNode { node: id });
            return Some(Repair::new(edits, vec![parent, retained.id, id]));
        }
        return None;
    }
    advance(pos);
    if retained.label == "Spectral Analysis" && retained.short_name == "spectral_analysis" && retained.can_be_disabled {
        return None;
    }
    Some(Repair::new(
        vec![Edit::PatchMeta {
            node: retained.id,
            patch: NodeMetaPatch {
                label: (retained.label != "Spectral Analysis").then(|| "Spectral Analysis".into()),
                short_name: (retained.short_name != "spectral_analysis").then(|| "spectral_analysis".into()),
                can_be_disabled: (!retained.can_be_disabled).then_some(true),
                ..NodeMetaPatch::default()
            },
        }],
        vec![parent, retained.id],
    ))
}

pub(super) fn optional(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    pos: &mut Position,
    spectral: bool,
) -> Option<Repair> {
    let path = if spectral {
        "values/spectral_analysis"
    } else {
        "values/pitch_detection"
    };
    let enabled = if spectral {
        spectral_analysis_enabled(snapshot, module)
    } else {
        find_path(snapshot, module, "parameters/processing/pitch_detection")
            .and_then(|id| snapshot.node(id))
            .and_then(|n| n.param_value.as_ref())
            .and_then(ParamValue::as_bool)
            .unwrap_or(false)
    };
    let existing = find_path(snapshot, module, path);
    advance(pos);
    if !enabled {
        return existing.map(|node| Repair::new(vec![Edit::RemoveNode { node }], vec![node]));
    }
    if existing.is_some() {
        return None;
    }
    let parent = find_path(snapshot, module, "values")?;
    let namespace = snapshot.node(module)?.uuid;
    let tree = if spectral {
        let mut node = SoundCardSpectralValues::new();
        set_derived_identity(
            &mut node,
            namespace,
            "spectral-values",
            "Spectral Analysis",
            "spectral_analysis",
        );
        NodeTree::new(node)
    } else {
        let mut node = SoundCardPitchValues::new();
        set_derived_identity(
            &mut node,
            namespace,
            "pitch-values",
            "Pitch Detection",
            "pitch_detection",
        );
        NodeTree::new(node)
    };
    Some(Repair::new(
        vec![Edit::AddNodeTree {
            parent,
            tree,
            prev_sibling: None,
        }],
        vec![parent],
    ))
}

pub(super) fn cleanup(
    source: &ProcessTreeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    pos: &mut Position,
    kind: usize,
) -> Option<Repair> {
    let path = match kind {
        0 => "values/spectral_analysis",
        1 => "values",
        _ => "parameters",
    };
    if kind == 0 && !spectral_analysis_enabled(snapshot, module) {
        advance(pos);
        return None;
    }
    let state = scan_child(source, snapshot, module, path, pos)?;
    let remove = match kind {
        0 => true,
        1 => state.decl_id.rsplit('/').next().is_some_and(|decl| {
            [
                "input_levels",
                "output_levels",
                "global_levels",
                "pitch_results",
                "spectrum_bands",
                "diagnostics",
            ]
            .contains(&decl)
        }),
        _ => {
            state.node_type == "folder"
                && state.label == "Folder"
                && state.child_count == 0
                && !matches!(state.decl_id.as_str(), "input" | "output" | "processing")
        }
    };
    remove.then(|| {
        Repair::new(
            vec![Edit::RemoveNode { node: state.id }],
            vec![state.id, state.parent.unwrap()],
        )
    })
}

pub(super) fn order(snapshot: &ProcessTreeSnapshot, module: NodeId, pos: &mut Position) -> Option<Repair> {
    advance(pos);
    let parent = find_path(snapshot, module, "values/levels")?;
    let input = find_child_by_key(snapshot, parent, "input")?;
    if snapshot.child_at(parent, 0) == Some(input) {
        return None;
    }
    Some(Repair::new(
        vec![Edit::MoveNode {
            node: input,
            new_parent: parent,
            new_prev_sibling: None,
        }],
        vec![parent, input],
    ))
}
