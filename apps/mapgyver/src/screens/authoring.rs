use golden_core::{
    edit::{Edit, NodeTree},
    events::CustomEvent,
    node::{Node, NodeId},
    parameter::ParameterEventBehaviour,
};
use serde_json::Value;

use super::{ScreenManagerNode, SurfaceNode, protocol::*};
use crate::app::{AppEngine, AppNode, labels};

pub fn initialize(engine: &mut AppEngine) {
    engine.add_node(ScreenManagerNode::new().into(), None);
}

pub(super) fn manager(engine: &AppEngine) -> Result<NodeId, String> {
    let mut child = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child);
    let mut found = None;
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing root child")?;
        if matches!(node, AppNode::ScreenManager(_)) {
            if found.is_some() {
                return Err("duplicate Screen Manager".into());
            }
            found = Some(id);
        }
        child = node.node_data().next_sibling;
    }
    found.ok_or_else(|| "Screen Manager is missing".into())
}

fn screen_id(engine: &AppEngine, screen: ScreenId) -> Result<NodeId, String> {
    let id = engine.node_id_by_uuid(screen.0).ok_or("screen reference is missing")?;
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        return Err("reference is not a Screen".into());
    };
    if node.node_data().parent != Some(manager(engine)?) {
        return Err("Screen is outside its manager".into());
    }
    Ok(id)
}

fn surface_id(engine: &AppEngine, reference: SurfaceReference) -> Result<NodeId, String> {
    let parent = screen_id(engine, reference.screen)?;
    let id = engine
        .node_id_by_uuid(reference.surface.0)
        .ok_or("surface reference is missing")?;
    let Some(AppNode::Surface(node)) = engine.nodes.get(id) else {
        return Err("reference is not a Surface".into());
    };
    if node.node_data().parent != Some(parent) {
        return Err("Surface belongs to another Screen".into());
    }
    Ok(id)
}

fn append(engine: &mut AppEngine, parent: NodeId, tree: NodeTree) {
    engine.edits.push(Edit::AddUserItemTree {
        parent,
        prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
        tree,
    });
}

