use std::collections::HashMap;

use golden_core::{
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const VALUE_LABEL_PREFIX: &str = "value ";

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ReceivedValuePayload {
    Single(ParamValue),
    Multi(Vec<ParamValue>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReceivedValueApplyResult {
    Applied {
        needs_snapshot_refresh: bool,
    },
    Retry,
    Ignored,
}

impl ReceivedValueApplyResult {
    pub(crate) fn applied(needs_snapshot_refresh: bool) -> Self {
        Self::Applied {
            needs_snapshot_refresh,
        }
    }
}

pub(crate) fn apply_received_value_payload(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    path_segments: &[String],
    payload: &ReceivedValuePayload,
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    if path_segments.is_empty() {
        return ReceivedValueApplyResult::Ignored;
    }

    let (parent_id, leaf_name) =
        match resolve_or_create_parent(ctx, snapshot, values_id, path_segments, options.auto_add) {
            ParentResolution::Ready { parent_id, leaf_name } => (parent_id, leaf_name),
            ParentResolution::Retry => return ReceivedValueApplyResult::Retry,
            ParentResolution::Ignored => return ReceivedValueApplyResult::Ignored,
        };

    match payload {
        ReceivedValuePayload::Single(value) => apply_single_value_message(
            ctx,
            snapshot,
            parent_id,
            leaf_name,
            value.clone(),
            options,
        ),
        ReceivedValuePayload::Multi(values) => {
            apply_multi_value_message(ctx, snapshot, parent_id, leaf_name, values, options)
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ReceivedValueApplyOptions<'a> {
    pub(crate) auto_add: bool,
    pub(crate) source_description: &'a str,
    pub(crate) event_behaviour: ParameterEventBehaviour,
}

pub(crate) struct ReceivedValueBatchMessage<'a> {
    pub(crate) path_segments: &'a [String],
    pub(crate) payload: &'a ReceivedValuePayload,
    pub(crate) source_description: &'a str,
}

#[derive(Clone, Copy)]
pub(crate) struct ReceivedValueBatchOptions {
    pub(crate) auto_add: bool,
    pub(crate) event_behaviour: ParameterEventBehaviour,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ReceivedValueBatchResult {
    pub(crate) applied: usize,
    pub(crate) ignored: usize,
    pub(crate) structure_changed: bool,
}

pub(crate) fn apply_received_value_batch<'a, I>(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    messages: I,
    options: ReceivedValueBatchOptions,
) -> ReceivedValueBatchResult
where
    I: IntoIterator<Item = ReceivedValueBatchMessage<'a>>,
{
    let mut planner = ReceivedValueBatchPlanner::new(ctx, snapshot, options);

    for message in messages {
        if message.path_segments.is_empty() {
            planner.result.ignored += 1;
            continue;
        }

        if planner.plan_existing_parent(
            values_id,
            message.path_segments,
            message.payload,
            message.source_description,
            true,
        ) {
            planner.result.applied += 1;
        } else {
            planner.result.ignored += 1;
        }
    }

    planner.flush_planned_edits();
    planner.result
}

struct ReceivedValueBatchPlanner<'a, 'ctx> {
    ctx: &'ctx mut ProcessCtx,
    snapshot: &'a ProcessTreeSnapshot,
    options: ReceivedValueBatchOptions,
    planned_children_by_existing_parent: HashMap<NodeId, Vec<PlannedReceivedNode>>,
    result: ReceivedValueBatchResult,
}

enum PlannedReceivedNode {
    Folder(PlannedReceivedFolder),
    Parameter(PlannedReceivedParameter),
}

struct PlannedReceivedFolder {
    label: String,
    existing_id: Option<NodeId>,
    children: Vec<PlannedReceivedNode>,
}

struct PlannedReceivedParameter {
    label: String,
    existing_id: Option<NodeId>,
    value: ParamValue,
    description: Option<String>,
}

impl<'a, 'ctx> ReceivedValueBatchPlanner<'a, 'ctx> {
    fn new(
        ctx: &'ctx mut ProcessCtx,
        snapshot: &'a ProcessTreeSnapshot,
        options: ReceivedValueBatchOptions,
    ) -> Self {
        Self {
            ctx,
            snapshot,
            options,
            planned_children_by_existing_parent: HashMap::new(),
            result: ReceivedValueBatchResult::default(),
        }
    }

    fn plan_existing_parent(
        &mut self,
        parent_id: NodeId,
        path_segments: &[String],
        payload: &ReceivedValuePayload,
        source_description: &str,
        describe_created_leaf: bool,
    ) -> bool {
        let Some((head, tail)) = path_segments.split_first() else {
            return false;
        };

        if tail.is_empty() {
            return self.plan_leaf_under_existing(
                parent_id,
                head.as_str(),
                payload,
                source_description,
                describe_created_leaf,
            );
        }

        if let Some(index) = self.planned_child_index_under_existing(parent_id, head.as_str()) {
            let children = self
                .planned_children_by_existing_parent
                .get_mut(&parent_id)
                .expect("planned child index should have a parent entry");
            return Self::plan_existing_planned_child(
                children,
                index,
                tail,
                payload,
                source_description,
                describe_created_leaf,
                self.options.auto_add,
            );
        }

        match self.snapshot.find_child(parent_id, head.as_str()) {
            Some(child_id) => {
                let Some(child_snapshot) = self.snapshot.node(child_id) else {
                    return false;
                };
                if child_snapshot.node_type == "folder" {
                    self.plan_existing_parent(
                        child_id,
                        tail,
                        payload,
                        source_description,
                        describe_created_leaf,
                    )
                } else if self.options.auto_add {
                    let children = self
                        .planned_children_by_existing_parent
                        .entry(parent_id)
                        .or_default();
                    children.push(PlannedReceivedNode::Folder(PlannedReceivedFolder {
                        label: head.clone(),
                        existing_id: Some(child_id),
                        children: Vec::new(),
                    }));
                    self.result.structure_changed = true;
                    let index = children.len() - 1;
                    Self::plan_existing_planned_child(
                        children,
                        index,
                        tail,
                        payload,
                        source_description,
                        describe_created_leaf,
                        self.options.auto_add,
                    )
                } else {
                    false
                }
            }
            None => {
                if !self.options.auto_add {
                    return false;
                }

                let children = self
                    .planned_children_by_existing_parent
                    .entry(parent_id)
                    .or_default();
                children.push(PlannedReceivedNode::Folder(PlannedReceivedFolder {
                    label: head.clone(),
                    existing_id: None,
                    children: Vec::new(),
                }));
                self.result.structure_changed = true;
                let index = children.len() - 1;
                Self::plan_existing_planned_child(
                    children,
                    index,
                    tail,
                    payload,
                    source_description,
                    describe_created_leaf,
                    self.options.auto_add,
                )
            }
        }
    }

    fn plan_existing_planned_child(
        siblings: &mut Vec<PlannedReceivedNode>,
        index: usize,
        path_segments: &[String],
        payload: &ReceivedValuePayload,
        source_description: &str,
        describe_created_leaf: bool,
        auto_add: bool,
    ) -> bool {
        match &mut siblings[index] {
            PlannedReceivedNode::Folder(folder) => {
                Self::plan_planned_parent(
                    folder,
                    path_segments,
                    payload,
                    source_description,
                    describe_created_leaf,
                    auto_add,
                )
            }
            PlannedReceivedNode::Parameter(_) if auto_add => {
                let label = siblings[index].label().to_string();
                let existing_id = siblings[index].existing_id();
                siblings[index] = PlannedReceivedNode::Folder(PlannedReceivedFolder {
                    label,
                    existing_id,
                    children: Vec::new(),
                });
                let PlannedReceivedNode::Folder(folder) = &mut siblings[index] else {
                    unreachable!("planned child was just converted to a folder");
                };
                Self::plan_planned_parent(
                    folder,
                    path_segments,
                    payload,
                    source_description,
                    describe_created_leaf,
                    auto_add,
                )
            }
            PlannedReceivedNode::Parameter(_) => false,
        }
    }

    fn plan_planned_parent(
        folder: &mut PlannedReceivedFolder,
        path_segments: &[String],
        payload: &ReceivedValuePayload,
        source_description: &str,
        describe_created_leaf: bool,
        auto_add: bool,
    ) -> bool {
        let Some((head, tail)) = path_segments.split_first() else {
            return false;
        };

        if tail.is_empty() {
            return Self::plan_leaf_under_planned(
                folder,
                head.as_str(),
                payload,
                source_description,
                describe_created_leaf,
                auto_add,
            );
        }

        if let Some(index) = planned_child_index(folder.children.as_slice(), head.as_str()) {
            return Self::plan_existing_planned_child(
                &mut folder.children,
                index,
                tail,
                payload,
                source_description,
                describe_created_leaf,
                auto_add,
            );
        }

        if !auto_add {
            return false;
        }

        folder
            .children
            .push(PlannedReceivedNode::Folder(PlannedReceivedFolder {
                label: head.clone(),
                existing_id: None,
                children: Vec::new(),
            }));
        let index = folder.children.len() - 1;
        Self::plan_existing_planned_child(
            &mut folder.children,
            index,
            tail,
            payload,
            source_description,
            describe_created_leaf,
            auto_add,
        )
    }

    fn plan_leaf_under_existing(
        &mut self,
        parent_id: NodeId,
        leaf_name: &str,
        payload: &ReceivedValuePayload,
        source_description: &str,
        describe_created_leaf: bool,
    ) -> bool {
        match payload {
            ReceivedValuePayload::Single(value) => {
                self.plan_single_leaf_under_existing(
                    parent_id,
                    leaf_name,
                    value.clone(),
                    source_description,
                    describe_created_leaf,
                )
            }
            ReceivedValuePayload::Multi(values) => self.plan_multi_leaf_under_existing(
                parent_id,
                leaf_name,
                values,
                source_description,
            ),
        }
    }

    fn plan_leaf_under_planned(
        folder: &mut PlannedReceivedFolder,
        leaf_name: &str,
        payload: &ReceivedValuePayload,
        source_description: &str,
        describe_created_leaf: bool,
        auto_add: bool,
    ) -> bool {
        match payload {
            ReceivedValuePayload::Single(value) => Self::plan_single_leaf_under_planned(
                folder,
                leaf_name,
                value.clone(),
                source_description,
                describe_created_leaf,
                auto_add,
            ),
            ReceivedValuePayload::Multi(values) => {
                Self::plan_multi_leaf_under_planned(folder, leaf_name, values, auto_add)
            }
        }
    }

    fn plan_single_leaf_under_existing(
        &mut self,
        parent_id: NodeId,
        leaf_name: &str,
        value: ParamValue,
        source_description: &str,
        describe_created_leaf: bool,
    ) -> bool {
        if let Some(index) = self.planned_child_index_under_existing(parent_id, leaf_name) {
            let children = self
                .planned_children_by_existing_parent
                .get_mut(&parent_id)
                .expect("planned child index should have a parent entry");
            Self::set_planned_parameter(
                children,
                index,
                leaf_name,
                None,
                value,
                planned_description(source_description, describe_created_leaf),
                self.options.auto_add,
            );
            return true;
        }

        match self.snapshot.find_child(parent_id, leaf_name) {
            Some(existing_id) => {
                let Some(existing_snapshot) = self.snapshot.node(existing_id) else {
                    return false;
                };

                if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                    if param_types_match(existing_value, &value) {
                        self.ctx
                            .set_param_with_behaviour(existing_id, value, self.options.event_behaviour);
                        return true;
                    }
                }

                if self.options.auto_add {
                    self.planned_children_by_existing_parent
                        .entry(parent_id)
                        .or_default()
                        .push(PlannedReceivedNode::Parameter(PlannedReceivedParameter {
                            label: leaf_name.to_string(),
                            existing_id: Some(existing_id),
                            value,
                            description: planned_description(source_description, describe_created_leaf),
                        }));
                    self.result.structure_changed = true;
                }
                true
            }
            None => {
                if !self.options.auto_add {
                    return false;
                }

                self.planned_children_by_existing_parent
                    .entry(parent_id)
                    .or_default()
                    .push(PlannedReceivedNode::Parameter(PlannedReceivedParameter {
                        label: leaf_name.to_string(),
                        existing_id: None,
                        value,
                        description: planned_description(source_description, describe_created_leaf),
                    }));
                self.result.structure_changed = true;
                true
            }
        }
    }

    fn plan_single_leaf_under_planned(
        folder: &mut PlannedReceivedFolder,
        leaf_name: &str,
        value: ParamValue,
        source_description: &str,
        describe_created_leaf: bool,
        auto_add: bool,
    ) -> bool {
        if let Some(index) = planned_child_index(folder.children.as_slice(), leaf_name) {
            Self::set_planned_parameter(
                &mut folder.children,
                index,
                leaf_name,
                None,
                value,
                planned_description(source_description, describe_created_leaf),
                auto_add,
            );
            return true;
        }

        if !auto_add {
            return false;
        }

        folder
            .children
            .push(PlannedReceivedNode::Parameter(PlannedReceivedParameter {
                label: leaf_name.to_string(),
                existing_id: None,
                value,
                description: planned_description(source_description, describe_created_leaf),
            }));
        true
    }

    fn set_planned_parameter(
        siblings: &mut [PlannedReceivedNode],
        index: usize,
        label: &str,
        existing_id: Option<NodeId>,
        value: ParamValue,
        description: Option<String>,
        auto_add: bool,
    ) {
        match &mut siblings[index] {
            PlannedReceivedNode::Parameter(parameter) => {
                parameter.value = value;
                parameter.description = description;
            }
            PlannedReceivedNode::Folder(_) if auto_add => {
                siblings[index] = PlannedReceivedNode::Parameter(PlannedReceivedParameter {
                    label: label.to_string(),
                    existing_id,
                    value,
                    description,
                });
            }
            PlannedReceivedNode::Folder(_) => {}
        }
    }

    fn plan_multi_leaf_under_existing(
        &mut self,
        parent_id: NodeId,
        leaf_name: &str,
        values: &[ParamValue],
        source_description: &str,
    ) -> bool {
        if let Some(index) = self.planned_child_index_under_existing(parent_id, leaf_name) {
            let children = self
                .planned_children_by_existing_parent
                .get_mut(&parent_id)
                .expect("planned child index should have a parent entry");
            return Self::set_planned_multi_folder(
                children,
                index,
                leaf_name,
                None,
                values,
                self.options.auto_add,
            );
        }

        match self.snapshot.find_child(parent_id, leaf_name) {
            Some(existing_id) => {
                let Some(existing_snapshot) = self.snapshot.node(existing_id) else {
                    return false;
                };

                if existing_snapshot.node_type == "folder" {
                    return self.sync_existing_multi_folder(existing_id, values, source_description);
                }

                if !self.options.auto_add {
                    return false;
                }

                let mut folder = PlannedReceivedFolder {
                    label: leaf_name.to_string(),
                    existing_id: Some(existing_id),
                    children: Vec::new(),
                };
                Self::fill_planned_multi_folder(&mut folder, values);
                self.planned_children_by_existing_parent
                    .entry(parent_id)
                    .or_default()
                    .push(PlannedReceivedNode::Folder(folder));
                self.result.structure_changed = true;
                true
            }
            None => {
                if !self.options.auto_add {
                    return false;
                }

                let mut folder = PlannedReceivedFolder {
                    label: leaf_name.to_string(),
                    existing_id: None,
                    children: Vec::new(),
                };
                Self::fill_planned_multi_folder(&mut folder, values);
                self.planned_children_by_existing_parent
                    .entry(parent_id)
                    .or_default()
                    .push(PlannedReceivedNode::Folder(folder));
                self.result.structure_changed = true;
                true
            }
        }
    }

    fn plan_multi_leaf_under_planned(
        folder: &mut PlannedReceivedFolder,
        leaf_name: &str,
        values: &[ParamValue],
        auto_add: bool,
    ) -> bool {
        if let Some(index) = planned_child_index(folder.children.as_slice(), leaf_name) {
            return Self::set_planned_multi_folder(&mut folder.children, index, leaf_name, None, values, auto_add);
        }

        if !auto_add {
            return false;
        }

        let mut child_folder = PlannedReceivedFolder {
            label: leaf_name.to_string(),
            existing_id: None,
            children: Vec::new(),
        };
        Self::fill_planned_multi_folder(&mut child_folder, values);
        folder
            .children
            .push(PlannedReceivedNode::Folder(child_folder));
        true
    }

    fn set_planned_multi_folder(
        siblings: &mut [PlannedReceivedNode],
        index: usize,
        label: &str,
        existing_id: Option<NodeId>,
        values: &[ParamValue],
        auto_add: bool,
    ) -> bool {
        match &mut siblings[index] {
            PlannedReceivedNode::Folder(folder) => {
                folder.children.clear();
                Self::fill_planned_multi_folder(folder, values);
                true
            }
            PlannedReceivedNode::Parameter(_) if auto_add => {
                let mut folder = PlannedReceivedFolder {
                    label: label.to_string(),
                    existing_id,
                    children: Vec::new(),
                };
                Self::fill_planned_multi_folder(&mut folder, values);
                siblings[index] = PlannedReceivedNode::Folder(folder);
                true
            }
            PlannedReceivedNode::Parameter(_) => false,
        }
    }

    fn fill_planned_multi_folder(folder: &mut PlannedReceivedFolder, values: &[ParamValue]) {
        for (index, value) in values.iter().enumerate() {
            folder
                .children
                .push(PlannedReceivedNode::Parameter(PlannedReceivedParameter {
                    label: indexed_value_label(index),
                    existing_id: None,
                    value: value.clone(),
                    description: None,
                }));
        }
    }

    fn sync_existing_multi_folder(
        &mut self,
        folder_id: NodeId,
        values: &[ParamValue],
        source_description: &str,
    ) -> bool {
        for (index, value) in values.iter().enumerate() {
            let label = indexed_value_label(index);
            self.plan_single_leaf_under_existing(
                folder_id,
                label.as_str(),
                value.clone(),
                source_description,
                false,
            );
        }

        for child_id in self.snapshot.child_ids(folder_id) {
            let Some(child_snapshot) = self.snapshot.node(child_id) else {
                continue;
            };
            if let Some(index) = indexed_value_label_index(child_snapshot.label.as_str()) {
                if index >= values.len() {
                    self.ctx.edits.push(Edit::RemoveNode { node: child_id });
                    self.result.structure_changed = true;
                }
            }
        }

        true
    }

    fn planned_child_index_under_existing(&self, parent_id: NodeId, key: &str) -> Option<usize> {
        self.planned_children_by_existing_parent
            .get(&parent_id)
            .and_then(|children| planned_child_index(children.as_slice(), key))
    }

    fn flush_planned_edits(&mut self) {
        let planned = std::mem::take(&mut self.planned_children_by_existing_parent);
        for (parent, children) in planned {
            for child in children {
                queue_planned_node(self.ctx, parent, child);
            }
        }
    }
}

impl PlannedReceivedNode {
    fn label(&self) -> &str {
        match self {
            Self::Folder(folder) => folder.label.as_str(),
            Self::Parameter(parameter) => parameter.label.as_str(),
        }
    }

    fn existing_id(&self) -> Option<NodeId> {
        match self {
            Self::Folder(folder) => folder.existing_id,
            Self::Parameter(parameter) => parameter.existing_id,
        }
    }
}

fn planned_child_index(children: &[PlannedReceivedNode], key: &str) -> Option<usize> {
    children.iter().position(|child| child.label() == key)
}

fn planned_description(source_description: &str, include: bool) -> Option<String> {
    include.then(|| format!("Auto-created from {source_description}"))
}

fn queue_planned_node(ctx: &mut ProcessCtx, parent: NodeId, node: PlannedReceivedNode) {
    match node {
        PlannedReceivedNode::Folder(folder) => {
            if let Some(existing_id) = folder.existing_id {
                ctx.replace_node_boxed(existing_id, Box::new(create_auto_values_folder(folder.label.as_str())));
                for child in folder.children {
                    queue_planned_node(ctx, existing_id, child);
                }
            } else {
                ctx.add_child_tree(parent, planned_folder_to_tree(folder), None);
            }
        }
        PlannedReceivedNode::Parameter(parameter) => {
            let node = Box::new(create_parameter_node(
                parameter.label.as_str(),
                parameter.value,
                parameter.description,
            ));
            if let Some(existing_id) = parameter.existing_id {
                ctx.replace_node_boxed(existing_id, node);
            } else {
                ctx.add_child_tree(parent, NodeTree::boxed(node), None);
            }
        }
    }
}

fn planned_folder_to_tree(folder: PlannedReceivedFolder) -> NodeTree {
    let mut tree = NodeTree::new(create_auto_values_folder(folder.label.as_str()));
    for child in folder.children {
        tree.push_child(planned_node_to_tree(child));
    }
    tree
}

fn planned_node_to_tree(node: PlannedReceivedNode) -> NodeTree {
    match node {
        PlannedReceivedNode::Folder(folder) => planned_folder_to_tree(folder),
        PlannedReceivedNode::Parameter(parameter) => NodeTree::new(create_parameter_node(
            parameter.label.as_str(),
            parameter.value,
            parameter.description,
        )),
    }
}

enum ParentResolution {
    Ready { parent_id: NodeId, leaf_name: String },
    Retry,
    Ignored,
}

fn resolve_or_create_parent(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    segments: &[String],
    auto_add: bool,
) -> ParentResolution {
    let mut current = values_id;
    let Some((leaf_name, parents)) = segments.split_last() else {
        return ParentResolution::Ignored;
    };

    for (index, segment) in parents.iter().enumerate() {
        match snapshot.find_child(current, segment.as_str()) {
            Some(child_id) => {
                let Some(child_snapshot) = snapshot.node(child_id) else {
                    return ParentResolution::Ignored;
                };
                if child_snapshot.node_type == "folder" {
                    current = child_id;
                    continue;
                }
                if !auto_add {
                    return ParentResolution::Ignored;
                }

                ctx.replace_node_boxed(child_id, Box::new(create_auto_values_folder(segment.as_str())));
                return ParentResolution::Retry;
            }
            None => {
                if !auto_add {
                    return ParentResolution::Ignored;
                }

                ctx.add_child_tree(current, folder_path_tree(&parents[index..]), None);
                return ParentResolution::Retry;
            }
        }
    }

    ParentResolution::Ready {
        parent_id: current,
        leaf_name: leaf_name.clone(),
    }
}

fn apply_single_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    value: ParamValue,
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    match snapshot.find_child(parent_id, leaf_name.as_str()) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return ReceivedValueApplyResult::Ignored;
            };

            if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                if param_types_match(existing_value, &value) {
                    ctx.set_param_with_behaviour(existing_id, value, options.event_behaviour);
                    return ReceivedValueApplyResult::applied(false);
                } else if options.auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(
                            leaf_name.as_str(),
                            value,
                            Some(format!("Auto-created from {}", options.source_description)),
                        )),
                    );
                    return ReceivedValueApplyResult::applied(true);
                }
            } else if options.auto_add {
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_parameter_node(
                        leaf_name.as_str(),
                        value,
                        Some(format!("Auto-created from {}", options.source_description)),
                    )),
                );
                return ReceivedValueApplyResult::applied(true);
            }

            ReceivedValueApplyResult::applied(false)
        }
        None => {
            if !options.auto_add {
                return ReceivedValueApplyResult::Ignored;
            }

            ctx.add_child_boxed(
                parent_id,
                Box::new(create_parameter_node(
                    leaf_name.as_str(),
                    value,
                    Some(format!("Auto-created from {}", options.source_description)),
                )),
                None,
            );
            ReceivedValueApplyResult::applied(true)
        }
    }
}

