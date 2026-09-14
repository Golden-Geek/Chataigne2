use golden_core::{
    edit::NodeTree,
    node::{DeclId, Node, NodeId, PresentationHint},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints},
    process_ctx::ProcessTreeSnapshot,
};

use crate::app::systems_alchemist_formula::{
    PROPERTY_FOLDER_NODE_TYPE, PROPERTY_MANAGER_NODE_TYPE, PROPERTY_NODE_TYPE,
};
use crate::app::{ConditionManager, FilterChainManager, InputsManager, OutputsManager};

use super::{locked_instance_permissions, processor_surface_decl_id_for_source};

#[derive(Clone, Debug)]
enum SurfaceKind {
    Parameter {
        value: ParamValue,
        constraints: Option<Box<ParameterConstraints>>,
    },
    Condition,
    Filter,
    Input,
    Output,
    Folder(Vec<ProcessorSurfaceTemplate>),
}

/// The small, cloneable Formula property description used for detached creation.
/// Every materialization constructs fresh nodes and UUIDs for one processor.
#[derive(Clone, Debug)]
pub(super) struct ProcessorSurfaceTemplate {
    label: String,
    decl_id: DeclId,
    presentation: PresentationHint,
    kind: SurfaceKind,
}

impl ProcessorSurfaceTemplate {
    pub(super) fn from_snapshot(snapshot: &ProcessTreeSnapshot, source: NodeId) -> Option<Self> {
        let source_node = snapshot.node(source)?;
        let kind = match source_node.node_type.as_str() {
            PROPERTY_NODE_TYPE => {
                if !property_is_exposed(snapshot, source) {
                    return None;
                }
                let value_id = snapshot.find_child_by_decl_id(source, "value")?;
                SurfaceKind::Parameter {
                    value: snapshot.node(value_id)?.param_value.clone()?,
                    constraints: snapshot.node(value_id)?.param_constraints.clone().map(Box::new),
                }
            }
            PROPERTY_MANAGER_NODE_TYPE => {
                if !property_is_exposed(snapshot, source) {
                    return None;
                }
                let role = snapshot
                    .find_child_by_decl_id(source, "role")
                    .and_then(|id| snapshot.node(id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_str)?;
                match role.as_str() {
                    "condition" => SurfaceKind::Condition,
                    "filter" => SurfaceKind::Filter,
                    "input" => SurfaceKind::Input,
                    "output" => SurfaceKind::Output,
                    _ => return None,
                }
            }
            PROPERTY_FOLDER_NODE_TYPE => SurfaceKind::Folder(
                snapshot
                    .child_ids(source)
                    .into_iter()
                    .filter_map(|child| Self::from_snapshot(snapshot, child))
                    .collect(),
            ),
            _ => return None,
        };
        Some(Self {
            label: source_node.label.clone(),
            decl_id: DeclId(processor_surface_decl_id_for_source(
                source_node.uuid,
                &source_node.tags,
            )),
            presentation: source_node.presentation.clone(),
            kind,
        })
    }

    pub(super) fn into_tree(self) -> NodeTree {
        let mut tree = match self.kind {
            SurfaceKind::Parameter { value, constraints } => {
                let mut parameter = Parameter::new(
                    &self.label,
                    value,
                    ParameterChangeCheck::ValueChange,
                );
                if let Some(constraints) = constraints {
                    parameter.constraints = *constraints;
                }
                NodeTree::new(parameter)
            }
            SurfaceKind::Condition => NodeTree::new(ConditionManager::new()),
            SurfaceKind::Filter => NodeTree::new(FilterChainManager::new()),
            SurfaceKind::Input => NodeTree::new(InputsManager::new()),
            SurfaceKind::Output => NodeTree::new(OutputsManager::new()),
            SurfaceKind::Folder(children) => {
                let mut folder = super::StateProcessorFolder::new();
                folder.node_data_mut().meta.user_permissions = locked_instance_permissions();
                let mut tree = NodeTree::new(folder);
                for child in children {
                    tree.push_child(child.into_tree());
                }
                tree
            }
        };
        let meta = &mut tree.node.node_data_mut().meta;
        meta.label = self.label;
        meta.decl_id = self.decl_id;
        meta.presentation.default_color = self.presentation.color.or(self.presentation.default_color);
        tree
    }
}

pub(super) fn processor_surface_child_tree(
    snapshot: &ProcessTreeSnapshot,
    source: NodeId,
) -> Option<NodeTree> {
    ProcessorSurfaceTemplate::from_snapshot(snapshot, source).map(ProcessorSurfaceTemplate::into_tree)
}

fn property_is_exposed(snapshot: &ProcessTreeSnapshot, source: NodeId) -> bool {
    snapshot
        .find_child_by_decl_id(source, "exposed")
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(true)
}
