use super::*;
use golden_core::{
    edit::{EditCompletion, EditCompletionStatus, EditPreconditions, EditQueue, NodeReadGuard, NodeTree},
    node::NodeUuid,
};

const WARNING: &str = "midi_cc_structure";
#[derive(Clone, Copy)]
pub(super) struct Pending {
    pub previous: bool,
    pub restore: bool,
}
#[derive(Default)]
pub(super) struct CcWork {
    pub pending: [Option<Pending>; 16],
    active: Option<Active>,
}
struct Active {
    channel: u8,
    folder: NodeId,
    uuid: NodeUuid,
    states: [Cc14BitState; 32],
    ordering: bool,
    receipt: EditCompletion,
    fence: EditCompletion,
    request: Pending,
    target: bool,
    revision: golden_core::process_ctx::SubtreeRevision,
    launch: Option<EditCompletion>,
}
impl CcWork {
    pub fn busy(&self) -> bool {
        self.active.is_some() || self.pending.iter().any(Option::is_some)
    }
    pub fn requires_snapshot(&self) -> bool {
        self.active.as_ref().map_or_else(
            || self.pending.iter().any(Option::is_some),
            |a| {
                !a.ordering
                    && a.receipt.status() != EditCompletionStatus::Pending
                    && (a.receipt.status() == EditCompletionStatus::Failed
                        || a.launch
                            .as_ref()
                            .is_none_or(|r| r.status() != EditCompletionStatus::Pending))
            },
        )
    }
}
struct Plan {
    folder: NodeId,
    uuid: NodeUuid,
    states: [Cc14BitState; 32],
    edits: EditQueue,
    fence: EditCompletion,
}
impl MidiModule {
    pub(super) fn service_cc_structure(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>) {
        self.advance_cc_structure(ctx, snapshot, true);
    }
    fn advance_cc_structure(&mut self, ctx: &mut ProcessCtx, snapshot: Option<&ProcessTreeSnapshot>, start: bool) {
        if let Some(mut active) = self.structure_work.cc.active.take() {
            match active.receipt.status() {
                EditCompletionStatus::Pending => self.structure_work.cc.active = Some(active),
                EditCompletionStatus::Failed => {
                    if !active.ordering && snapshot.is_none() {
                        self.structure_work.cc.active = Some(active);
                        return;
                    }
                    let index = usize::from(active.channel - 1);
                    let changed = !active.ordering
                        && active.fence.status() == EditCompletionStatus::Applied
                        && snapshot.is_some_and(|s| {
                            s.node(active.folder).is_some_and(|n| n.uuid == active.uuid)
                                && s.subtree_revision(active.folder) != Some(active.revision)
                        });
                    if changed {
                        // Every scalar range/value update is one node mutation.
                        // Rebuilding this latest-state plan skips completed widths
                        // and cannot apply a relative conversion twice.
                        self.structure_work.cc.pending[index] = Some(Pending {
                            previous: !self.cc_channel_is_14_bit_handle(active.channel),
                            restore: false,
                        });
                    } else if !active.ordering {
                        if let Some(next) = self.structure_work.cc.pending[index].as_mut() {
                            next.previous = active.request.previous;
                        } else if active.request.restore
                            && self.cc_channel_is_14_bit_handle(active.channel) == active.target
                        {
                            if let Some(toggle) = self.channel_toggle_id(active.channel) {
                                self.set_internal_param(ctx, toggle, ParamValue::Bool(active.request.previous));
                            }
                        }
                    }
                    ctx.set_node_warning_with(
                        self.id(),
                        Some(WARNING),
                        "MIDI CC structure edits failed; applied state retained and remaining widths rechecked",
                        None,
                    );
                }
                EditCompletionStatus::Applied if active.ordering => ctx.clear_node_warning(self.id(), Some(WARNING)),
                EditCompletionStatus::Applied => {
                    if active
                        .launch
                        .as_ref()
                        .is_some_and(|r| r.status() == EditCompletionStatus::Pending)
                    {
                        self.structure_work.cc.active = Some(active);
                        return;
                    }
                    self.cc_14_bit_state[usize::from(active.channel - 1)] = active.states;
                    let Some(snapshot) = snapshot else {
                        self.structure_work.cc.active = Some(active);
                        return;
                    };
                    match order(snapshot, active.folder, active.uuid) {
                        Err(error) => ctx.set_node_warning_with(self.id(), Some(WARNING), error, None),
                        Ok(mut edits) if !edits.pending.is_empty() => {
                            active.receipt = edits.completion();
                            active.ordering = true;
                            ctx.edits.pending.extend(edits.drain());
                            self.structure_work.cc.active = Some(active);
                        }
                        Ok(_) => ctx.clear_node_warning(self.id(), Some(WARNING)),
                    }
                }
            }
            if self.structure_work.cc.active.is_some() {
                return;
            }
        }
        if !start {
            return;
        }
        let Some(snapshot) = snapshot else {
            return;
        };
        let Some(index) = self.structure_work.cc.pending.iter().position(Option::is_some) else {
            return;
        };
        let previous = self.structure_work.cc.pending[index].take().expect("pending width");
        let channel = (index + 1) as u8;
        let target = self.cc_channel_is_14_bit_handle(channel);
        if previous.previous == target {
            return;
        }
        match plan(self, snapshot, channel, target) {
            Ok(Some(mut plan)) => {
                let receipt = plan.edits.completion();
                ctx.edits.pending.extend(plan.edits.drain());
                self.structure_work.cc.active = Some(Active {
                    channel,
                    folder: plan.folder,
                    uuid: plan.uuid,
                    states: plan.states,
                    ordering: false,
                    receipt,
                    fence: plan.fence,
                    request: previous,
                    target,
                    revision: snapshot.subtree_revision(plan.folder).expect("planned folder"),
                    launch: None,
                });
                let generation = self.structure_work.generation;
                let source = ctx.node_waker().ok();
                let owner = self.id();
                let mut ordering = EditQueue::new();
                ordering.push(Edit::CallNodeMutation {
                    node: owner,
                    needs_tree_snapshot: true,
                    callback: Box::new(move |node, inner| {
                        if source.as_ref().is_some_and(|s| s.is_closed()) {
                            return Ok(());
                        }
                        let module = node
                            .as_any_mut()
                            .downcast_mut::<MidiModule>()
                            .ok_or("MIDI ordering owner missing")?;
                        if module.structure_work.generation != generation {
                            return Ok(());
                        }
                        if let Some(active) = module.structure_work.cc.active.as_mut() {
                            active.launch = None;
                        }
                        let snapshot = inner.tree_snapshot_arc();
                        module.advance_cc_structure(inner, snapshot.as_deref(), false);
                        module.request_structure_wake(inner);
                        Ok(())
                    }),
                });
                let launch = ordering.completion();
                self.structure_work
                    .cc
                    .active
                    .as_mut()
                    .expect("admitted conversion")
                    .launch = Some(launch);
                ctx.edits.pending.extend(ordering.drain());
            }
            Ok(None) => {
                self.cc_14_bit_state[index] = [Cc14BitState::default(); 32];
                ctx.clear_node_warning(self.id(), Some(WARNING));
            }
            Err(error) => {
                if let Some(toggle) = self.channel_toggle_id(channel).filter(|_| previous.restore) {
                    self.set_internal_param(ctx, toggle, ParamValue::Bool(previous.previous));
                }
                ctx.set_node_warning_with(self.id(), Some(WARNING), error, None);
            }
        }
    }
}