fn apply_multi_value_message(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent_id: NodeId,
    leaf_name: String,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions<'_>,
) -> ReceivedValueApplyResult {
    let folder_id = match snapshot.find_child(parent_id, leaf_name.as_str()) {
        Some(existing_id) => {
            let Some(existing_snapshot) = snapshot.node(existing_id) else {
                return ReceivedValueApplyResult::Ignored;
            };
            if existing_snapshot.node_type == "folder" {
                existing_id
            } else {
                if !options.auto_add {
                    return ReceivedValueApplyResult::Ignored;
                }

                ctx.replace_node_boxed(existing_id, Box::new(create_auto_values_folder(leaf_name.as_str())));
                return ReceivedValueApplyResult::Retry;
            }
        }
        None => {
            if !options.auto_add {
                return ReceivedValueApplyResult::Ignored;
            }

            ctx.add_child_tree(parent_id, multi_value_folder_tree(leaf_name.as_str(), values), None);
            return ReceivedValueApplyResult::applied(true);
        }
    };

    ReceivedValueApplyResult::applied(sync_multi_value_folder(ctx, snapshot, folder_id, values, options))
}

fn sync_multi_value_folder(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    folder_id: NodeId,
    values: &[ParamValue],
    options: ReceivedValueApplyOptions<'_>,
) -> bool {
    let mut structure_changed = false;

    for (index, value) in values.iter().enumerate() {
        let label = indexed_value_label(index);
        match snapshot.find_child(folder_id, label.as_str()) {
            Some(existing_id) => {
                let Some(existing_snapshot) = snapshot.node(existing_id) else {
                    continue;
                };

                if let Some(existing_value) = existing_snapshot.param_value.as_ref() {
                    if param_types_match(existing_value, value) {
                        ctx.set_param_with_behaviour(existing_id, value.clone(), options.event_behaviour);
                    } else if options.auto_add {
                        ctx.replace_node_boxed(
                            existing_id,
                            Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                        );
                        structure_changed = true;
                    }
                } else if options.auto_add {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    );
                    structure_changed = true;
                }
            }
            None => {
                if !options.auto_add {
                    continue;
                }

                ctx.add_child_boxed(
                    folder_id,
                    Box::new(create_parameter_node(label.as_str(), value.clone(), None)),
                    None,
                );
                structure_changed = true;
            }
        }
    }

    for child_id in snapshot.child_ids(folder_id) {
        let Some(child_snapshot) = snapshot.node(child_id) else {
            continue;
        };
        if let Some(index) = indexed_value_label_index(child_snapshot.label.as_str()) {
            if index >= values.len() {
                ctx.edits.push(Edit::RemoveNode { node: child_id });
                structure_changed = true;
            }
        }
    }

    structure_changed
}