pub fn apply_operation(engine: &mut AppEngine, operation: &str, payload: Value) -> Result<Value, String> {
    if operation != SCREEN_OPERATION {
        return Err("unsupported screen operation".into());
    }
    let edit: ScreenEdit = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let pending = engine.edits.pending.len();
    let result = match edit {
        ScreenEdit::InspectManager => ScreenEditResult::Manager {
            snapshot: inspect(engine)?,
        },
        ScreenEdit::InspectSurface { reference } => {
            let id = surface_id(engine, reference)?;
            let Some(AppNode::Surface(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            ScreenEditResult::Surface {
                snapshot: SurfaceSnapshot {
                    reference,
                    label: node.node_data().meta.label.clone(),
                    definition: node.definition(),
                },
            }
        }
        ScreenEdit::InspectScreen { screen } => {
            let id = screen_id(engine, screen)?;
            let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            ScreenEditResult::ScreenDefinition {
                screen,
                definition: node.definition(),
            }
        }
        ScreenEdit::CreateScreen { label } => {
            let parent = manager(engine)?;
            let label = labels::unique(engine, parent, label, "Screen", &[])?;
            let lookup = |id| engine.nodes.get(id).map(|node| node as &dyn Node);
            let context = golden_core::node::UserItemCreationContext::new(engine.root, &lookup);
            let tree = super::creation::tree(&context, label)?;
            let id = ScreenId(tree.node.node_data().meta.uuid);
            let reference = SurfaceReference {
                screen: id,
                surface: SurfaceId(
                    tree.children
                        .first()
                        .ok_or("Screen factory omitted initial Surface")?
                        .node
                        .node_data()
                        .meta
                        .uuid,
                ),
            };
            append(engine, parent, tree);
            ScreenEditResult::ScreenCreated {
                screen: id,
                surface: reference,
            }
        }
        ScreenEdit::ConfigureScreen { screen, definition } => {
            let node = screen_id(engine, screen)?;
            definition.validate()?;
            if definition.enabled
                && let Some(ScreenTarget::Spout { sender, .. }) = &definition.target
            {
                let mut child = engine
                    .nodes
                    .get(manager(engine)?)
                    .and_then(|node| node.node_data().first_child);
                while let Some(other) = child {
                    let candidate = engine.nodes.get(other).ok_or("Screen Manager child is missing")?;
                    child = candidate.node_data().next_sibling;
                    if other != node
                        && let AppNode::Screen(candidate) = candidate
                        && candidate.definition().enabled
                        && matches!(&candidate.definition().target, Some(ScreenTarget::Spout { sender: name, .. }) if name == sender)
                    {
                        return Err("enabled Screens must have distinct Shared Texture sender names".into());
                    }
                }
            }
            let Some(AppNode::Screen(current)) = engine.nodes.get(node) else {
                unreachable!()
            };
            let enabled = definition.enabled;
            engine.edits.push(Edit::SetParamsAtomic {
                values: current.parameter_values(definition),
                behaviour: ParameterEventBehaviour::Append,
            });
            engine.edits.push(Edit::PatchMeta {
                node,
                patch: golden_core::node::NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                },
            });
            ScreenEditResult::Changed
        }
        ScreenEdit::RenameScreen { screen, label } => {
            let node = screen_id(engine, screen)?;
            labels::rename(engine, node, label)?;
            ScreenEditResult::Changed
        }
        ScreenEdit::ReorderScreen { screen, after } => {
            let node = screen_id(engine, screen)?;
            let parent = manager(engine)?;
            let previous = after.map(|id| screen_id(engine, id)).transpose()?;
            reorder(engine, node, parent, previous)?;
            ScreenEditResult::Changed
        }
        ScreenEdit::RemoveScreen { screen } => {
            let node = screen_id(engine, screen)?;
            engine.edits.push(Edit::RemoveNode { node });
            ScreenEditResult::Changed
        }
        ScreenEdit::AddSurface { screen, label } => add_surface(engine, screen, label, SurfaceDefinition::default())?,
        ScreenEdit::DropSource {
            screen,
            surface,
            source,
        } => {
            let source = crate::media::resolve_source_node(engine, source)?;
            if let Some(surface) = surface {
                let reference = SurfaceReference { screen, surface };
                let node = surface_id(engine, reference)?;
                let Some(AppNode::Surface(current)) = engine.nodes.get(node) else {
                    unreachable!()
                };
                let mut definition = current.definition();
                definition.source = Some(source);
                let snapshot = SurfaceSnapshot {
                    reference,
                    label: current.node_data().meta.label.clone(),
                    definition,
                };
                engine.edits.push(Edit::SetParamsAtomic {
                    values: current.source_parameter_values(Some(source)).into(),
                    behaviour: ParameterEventBehaviour::Append,
                });
                ScreenEditResult::SurfaceChanged { snapshot }
            } else {
                add_surface(
                    engine,
                    screen,
                    None,
                    SurfaceDefinition {
                        source: Some(source),
                        ..Default::default()
                    },
                )?
            }
        }
        ScreenEdit::ConfigureSurface { reference, definition } => {
            let node = surface_id(engine, reference)?;
            definition.validate()?;
            let Some(AppNode::Surface(current)) = engine.nodes.get(node) else {
                unreachable!()
            };
            if definition.source != current.definition().source
                && let Some(source) = definition.source
            {
                crate::media::channel(engine, source)?;
            }
            engine.edits.push(Edit::SetParamsAtomic {
                values: current.parameter_values(definition),
                behaviour: ParameterEventBehaviour::Append,
            });
            ScreenEditResult::Changed
        }
        ScreenEdit::RenameSurface { reference, label } => {
            let node = surface_id(engine, reference)?;
            labels::rename(engine, node, label)?;
            ScreenEditResult::Changed
        }
        ScreenEdit::EditSurfaceGeometry { reference, edit } => {
            let node = surface_id(engine, reference)?;
            let Some(AppNode::Surface(current)) = engine.nodes.get(node) else {
                unreachable!()
            };
            let mut definition = current.definition();
            edit.apply(&mut definition)?;
            let snapshot = SurfaceSnapshot {
                reference,
                label: current.node_data().meta.label.clone(),
                definition: definition.clone(),
            };
            engine.edits.push(Edit::SetParamsAtomic {
                values: current.parameter_values(definition),
                behaviour: ParameterEventBehaviour::Append,
            });
            ScreenEditResult::SurfaceChanged { snapshot }
        }
        ScreenEdit::ReorderSurface { reference, after } => {
            let node = surface_id(engine, reference)?;
            let parent = screen_id(engine, reference.screen)?;
            let previous = after
                .map(|surface| surface_id(engine, SurfaceReference { surface, ..reference }))
                .transpose()?;
            reorder(engine, node, parent, previous)?;
            ScreenEditResult::Changed
        }
        ScreenEdit::RemoveSurface { reference } => {
            let node = surface_id(engine, reference)?;
            engine.edits.push(Edit::RemoveNode { node });
            ScreenEditResult::Changed
        }
    };
    if engine.edits.pending.len() != pending {
        let event = CustomEvent::from_latest_payload(SCREEN_NAMESPACE, None, &()).map_err(|error| error.to_string())?;
        engine.edits.push(Edit::EmitCustomEvent { event });
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}

fn add_surface(
    engine: &mut AppEngine,
    screen: ScreenId,
    label: Option<String>,
    definition: SurfaceDefinition,
) -> Result<ScreenEditResult, String> {
    let parent = screen_id(engine, screen)?;
    let surface = SurfaceNode::from_definition(labels::unique(engine, parent, label, "Surface", &[])?, definition);
    let reference = SurfaceReference {
        screen,
        surface: SurfaceId(surface.node_data().meta.uuid),
    };
    append(engine, parent, NodeTree::new(surface));
    Ok(ScreenEditResult::SurfaceCreated { reference })
}

fn reorder(engine: &mut AppEngine, node: NodeId, parent: NodeId, previous: Option<NodeId>) -> Result<(), String> {
    if previous == Some(node) {
        return Err("item cannot follow itself".into());
    }
    engine.edits.push(Edit::MoveNode {
        node,
        new_parent: parent,
        new_prev_sibling: previous,
    });
    Ok(())
}

pub fn inspect(engine: &AppEngine) -> Result<ScreenManagerSnapshot, String> {
    let manager = manager(engine)?;
    let mut child = engine.nodes.get(manager).and_then(|node| node.node_data().first_child);
    let mut screens = Vec::new();
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing manager child")?;
        if let AppNode::Screen(screen) = node {
            let owner = ScreenId(screen.node_data().meta.uuid);
            let mut surface = screen.node_data().first_child;
            let mut surfaces = Vec::new();
            while let Some(id) = surface {
                let child = engine.nodes.get(id).ok_or("missing Screen child")?;
                surface = child.node_data().next_sibling;
                if crate::parameters::is_parameter_folder(child) {
                    continue;
                }
                let AppNode::Surface(node) = child else {
                    return Err("Screen children must be Surfaces".into());
                };
                surfaces.push(SurfaceSnapshot {
                    reference: SurfaceReference {
                        screen: owner,
                        surface: SurfaceId(node.node_data().meta.uuid),
                    },
                    label: node.node_data().meta.label.clone(),
                    definition: node.definition(),
                });
            }
            screens.push(ScreenSnapshot {
                id: owner,
                label: screen.node_data().meta.label.clone(),
                definition: screen.definition(),
                surfaces,
            });
        }
        child = node.node_data().next_sibling;
    }
    Ok(ScreenManagerSnapshot { screens })
}

pub fn validate_project(engine: &AppEngine) -> Result<(), String> {
    let manager = manager(engine)?;
    let mut senders = std::collections::HashSet::new();
    for (_, node) in engine.nodes.iter() {
        match node {
            AppNode::ScreenManager(node) => {
                if node.node_data().parent != Some(engine.root) {
                    return Err("Screen Manager must be a root child".into());
                }
            }
            AppNode::Screen(node) => {
                if node.node_data().parent != Some(manager) {
                    return Err("Screen is outside its manager".into());
                }
                let definition = node.definition();
                definition.validate()?;
                if definition.enabled
                    && let Some(ScreenTarget::Spout { sender, .. }) = definition.target
                    && !senders.insert(sender)
                {
                    return Err("enabled Screens must have distinct Shared Texture sender names".into());
                }
                let mut child = node.node_data().first_child;
                while let Some(id) = child {
                    let node = engine.nodes.get(id).ok_or("missing Screen child")?;
                    child = node.node_data().next_sibling;
                    if crate::parameters::is_parameter_folder(node) {
                        continue;
                    }
                    let AppNode::Surface(_) = node else {
                        return Err("Screen children must be Surfaces".into());
                    };
                }
            }
            AppNode::Surface(node) => {
                node.definition().validate()?;
                let parent = node.node_data().parent.and_then(|id| engine.nodes.get(id));
                if !matches!(parent, Some(AppNode::Screen(_))) {
                    return Err("Surface owner is not a Screen".into());
                }
                // A deleted or disconnected source remains an authored reference, so Undo/relink
                // can restore it. Resolution observes availability without mutating the project.
            }
            _ => {}
        }
    }
    Ok(())
}