fn plan(module: &MidiModule, snapshot: &ProcessTreeSnapshot, channel: u8, wide: bool) -> Result<Option<Plan>, String> {
    let Some(folder) = module.find_cc_folder(snapshot, channel) else {
        return Ok(None);
    };
    let parent = snapshot.node(folder).ok_or("CC folder disappeared")?;
    if parent.node_type != "folder" || parent.child_count > 256 {
        return Err("CC conversion requires a folder with at most 256 children".into());
    }
    let channel_node = snapshot
        .node(parent.parent.ok_or("CC channel missing")?)
        .ok_or("CC channel missing")?;
    let values = module.base.values_id().ok_or("MIDI Values folder missing")?;
    if channel_node.node_type != "folder"
        || channel_node.parent != Some(values)
        || snapshot.node(values).is_none_or(|n| n.parent != Some(module.id()))
    {
        return Err("CC conversion ownership changed".into());
    }
    let mut guards = vec![NodeReadGuard::capture(snapshot, folder).ok_or("CC folder read unavailable")?];
    guards.push(NodeReadGuard::capture(snapshot, channel_node.id).ok_or("CC channel read unavailable")?);
    guards.push(NodeReadGuard::capture(snapshot, values).ok_or("MIDI Values read unavailable")?);
    if let Some(toggle) = module.channel_toggle_id(channel) {
        guards.push(NodeReadGuard::capture(snapshot, toggle).ok_or("CC width read unavailable")?);
    }
    let mut builder = Builder {
        source: module.id(),
        snapshot,
        folder,
        edits: EditQueue::new(),
        guards,
        footprint: 256,
    };
    let mut states = [Cc14BitState::default(); 32];
    for controller in 0..32u8 {
        let base = snapshot.find_child_by_decl_id(folder, &cc_decl_id(controller));
        let low = snapshot.find_child_by_decl_id(folder, &cc_decl_id(controller + 32));
        if base.is_none() && low.is_none() {
            continue;
        }
        let msb = integer(snapshot, base, &mut builder.guards)?;
        let lsb = integer(snapshot, low, &mut builder.guards)?;
        let pair_wide = match base
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_constraints.as_ref())
            .and_then(|constraints| constraints.range.as_ref())
        {
            Some(RangeConstraint::Uniform {
                min: Some(min),
                max: Some(max),
            }) if *min == 0.0 && *max == 16383.0 => true,
            Some(RangeConstraint::Uniform {
                min: Some(min),
                max: Some(max),
            }) if *min == 0.0 && *max == 127.0 => false,
            _ if base.is_none() => false,
            _ => return Err("CC conversion requires declared 7-bit or 14-bit ranges".into()),
        };
        if wide {
            let (msb, lsb) = if pair_wide {
                let combined = clamp_i32_to_u14(msb);
                ((combined >> 7) as u8, (combined & 127) as u8)
            } else {
                (clamp_i32_to_u7(msb), clamp_i32_to_u7(lsb))
            };
            builder.write(
                controller,
                base,
                i32::from((u16::from(msb) << 7) | u16::from(lsb)),
                true,
            )?;
            if let Some(low) = low {
                admit_removal(snapshot, low, &mut builder.footprint, &mut builder.guards)?;
                builder.edits.push(Edit::RemoveNode { node: low });
            }
            states[usize::from(controller)] = Cc14BitState {
                msb: Some(msb),
                lsb: Some(lsb),
            };
        } else {
            let combined = if pair_wide {
                clamp_i32_to_u14(msb)
            } else {
                (u16::from(clamp_i32_to_u7(msb)) << 7) | u16::from(clamp_i32_to_u7(lsb))
            };
            // Preserve the low bits before narrowing the combined value. A failed
            // prefix must leave either the full-width source or its complete split.
            builder.write(controller + 32, low, i32::from(combined & 127), false)?;
            builder.write(controller, base, i32::from((combined >> 7) & 127), false)?;
        }
    }
    // All read fences precede authored effects, even when more than 32 reads
    // are needed. Each individual public fence retains its own bounded shape.
    let mut fenced = EditQueue::new();
    for chunk in builder.guards.chunks(32) {
        fenced.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(chunk.to_vec(), vec![]).expect("bounded CC reads"),
        });
    }
    let fence = fenced.completion();
    fenced.pending.extend(builder.edits.drain());
    assert!(fenced.pending.len() <= 200, "fixed 32-pair CC factory");
    Ok(Some(Plan {
        folder,
        uuid: parent.uuid,
        states,
        edits: fenced,
        fence,
    }))
}
fn integer(snapshot: &ProcessTreeSnapshot, id: Option<NodeId>, guards: &mut Vec<NodeReadGuard>) -> Result<i32, String> {
    let Some(id) = id else { return Ok(0) };
    let node = snapshot.node(id).ok_or("CC value disappeared")?;
    if node.node_type != "int" {
        return Err("CC conversion requires integer parameter nodes".into());
    }
    let Some(ParamValue::Int(value)) = &node.param_value else {
        return Err("CC conversion requires typed integer controls".into());
    };
    guards.push(NodeReadGuard::capture(snapshot, id).ok_or("CC value read unavailable")?);
    Ok(*value)
}
struct Builder<'a> {
    source: NodeId,
    snapshot: &'a ProcessTreeSnapshot,
    folder: NodeId,
    edits: EditQueue,
    guards: Vec<NodeReadGuard>,
    footprint: usize,
}
impl Builder<'_> {
    fn write(&mut self, controller: u8, id: Option<NodeId>, value: i32, wide: bool) -> Result<(), String> {
        let snapshot = self.snapshot;
        let folder = self.folder;
        let edits = &mut self.edits;
        let guards = &mut self.guards;
        let range = Some(if wide && controller < 32 {
            MIDI_14_BIT_VALUE_RANGE
        } else {
            MIDI_7_BIT_VALUE_RANGE
        });
        let constraints = int_parameter_constraints(range);
        if let Some(id) = id {
            let node = snapshot.node(id).ok_or("CC value disappeared")?;
            if node.param_constraints.as_ref() != Some(&constraints) {
                admit_removal(snapshot, id, &mut self.footprint, guards)?;
                edits.push_from(
                    self.source,
                    Edit::CallNodeMutation {
                        node: id,
                        needs_tree_snapshot: false,
                        callback: Box::new(move |target, _| {
                            let parameter = target
                                .as_any_mut()
                                .downcast_mut::<Parameter>()
                                .ok_or("CC state target changed type")?;
                            let ParamValue::Int(default) = parameter.default_value else {
                                return Err("CC default must be an integer".into());
                            };
                            let next = constraints.normalize(ParamValue::Int(value))?;
                            let default = constraints.normalize(ParamValue::Int(default))?;
                            // Both values are prepared before this infallible commit.
                            // Core captures this one node mutation as one history effect.
                            parameter.value = next;
                            parameter.default_value = default;
                            parameter.constraints = constraints;
                            Ok(())
                        }),
                    },
                );
            } else if node.param_value.as_ref() != Some(&ParamValue::Int(value)) {
                edits.push_from(
                    self.source,
                    Edit::SetParam {
                        node: id,
                        value: ParamValue::Int(value),
                        behaviour: ParameterEventBehaviour::Append,
                    },
                );
            }
            match snapshot.find_child_by_decl_id(id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                Some(mode) => {
                    if snapshot.node(mode).is_none_or(|n| n.node_type != "enum") {
                        return Err("CC rotary selector must be an enum".into());
                    }
                    guards.push(NodeReadGuard::capture(snapshot, mode).ok_or("CC selector read unavailable")?);
                }
                None => edits.push(Edit::AddNodeTree {
                    parent: id,
                    prev_sibling: None,
                    tree: NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                }),
            }
        } else {
            edits.push(Edit::AddNodeTree {
                parent: folder,
                prev_sibling: None,
                tree: NodeTree::new(create_int_parameter(
                    &cc_label(controller),
                    &cc_decl_id(controller),
                    value,
                    range,
                    false,
                ))
                .with_child(NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE))),
            });
        }
        Ok(())
    }
}

