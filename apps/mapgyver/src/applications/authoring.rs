//! Owner-based app-control operations. Authored settings always belong to a Column or Cell.
use super::{AppControlNode, protocol::*};
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::MediaEditResult,
};
use golden_core::{
    node::{Node, NodeId},
    ui_sync::UiEditIntent,
};

fn node_id(engine: &AppEngine, application: ApplicationId) -> Result<NodeId, String> {
    let owner = engine
        .node_id_by_uuid(application.0)
        .ok_or("App-control owner is missing")?;
    let settings = match engine.nodes.get(owner) {
        Some(AppNode::MatrixCell(node)) => node.application_node(),
        Some(AppNode::MatrixColumn(node)) => node.application_node(),
        _ => return Err("App control requires a Column or Cell owner identity".into()),
    }
    .ok_or("Declared app-control settings are missing")?;
    if !matches!(engine.nodes.get(settings), Some(AppNode::Application(_))) {
        return Err("App-control settings type is invalid".into());
    }
    Ok(settings)
}
pub(crate) fn control_node(engine: &AppEngine, application: ApplicationId) -> Option<&AppControlNode> {
    match engine.nodes.get(node_id(engine, application).ok()?) {
        Some(AppNode::Application(node)) => Some(node),
        _ => None,
    }
}
pub fn inspect(engine: &AppEngine, application: ApplicationId) -> Result<ApplicationSnapshot, String> {
    control_node(engine, application)
        .ok_or("App-control owner is missing or invalid")?
        .snapshot(engine)
}
pub(crate) fn apply(engine: &mut AppEngine, edit: ApplicationEdit) -> Result<MediaEditResult, String> {
    match edit {
        ApplicationEdit::Configure {
            application,
            definition,
        } => {
            let node = node_id(engine, application)?;
            super::settings::configure(engine, node, definition)?;
            Ok(MediaEditResult::Changed)
        }
        ApplicationEdit::Inspect { application } => {
            if engine.node_id_by_uuid(application.0).is_none() {
                return Ok(MediaEditResult::ApplicationMissing { application });
            }
            Ok(MediaEditResult::Application {
                snapshot: inspect(engine, application)?,
            })
        }
        ApplicationEdit::DispatchCallbacks { ticket } => super::runtime::callbacks::dispatch(engine, ticket),
        ApplicationEdit::InspectCallbacks {} | ApplicationEdit::InspectRuntime {} => {
            let manager = crate::media::authoring::manager(engine)?;
            let Some(AppNode::MediaManager(node)) = engine.nodes.get(manager) else {
                unreachable!()
            };
            if matches!(edit, ApplicationEdit::InspectCallbacks {}) {
                Ok(MediaEditResult::ApplicationCallbacks {
                    state: node.applications.callback_state(),
                })
            } else {
                Ok(MediaEditResult::ApplicationRuntime {
                    snapshot: node.applications.snapshot(node.instance),
                })
            }
        }
    }
}
pub(crate) fn validate_project(engine: &AppEngine) -> Result<(), String> {
    for (_, node) in engine.nodes.iter() {
        if let AppNode::Application(settings) = node {
            let valid = match settings.node_data().parent.and_then(|id| engine.nodes.get(id)) {
                Some(AppNode::MatrixCell(owner)) => owner.application_node() == Some(settings.id()),
                Some(AppNode::MatrixColumn(owner)) => owner.application_node() == Some(settings.id()),
                _ => false,
            };
            if !valid {
                return Err(
                    "Standalone app-control definitions are unsupported; settings require a Column or Cell".into(),
                );
            }
            settings.definition(engine)?.validate()?;
        }
        if matches!(
            node,
            AppNode::ApplicationArgument(_) | AppNode::ApplicationEnvironment(_)
        ) && (!matches!(
            node.node_data().parent.and_then(|id| engine.nodes.get(id)),
            Some(AppNode::Application(_))
        ) || crate::parameters::has_domain_children(engine, node.id()))
        {
            return Err("App-control entry ownership is invalid".into());
        }
    }
    Ok(())
}
pub(crate) fn validate_ui_intent(engine: &AppEngine, intent: &UiEditIntent) -> Result<(), String> {
    let root = |id| matches!(engine.nodes.get(id), Some(AppNode::Application(_)));
    let protected = match intent {
        UiEditIntent::RemoveNode { node } | UiEditIntent::MoveNode { node, .. } => root(*node),
        UiEditIntent::RemoveNodes { nodes } => nodes.iter().any(|node| root(*node)),
        UiEditIntent::DuplicateNode { source, .. } => root(*source),
        UiEditIntent::DuplicateNodes { nodes, .. } => nodes.iter().any(|node| root(node.source)),
        _ => false,
    };
    if protected {
        return Err("App-control settings are declared children and follow their owning Column or Cell".into());
    }
    let mut additions = std::collections::HashMap::new();
    let mut admit = |source: NodeId, parent: NodeId, moving: bool| -> Result<(), String> {
        let Some(node) = engine.nodes.get(source) else {
            return Ok(());
        };
        if moving && node.node_data().parent == Some(parent) {
            return Ok(());
        }
        let limit = match node {
            AppNode::ApplicationArgument(_) => 256,
            AppNode::ApplicationEnvironment(_) => 128,
            _ => return Ok(()),
        };
        let count = additions
            .entry((parent, node.get_type().to_owned()))
            .or_insert_with(|| {
                engine
                    .nodes
                    .iter()
                    .filter(|(_, child)| {
                        child.node_data().parent == Some(parent) && child.get_type() == node.get_type()
                    })
                    .count()
            });
        *count += 1;
        if *count > limit {
            return Err("App-control collection capacity exceeded".into());
        }
        Ok(())
    };
    match intent {
        UiEditIntent::MoveNode { node, new_parent, .. } => admit(*node, *new_parent, true)?,
        UiEditIntent::DuplicateNode { source, new_parent, .. } => admit(*source, *new_parent, false)?,
        UiEditIntent::DuplicateNodes { nodes, .. } => {
            for node in nodes {
                admit(node.source, node.new_parent, false)?;
            }
        }
        _ => {}
    }
    Ok(())
}
