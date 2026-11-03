use super::{AppControlNode, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::MediaEditResult,
};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, NodeMetaPatch},
    ui_sync::UiEditIntent,
};
use std::collections::HashSet;

const MAX_DEFINITIONS: usize = 4096;
fn node_id(engine: &AppEngine, application: ApplicationId) -> Result<NodeId, String> {
    let id = engine
        .node_id_by_uuid(application.0)
        .ok_or("application reference is missing")?;
    let Some(AppNode::Application(node)) = engine.nodes.get(id) else {
        return Err("reference is not an application".into());
    };
    if node.node_data().parent != Some(crate::media::authoring::manager(engine)?) {
        return Err("application is outside its catalog".into());
    }
    Ok(id)
}
pub fn inspect(engine: &AppEngine, application: ApplicationId) -> Result<ApplicationSnapshot, String> {
    let Some(AppNode::Application(node)) = engine.nodes.get(node_id(engine, application)?) else {
        unreachable!()
    };
    node.snapshot(engine)
}
fn create(
    engine: &mut AppEngine,
    label: Option<String>,
    definition: ApplicationDefinition,
    enabled: bool,
) -> Result<MediaEditResult, String> {
    let manager = crate::media::authoring::manager(engine)?;
    let count = engine
        .nodes
        .iter()
        .filter(|(_, node)| matches!(node, AppNode::Application(_)))
        .count();
    if count >= MAX_DEFINITIONS {
        return Err("application catalog exceeds 4096 definitions".into());
    }
    let label = labels::unique(engine, manager, label, "Application", &[])?;
    let tree = AppControlNode::tree(label, definition, enabled);
    let application = ApplicationId(tree.node.node_data().meta.uuid);
    engine.edits.push(Edit::AddUserItemTree {
        parent: manager,
        prev_sibling: engine.nodes.get(manager).unwrap().node_data().last_child,
        tree,
    });
    Ok(MediaEditResult::ApplicationCreated { application })
}
pub(crate) fn apply(engine: &mut AppEngine, edit: ApplicationEdit) -> Result<MediaEditResult, String> {
    match edit {
        ApplicationEdit::SetEnabled { application, enabled } => {
            let node = node_id(engine, application)?;
            if engine.nodes.get(node).unwrap().node_data().meta.enabled != enabled {
                engine.edits.push(Edit::PatchMeta {
                    node,
                    patch: NodeMetaPatch {
                        enabled: Some(enabled),
                        ..Default::default()
                    },
                });
            }
            Ok(MediaEditResult::Changed)
        }
        ApplicationEdit::DispatchCallbacks { ticket } => super::runtime::callbacks::dispatch(engine, ticket),
        ApplicationEdit::InspectCallbacks {} => {
            let manager = crate::media::authoring::manager(engine)?;
            let Some(AppNode::MediaManager(node)) = engine.nodes.get(manager) else {
                unreachable!()
            };
            Ok(MediaEditResult::ApplicationCallbacks {
                state: node.applications.callback_state(),
            })
        }
        ApplicationEdit::InspectReferences { applications } => {
            if applications.len() > 64 || applications.iter().collect::<HashSet<_>>().len() != applications.len() {
                return Err("application reference lookup requires at most 64 unique identities".into());
            }
            let references = applications
                .into_iter()
                .map(|application| {
                    let summary = node_id(engine, application)
                        .ok()
                        .and_then(|id| match engine.nodes.get(id) {
                            Some(AppNode::Application(node)) => Some(ApplicationSummary {
                                id: application,
                                label: node.node_data().meta.label.clone(),
                                enabled: node.node_data().effective_enabled,
                                configured: node.configured(),
                            }),
                            _ => None,
                        });
                    ApplicationReference { application, summary }
                })
                .collect();
            Ok(MediaEditResult::ApplicationReferences { references })
        }
        ApplicationEdit::InspectRuntime {} => {
            let manager = crate::media::authoring::manager(engine)?;
            let Some(AppNode::MediaManager(node)) = engine.nodes.get(manager) else {
                unreachable!()
            };
            Ok(MediaEditResult::ApplicationRuntime {
                snapshot: node.applications.snapshot(node.instance),
            })
        }
        ApplicationEdit::Create { label } => create(engine, label, Default::default(), true),
        ApplicationEdit::Inspect { application } => {
            if engine.node_id_by_uuid(application.0).is_none() {
                return Ok(MediaEditResult::ApplicationMissing { application });
            }
            Ok(MediaEditResult::Application {
                snapshot: inspect(engine, application)?,
            })
        }
        ApplicationEdit::InspectCatalog { search, start, count } => {
            if count == 0 || count > 128 || search.len() > 256 {
                return Err("application catalog requires 1..128 results and search at most 256 bytes".into());
            }
            let search = search.to_lowercase();
            let mut page = ApplicationPage {
                applications: vec![],
                total: 0,
                start,
            };
            let manager = crate::media::authoring::manager(engine)?;
            let mut next = engine.nodes.get(manager).unwrap().node_data().first_child;
            while let Some(id) = next {
                let node = engine.nodes.get(id).ok_or("application catalog changed")?;
                next = node.node_data().next_sibling;
                let AppNode::Application(node) = node else {
                    continue;
                };
                if !node.node_data().meta.label.to_lowercase().contains(&search) {
                    continue;
                }
                let position = page.total;
                page.total += 1;
                if position >= start && page.applications.len() < usize::from(count) {
                    page.applications.push(ApplicationSummary {
                        id: ApplicationId(node.node_data().meta.uuid),
                        label: node.node_data().meta.label.clone(),
                        enabled: node.node_data().effective_enabled,
                        configured: node.configured(),
                    });
                }
            }
            Ok(MediaEditResult::Applications { page })
        }
        ApplicationEdit::Configure {
            application,
            definition,
        } => {
            definition.validate()?;
            let node = node_id(engine, application)?;
            super::settings::configure(engine, node, definition)?;
            Ok(MediaEditResult::Changed)
        }
        ApplicationEdit::Rename { application, label } => {
            labels::rename(engine, node_id(engine, application)?, label)?;
            Ok(MediaEditResult::Changed)
        }
        ApplicationEdit::Remove { application } => {
            let node = node_id(engine, application)?;
            engine.edits.push(Edit::RemoveNode { node });
            Ok(MediaEditResult::Changed)
        }
        ApplicationEdit::Duplicate { application } => {
            let snapshot = inspect(engine, application)?;
            create(
                engine,
                Some(format!("{} copy", snapshot.label)),
                snapshot.definition,
                snapshot.enablement != ApplicationEnablement::Disabled,
            )
        }
    }
}
pub(crate) fn validate_associations(
    engine: &AppEngine,
    previous: &[ColumnApplication],
    associations: &[ColumnApplication],
) -> Result<(), String> {
    if associations.len() > 64 {
        return Err("a column admits at most 64 application associations".into());
    }
    let mut ids = HashSet::new();
    for association in associations {
        if !ids.insert(association.application) {
            return Err("column application associations must be unique".into());
        }
        // Deletion preserves authored references for Undo and truthful missing-state presentation.
        if !previous
            .iter()
            .any(|entry| entry.application == association.application)
        {
            node_id(engine, association.application)?;
        }
    }
    Ok(())
}
pub(crate) fn validate_project(engine: &AppEngine) -> Result<(), String> {
    let manager = crate::media::authoring::manager(engine)?;
    let mut count = 0;
    for (_, node) in engine.nodes.iter() {
        if let AppNode::Application(node) = node {
            count += 1;
            if count > MAX_DEFINITIONS || node.node_data().parent != Some(manager) {
                return Err("application catalog ownership/count is invalid".into());
            }
            node.definition(engine)?.validate()?;
        }
        if matches!(
            node,
            AppNode::ApplicationArgument(_) | AppNode::ApplicationEnvironment(_)
        ) && (!matches!(
            node.node_data().parent.and_then(|id| engine.nodes.get(id)),
            Some(AppNode::Application(_))
        ) || crate::parameters::has_domain_children(engine, node.id()))
        {
            return Err("app-control entry ownership is invalid".into());
        }
        if let AppNode::MatrixElement(node) = node
            && let crate::matrix::protocol::ElementDefinition::Column { definition } = node.definition()
        {
            validate_associations(engine, &definition.applications, &definition.applications)?;
        }
    }
    Ok(())
}
pub(crate) fn validate_ui_intent(engine: &AppEngine, intent: &UiEditIntent) -> Result<(), String> {
    let owned = |id| matches!(engine.nodes.get(id), Some(AppNode::Application(_)));
    let mut additions = std::collections::HashMap::new();
    let mut admit = |source: NodeId, parent: NodeId, moving: bool| -> Result<(), String> {
        let Some(node) = engine.nodes.get(source) else {
            return Ok(());
        };
        if moving && node.node_data().parent == Some(parent) {
            return Ok(());
        }
        let limit = match node {
            AppNode::Application(_) => MAX_DEFINITIONS,
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
                    .filter(|(_, candidate)| {
                        candidate.node_data().parent == Some(parent) && candidate.get_type() == node.get_type()
                    })
                    .count()
            });
        *count += 1;
        if *count > limit {
            return Err("app-control collection capacity exceeded".into());
        }
        Ok(())
    };
    match intent {
        UiEditIntent::MoveNode { node, new_parent, .. } => admit(*node, *new_parent, true)?,
        UiEditIntent::DuplicateNode { source, new_parent, .. } => admit(*source, *new_parent, false)?,
        UiEditIntent::DuplicateNodes { nodes, .. } => {
            for entry in nodes {
                admit(entry.source, entry.new_parent, false)?;
            }
        }
        _ => {}
    }
    let protected = match intent {
        UiEditIntent::MoveNode { node, new_parent, .. } => {
            owned(*node) && engine.nodes.get(*node).and_then(|node| node.node_data().parent) != Some(*new_parent)
        }
        UiEditIntent::DuplicateNode { source, new_parent, .. } => {
            owned(*source) && engine.nodes.get(*source).and_then(|node| node.node_data().parent) != Some(*new_parent)
        }
        UiEditIntent::DuplicateNodes { nodes, .. } => nodes.iter().any(|entry| {
            owned(entry.source)
                && engine.nodes.get(entry.source).and_then(|node| node.node_data().parent) != Some(entry.new_parent)
        }),
        _ => false,
    };
    if protected {
        Err("use Application operations to edit application ownership".into())
    } else {
        Ok(())
    }
}