fn admit_removal(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    budget: &mut usize,
    guards: &mut Vec<NodeReadGuard>,
) -> Result<(), String> {
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        if *budget == 0 {
            return Err("CC conversion exceeds 256 retired nodes".into());
        }
        *budget -= 1;
        guards.push(NodeReadGuard::capture(snapshot, id).ok_or("CC retirement read unavailable")?);
        let children = snapshot.child_ids_slice(id);
        if children.len() + pending.len() > *budget {
            return Err("CC conversion exceeds 256 retired nodes".into());
        }
        pending.extend_from_slice(children);
    }
    Ok(())
}
pub(super) fn order(snapshot: &ProcessTreeSnapshot, folder: NodeId, uuid: NodeUuid) -> Result<EditQueue, String> {
    let parent = snapshot.node(folder).ok_or("CC ordering folder disappeared")?;
    if parent.uuid != uuid || parent.child_count > 256 {
        return Err("CC ordering target changed or exceeds 256 children".into());
    }
    let desired = (0..128)
        .filter_map(|cc| snapshot.find_child_by_decl_id(folder, &cc_decl_id(cc)))
        .collect::<Vec<_>>();
    let current = snapshot
        .child_ids_slice(folder)
        .iter()
        .filter_map(|id| {
            let node = snapshot.node(*id)?;
            (node.decl_id.len() <= 6)
                .then(|| cc_from_decl_id(&node.decl_id))
                .flatten()
                .map(|_| *id)
        })
        .collect::<Vec<_>>();
    let mut edits = EditQueue::new();
    if current != desired {
        edits.push(Edit::CheckPreconditions {
            preconditions: EditPreconditions::new(
                vec![NodeReadGuard::capture(snapshot, folder).ok_or("CC order read unavailable")?],
                vec![],
            )
            .expect("one folder"),
        });
        for id in desired.into_iter().rev() {
            edits.push(Edit::MoveNode {
                node: id,
                new_parent: folder,
                new_prev_sibling: None,
            });
        }
    }
    Ok(edits)
}