fn create_parameter_node(label: &str, value: ParamValue, description: Option<String>) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.description = description;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn create_auto_values_folder(label: &str) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn folder_path_tree(segments: &[String]) -> NodeTree {
    let (head, tail) = segments
        .split_first()
        .expect("folder_path_tree requires at least one segment");
    let mut tree = NodeTree::new(create_auto_values_folder(head.as_str()));
    if !tail.is_empty() {
        tree.push_child(folder_path_tree(tail));
    }
    tree
}

fn multi_value_folder_tree(leaf_name: &str, values: &[ParamValue]) -> NodeTree {
    let mut tree = NodeTree::new(create_auto_values_folder(leaf_name));
    for (index, value) in values.iter().enumerate() {
        let label = indexed_value_label(index);
        tree.push_child(NodeTree::new(create_parameter_node(label.as_str(), value.clone(), None)));
    }
    tree
}

fn indexed_value_label(index: usize) -> String {
    format!("{VALUE_LABEL_PREFIX}{}", index + 1)
}

fn indexed_value_label_index(label: &str) -> Option<usize> {
    let suffix = label.strip_prefix(VALUE_LABEL_PREFIX)?;
    suffix.parse::<usize>().ok()?.checked_sub(1)
}

fn param_types_match(lhs: &ParamValue, rhs: &ParamValue) -> bool {
    std::mem::discriminant(lhs) == std::mem::discriminant(rhs)
}
