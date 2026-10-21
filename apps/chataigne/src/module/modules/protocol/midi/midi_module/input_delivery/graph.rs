use super::*;
use golden_core::edit::NodeTree;

pub(super) enum Value {
    Small(ParamValue),
    Hex,
}
#[derive(Clone)]
pub(super) struct FolderSpec {
    pub label: String,
    pub decl: String,
}
pub(super) enum Progress {
    Done,
    Pending,
}
pub(super) enum Target {
    Direct(NodeId),
    Path(Vec<FolderSpec>),
}
pub(super) struct Write {
    pub target: Target,
    pub value: Value,
    pub label: String,
    pub decl: String,
    pub range: Option<(i32, i32)>,
    pub read_only: bool,
    pub rotary: Option<&'static str>,
    pub initialized: bool,
}
impl Write {
    pub(super) fn direct(node: NodeId, value: ParamValue) -> Self {
        Self {
            target: Target::Direct(node),
            value: Value::Small(value),
            label: String::new(),
            decl: String::new(),
            range: None,
            read_only: false,
            rotary: None,
            initialized: false,
        }
    }
    pub(super) fn value<'a>(&'a self, input: &'a PreparedInput) -> &'a ParamValue {
        match &self.value {
            Value::Small(value) => value,
            Value::Hex => input.hex.as_ref().expect("worker-prepared SysEx text"),
        }
    }
    pub(super) fn apply(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        root: Option<NodeId>,
        input: &PreparedInput,
        auto_add: bool,
    ) -> Result<Progress, String> {
        let value = match &self.value {
            Value::Small(value) => value,
            Value::Hex => input.hex.as_ref().expect("worker-prepared SysEx text"),
        };
        let target = match &self.target {
            Target::Direct(id) => *id,
            Target::Path(folders) => {
                let Some(mut parent) = root else {
                    return Ok(Progress::Done);
                };
                for (index, folder) in folders.iter().enumerate() {
                    bounded_parent(snapshot, parent)?;
                    match snapshot.find_child_by_decl_id(parent, &folder.decl) {
                        Some(id) if snapshot.node(id).is_some_and(|n| n.node_type == "folder") => parent = id,
                        Some(id) => {
                            if !auto_add {
                                return Ok(Progress::Done);
                            }
                            bounded_parent(snapshot, id)?;
                            ctx.replace_node_boxed(id, Box::new(create_folder(&folder.label, &folder.decl)));
                            return Ok(Progress::Pending);
                        }
                        None => {
                            if !auto_add {
                                return Ok(Progress::Done);
                            }
                            self.allow_creation()?;
                            ctx.add_child_tree(parent, self.tree(&folders[index..], value), None);
                            self.initialized = true;
                            return Ok(Progress::Pending);
                        }
                    }
                }
                bounded_parent(snapshot, parent)?;
                match snapshot.find_child_by_decl_id(parent, &self.decl) {
                    Some(id) if snapshot.node(id).is_some_and(|n| n.node_type == parameter_kind(value)) => id,
                    Some(id) => {
                        if !auto_add {
                            return Ok(Progress::Done);
                        }
                        bounded_parent(snapshot, id)?;
                        ctx.replace_node_boxed(id, Box::new(self.parameter(value)));
                        if self.rotary.is_some() {
                            match snapshot.find_child_by_decl_id(id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                                Some(mode) => ctx.replace_node_boxed(
                                    mode,
                                    Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                                ),
                                None => ctx.add_child_tree(
                                    id,
                                    NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                                    None,
                                ),
                            }
                        }
                        self.initialized = true;
                        return Ok(Progress::Pending);
                    }
                    None => {
                        if !auto_add {
                            return Ok(Progress::Done);
                        }
                        self.allow_creation()?;
                        let previous = ordered_previous(snapshot, parent, &self.decl);
                        ctx.add_child_tree(parent, self.tree(&[], value), previous);
                        self.initialized = true;
                        return Ok(Progress::Pending);
                    }
                }
            }
        };
        let Some(node) = snapshot.node(target) else {
            return Err("input parameter disappeared".into());
        };
        if matches!(self.target, Target::Path(_)) {
            if matches!(value, ParamValue::Int(_)) {
                sync_int_parameter_constraints(ctx, snapshot, target, self.range);
            }
            if let Some(expected) = self.rotary {
                bounded_parent(snapshot, target)?;
                if midi_cc_config(snapshot, target).rotary_mechanism != expected {
                    return Err("CC rotary mechanism changed during input delivery".into());
                }
                match snapshot.find_child_by_decl_id(target, MIDI_CC_ROTARY_MODE_DECL_ID) {
                    Some(mode) if snapshot.node(mode).is_some_and(|n| n.node_type == "enum") => {}
                    Some(mode) => {
                        ctx.replace_node_boxed(mode, Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)));
                        return Ok(Progress::Pending);
                    }
                    None => {
                        ctx.add_child_tree(
                            target,
                            NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                            None,
                        );
                        return Ok(Progress::Pending);
                    }
                }
            }
        }
        if self.initialized {
            let parent = node.parent.ok_or("created input lost its parent")?;
            if ordered_midi_value_kind(snapshot, parent).is_some()
                && ordered_previous(snapshot, parent, &self.decl).is_none()
                && snapshot.node(parent).is_some_and(|n| n.first_child != Some(target))
            {
                // AddNodeTree(None) appends; MoveNode(None) prepends. Only
                // the new numeric minimum needs this follow-up edit.
                ctx.edits.push(Edit::MoveNode {
                    node: target,
                    new_parent: parent,
                    new_prev_sibling: None,
                });
            }
            if !matches!(value, ParamValue::Trigger()) {
                return Ok(Progress::Done);
            }
        }
        if node.param_value.is_none() {
            return Err("input target is not a parameter".into());
        }
        ctx.set_param_with_behaviour(target, value.clone(), ParameterEventBehaviour::Append);
        Ok(Progress::Done)
    }
    fn parameter(&self, value: &ParamValue) -> Parameter {
        match value {
            ParamValue::Int(v) => create_int_parameter(&self.label, &self.decl, *v, self.range, self.read_only),
            ParamValue::Str(v) => create_string_parameter(&self.label, &self.decl, v.clone(), self.read_only),
            ParamValue::Trigger() => create_trigger_parameter(&self.label, &self.decl),
            _ => unreachable!("MIDI value-tree leaf kinds"),
        }
    }
    fn allow_creation(&self) -> Result<(), String> {
        if self.rotary.is_some_and(|mode| mode != ROTARY_ABSOLUTE) {
            Err("relative CC target disappeared during input delivery".into())
        } else {
            Ok(())
        }
    }
    fn tree(&self, folders: &[FolderSpec], value: &ParamValue) -> NodeTree {
        if let Some((head, tail)) = folders.split_first() {
            NodeTree::new(create_folder(&head.label, &head.decl)).with_child(self.tree(tail, value))
        } else {
            let mut tree = NodeTree::new(self.parameter(value));
            if self.rotary.is_some() {
                tree.push_child(NodeTree::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)));
            }
            tree
        }
    }
}
fn parameter_kind(value: &ParamValue) -> &str {
    match value {
        ParamValue::Int(_) => "int",
        ParamValue::Str(_) => "str",
        ParamValue::Trigger() => "trigger",
        _ => unreachable!(),
    }
}
fn bounded_parent(snapshot: &ProcessTreeSnapshot, parent: NodeId) -> Result<(), String> {
    let node = snapshot.node(parent).ok_or("input parent disappeared")?;
    if node.child_count > 256 {
        Err("MIDI input parent exceeds 256 children".into())
    } else {
        Ok(())
    }
}
fn ordered_previous(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl: &str) -> Option<NodeId> {
    let kind = ordered_midi_value_kind(snapshot, parent)?;
    let key = ordered_midi_value_key(kind, decl)?;
    snapshot
        .child_ids_slice(parent)
        .iter()
        .filter_map(|id| {
            let node = snapshot.node(*id)?;
            let other = ordered_midi_value_key(kind, &node.decl_id)?;
            (other < key).then_some((other, *id))
        })
        .max_by_key(|(key, _)| *key)
        .map(|(_, id)| id)
}
