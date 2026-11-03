//! Configure reconciles a bounded typed subtree; retained entries keep their document identities.
use super::{ArgumentNode, EnvironmentNode, protocol::ApplicationDefinition};
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::{Edit, NodeTree, UserItemTreeInsertion},
    node::NodeId,
    parameter::ParameterEventBehaviour,
};

pub(super) fn configure(
    engine: &mut AppEngine,
    parent: NodeId,
    definition: ApplicationDefinition,
) -> Result<(), String> {
    definition.validate()?;
    let Some(AppNode::Application(node)) = engine.nodes.get(parent) else {
        return Err("app control is missing".into());
    };
    let mut values = node.configuration_values(&definition);
    let mut arguments = Vec::new();
    let mut environments = Vec::new();
    for id in crate::parameters::domain_children(engine, parent)? {
        match engine.nodes.get(id) {
            Some(AppNode::ApplicationArgument(_)) => arguments.push(id),
            Some(AppNode::ApplicationEnvironment(_)) => environments.push(id),
            _ => return Err("unexpected app-control child".into()),
        }
    }
    let mut added = Vec::new();
    let argument_count = definition.arguments.len();
    let environment_count = definition.environment.as_ref().map_or(0, Vec::len);
    for (index, value) in definition.arguments.into_iter().enumerate() {
        if let Some(id) = arguments.get(index) {
            let Some(AppNode::ApplicationArgument(node)) = engine.nodes.get(*id) else {
                unreachable!()
            };
            values.push(node.parameter_value(value));
        } else {
            added.push(UserItemTreeInsertion::new(
                parent,
                NodeTree::new(ArgumentNode::from_value(value)),
            ));
        }
    }
    for (index, (name, value)) in definition.environment.into_iter().flatten().enumerate() {
        if let Some(id) = environments.get(index) {
            let Some(AppNode::ApplicationEnvironment(node)) = engine.nodes.get(*id) else {
                unreachable!()
            };
            values.extend(node.parameter_values(name, value));
        } else {
            added.push(UserItemTreeInsertion::new(
                parent,
                NodeTree::new(EnvironmentNode::from_entry(name, value)),
            ));
        }
    }
    // Remove surplus entries before validating one final Parameter proposal. No caller sees
    // intermediate authored state: the owning operation/history transaction encloses this slice.
    for node in arguments
        .into_iter()
        .skip(argument_count)
        .chain(environments.into_iter().skip(environment_count))
    {
        engine.edits.push(Edit::RemoveNode { node });
    }
    engine.edits.push(Edit::SetParamsAtomic {
        values,
        behaviour: ParameterEventBehaviour::Append,
    });
    if !added.is_empty() {
        engine.edits.push(Edit::AddUserItemTrees { items: added });
    }
    Ok(())
}
