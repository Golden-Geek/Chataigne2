use super::{
    apply_received_value_batch, indexed_value_label, indexed_value_label_index, param_types_match,
    ReceivedValueBatchMessage, ReceivedValueBatchOptions, ReceivedValueMessage, ReceivedValuePayload,
};
use golden_core::edit::{Edit, EditCompletion, EditCompletionStatus};
use golden_core::{
    node::NodeId,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(crate) const VALUE_WORK_UNITS: usize = 256;
const VALUE_WORK_BYTES: usize = 64 * 1024;

#[derive(PartialEq, Eq)]
pub(crate) enum ReceivedValueProgress {
    Pending,
    Complete,
    Failed,
}

#[derive(Default)]
pub(crate) struct ReceivedValueCursor {
    message: usize,
    list_item: usize,
    trim: Option<TrimCursor>,
    completion: Option<EditCompletion>,
}

#[derive(Default)]
struct TrimCursor {
    folder: Option<NodeId>,
    next_child: Option<NodeId>,
}

struct ValueUnit<'a> {
    path: Vec<String>,
    payload: ReceivedValuePayload,
    source_description: Option<&'a str>,
    existing_compatible: bool,
}

struct TurnBudget {
    units: usize,
    bytes: usize,
}

impl TurnBudget {
    fn take(&mut self, units: usize, bytes: usize) -> bool {
        // A single large scalar/path gets a whole turn instead of being stranded or truncated.
        // Its size was already bounded at the producer admission boundary.
        let untouched = self.bytes == VALUE_WORK_BYTES;
        if units > self.units || (bytes > self.bytes && !untouched) {
            return false;
        }
        self.units -= units;
        self.bytes = self.bytes.saturating_sub(bytes);
        true
    }
}

impl ReceivedValueCursor {
    pub(crate) fn requires_snapshot(&self, messages: &[ReceivedValueMessage]) -> bool {
        self.completion
            .as_ref()
            .is_none_or(|completion| completion.status() == EditCompletionStatus::Applied)
            && self.message < messages.len()
    }

    /// Plans a bounded prefix. Confirm application before using the next snapshot to advance.
    pub(crate) fn service(
        &mut self,
        messages: &[ReceivedValueMessage],
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        values_id: Option<NodeId>,
        auto_add: bool,
        include_source: bool,
    ) -> ReceivedValueProgress {
        if let Some(completion) = self.completion.take() {
            match completion.status() {
                EditCompletionStatus::Pending => {
                    self.completion = Some(completion);
                    return ReceivedValueProgress::Pending;
                }
                EditCompletionStatus::Failed => return ReceivedValueProgress::Failed,
                EditCompletionStatus::Applied => {}
            }
        }
        let Some(root) = values_id else {
            return ReceivedValueProgress::Complete;
        };
        if self.message == messages.len() {
            return ReceivedValueProgress::Complete;
        }
        let Some(snapshot) = snapshot else {
            return ReceivedValueProgress::Pending;
        };
        if snapshot.node(root).is_none() {
            return ReceivedValueProgress::Complete;
        }
        let mut budget = TurnBudget {
            units: VALUE_WORK_UNITS,
            bytes: VALUE_WORK_BYTES,
        };
        let mut units = Vec::<ValueUnit<'_>>::new();
        let mut removals = Vec::new();

        while let Some(message) = messages.get(self.message) {
            if message.path_segments.is_empty() {
                if !budget.take(1, 0) {
                    break;
                }
                self.message += 1;
                continue;
            }
            match &message.payload {
                ReceivedValuePayload::Single(value) => {
                    let Some(existing_compatible) =
                        compatible_target(snapshot, root, &message.path_segments, None, value, &mut budget)
                    else {
                        break;
                    };
                    if !push_unit(
                        &mut units,
                        &mut budget,
                        &message.path_segments,
                        None,
                        value,
                        include_source.then_some(message.source_description.as_str()),
                        existing_compatible,
                    ) {
                        break;
                    }
                    if matches!(value, ParamValue::Trigger()) && auto_add && !existing_compatible {
                        break;
                    }
                    self.message += 1;
                }
                ReceivedValuePayload::Multi(values) => {
                    if let Some(value) = values.get(self.list_item) {
                        let Some(existing_compatible) = compatible_target(
                            snapshot,
                            root,
                            &message.path_segments,
                            Some(self.list_item),
                            value,
                            &mut budget,
                        ) else {
                            break;
                        };
                        if !push_unit(
                            &mut units,
                            &mut budget,
                            &message.path_segments,
                            Some(self.list_item),
                            value,
                            include_source.then_some(message.source_description.as_str()),
                            existing_compatible,
                        ) {
                            break;
                        }
                        if matches!(value, ParamValue::Trigger()) && auto_add && !existing_compatible {
                            break;
                        }
                        self.list_item += 1;
                        continue;
                    }
                    // Pruning scans only a bounded sibling prefix. It uses stable next-sibling
                    // identities so our own deletions cannot shift a numeric scan past a child.
                    let trim = self.trim.get_or_insert_with(TrimCursor::default);
                    if !trim.service(
                        snapshot,
                        root,
                        &message.path_segments,
                        values.len(),
                        &mut budget,
                        &mut removals,
                    ) {
                        break;
                    }
                    self.message += 1;
                    self.list_item = 0;
                    self.trim = None;
                    // A later message may target a removed child; let the edits settle first.
                    if !removals.is_empty() {
                        break;
                    }
                }
            }
        }

        let attempted_values = !units.is_empty();
        if attempted_values {
            apply_received_value_batch(
                ctx,
                snapshot,
                root,
                units.iter().map(|unit| ReceivedValueBatchMessage {
                    path_segments: &unit.path,
                    payload: &unit.payload,
                    source_description: unit.source_description,
                }),
                ReceivedValueBatchOptions {
                    auto_add,
                    event_behaviour: ParameterEventBehaviour::Append,
                },
            );
        }
        let changed_structure = !removals.is_empty();
        for node in removals {
            ctx.edits.push(Edit::RemoveNode { node });
        }
        if attempted_values || changed_structure {
            self.completion = Some(ctx.edits.completion());
            ReceivedValueProgress::Pending
        } else if self.message == messages.len() {
            ReceivedValueProgress::Complete
        } else {
            ReceivedValueProgress::Pending
        }
    }
}

