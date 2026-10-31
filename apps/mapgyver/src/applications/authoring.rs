use super::{ApplicationNode, protocol::*};
use crate::{
    app::{AppEngine, AppNode, labels},
    media::protocol::MediaEditResult,
};
use golden_core::{
    edit::{Edit, NodeTree},
    node::{Node, NodeId},
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
    Ok(node.snapshot())
}
fn create(
    engine: &mut AppEngine,
    label: Option<String>,
    definition: ApplicationDefinition,
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
    let node = ApplicationNode::new(label, definition);
    let application = ApplicationId(node.node_data().meta.uuid);
    engine.edits.push(Edit::AddNodeTree {
        parent: manager,
        prev_sibling: engine.nodes.get(manager).unwrap().node_data().last_child,
        tree: NodeTree::new(node),
    });
    Ok(MediaEditResult::ApplicationCreated { application })
}
pub(crate) fn apply(engine: &mut AppEngine, edit: ApplicationEdit) -> Result<MediaEditResult, String> {
    match edit {
        ApplicationEdit::Create { label } => create(engine, label, Default::default()),
        ApplicationEdit::Inspect { application } => Ok(MediaEditResult::Application {
            snapshot: inspect(engine, application)?,
        }),
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
                        configured: node.definition().executable.is_some(),
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
            engine.edits.push(Edit::CallNodeMutation {
                node,
                needs_tree_snapshot: false,
                callback: Box::new(move |node, _| {
                    node.as_any_mut()
                        .downcast_mut::<ApplicationNode>()
                        .ok_or("application type changed")?
                        .set_definition(definition);
                    Ok(())
                }),
            });
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
            create(engine, Some(format!("{} copy", snapshot.label)), snapshot.definition)
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
            if count > MAX_DEFINITIONS
                || node.node_data().parent != Some(manager)
                || node.node_data().first_child.is_some()
            {
                return Err("application catalog ownership/count is invalid".into());
            }
            node.definition().validate()?;
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
    let protected = match intent {
        UiEditIntent::MoveNode { node, new_parent, .. } => owned(*node) || owned(*new_parent),
        UiEditIntent::DuplicateNode { source, new_parent, .. } => owned(*source) || owned(*new_parent),
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items,
            dependent_items,
        } => {
            nodes.iter().any(|entry| owned(entry.source) || owned(entry.new_parent))
                || created_items.iter().any(|entry| owned(entry.parent))
                || dependent_items.iter().any(|entry| owned(entry.parent))
        }
        UiEditIntent::CreateUserItem { parent, .. } => owned(*parent),
        _ => false,
    };
    if protected {
        Err("use Application operations to edit application ownership".into())
    } else {
        Ok(())
    }
}