fn push_unit<'a>(
    units: &mut Vec<ValueUnit<'a>>,
    budget: &mut TurnBudget,
    path: &[String],
    list_item: Option<usize>,
    value: &ParamValue,
    source_description: Option<&'a str>,
    existing_compatible: bool,
) -> bool {
    let label = list_item.map(indexed_value_label);
    let source_description = source_description.filter(|_| list_item.is_none());
    let value_bytes = super::message::value_dynamic_weight(value);
    let bytes = path.iter().map(String::len).sum::<usize>()
        + label.as_ref().map_or(0, String::len)
        + std::mem::size_of::<ParamValue>()
        + value_bytes
        + source_description.map_or(0, str::len);
    if !budget.take(path.len() + usize::from(label.is_some()) + 1, bytes) {
        return false;
    }
    let mut path = path.to_vec();
    if let Some(label) = label {
        path.push(label);
    }
    // Existing compatible values may repeat: the planner tracks their ordered prefix.
    // Structural collisions need a fresh snapshot before planning the next operation.
    if units.iter().any(|unit| {
        !(existing_compatible && unit.existing_compatible && unit.path == path)
            && (unit.path.starts_with(&path) || path.starts_with(&unit.path))
    }) {
        return false;
    }
    units.push(ValueUnit {
        path,
        payload: ReceivedValuePayload::Single(value.clone()),
        source_description,
        existing_compatible,
    });
    true
}

fn compatible_target(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    path: &[String],
    list_item: Option<usize>,
    value: &ParamValue,
    budget: &mut TurnBudget,
) -> Option<bool> {
    if !budget.take(path.len() + usize::from(list_item.is_some()) + 1, 0) {
        return None;
    }
    let mut target = path
        .iter()
        .try_fold(root, |parent, segment| snapshot.find_child(parent, segment));
    if let Some(index) = list_item {
        target = target.and_then(|parent| snapshot.find_child(parent, &indexed_value_label(index)));
    }
    // Construction establishes an address; trigger construction is not a pulse.
    // Existing compatible parameters can receive an ordered Append sequence in this turn.
    Some(target.and_then(|id| snapshot.node(id)).is_some_and(|node| {
        node.param_value
            .as_ref()
            .is_some_and(|current| param_types_match(current, value))
    }))
}

impl TrimCursor {
    fn service(
        &mut self,
        snapshot: &ProcessTreeSnapshot,
        root: NodeId,
        path: &[String],
        desired_count: usize,
        budget: &mut TurnBudget,
        removals: &mut Vec<NodeId>,
    ) -> bool {
        if !budget.take(path.len() + 1, 0) {
            return false;
        }
        let folder = path
            .iter()
            .try_fold(root, |parent, segment| snapshot.find_child(parent, segment));
        let Some(folder) = folder.filter(|id| snapshot.node(*id).is_some_and(|node| node.node_type == "folder")) else {
            // A missing/replaced folder is being created from detached value units in this
            // turn, or auto-add is disabled. There is no old list tail to delete.
            return true;
        };
        if self.folder != Some(folder) {
            self.folder = Some(folder);
            self.next_child = snapshot.node(folder).and_then(|node| node.first_child);
        }
        while let Some(child) = self.next_child {
            if !budget.take(1, 0) {
                return false;
            }
            let Some(node) = snapshot.node(child).filter(|node| node.parent == Some(folder)) else {
                // External edits may have removed our next marker. Re-arm a bounded scan.
                self.next_child = snapshot.node(folder).and_then(|node| node.first_child);
                continue;
            };
            self.next_child = node.next_sibling;
            if indexed_value_label_index(&node.label).is_some_and(|index| index >= desired_count) {
                removals.push(child);
            }
        }
        true
    }
}
