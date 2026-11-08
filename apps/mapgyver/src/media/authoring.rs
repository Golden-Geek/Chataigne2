use golden_core::app::add_default_project_nodes;
use golden_core::edit::{Edit, NodeTree};
use golden_core::node::{Node, NodeId};
use serde_json::Value;

use super::protocol::*;
use super::{ChannelNode, MediaManagerNode, MediaNode};
use crate::app::labels::rename;
pub(super) use crate::app::labels::unique as label_with_reserved;
use crate::app::{AppEngine, AppNode};

const MANAGER_DECL: &str = "mapgyver_media_manager";

pub fn initialize(engine: &mut AppEngine) {
    add_default_project_nodes(engine);
    engine.add_node(MediaManagerNode::new().into(), None);
}

pub(crate) fn manager(engine: &AppEngine) -> Result<NodeId, String> {
    let mut child = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child);
    let mut found = None;
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("missing root child")?;
        if node.node_data().meta.decl_id.0 == MANAGER_DECL && matches!(node, AppNode::MediaManager(_)) {
            if found.is_some() {
                return Err("duplicate Medias".into());
            }
            found = Some(id);
        }
        child = node.node_data().next_sibling;
    }
    found.ok_or_else(|| "Medias is missing".into())
}

pub(crate) fn project_instance(engine: &AppEngine) -> Result<uuid::Uuid, String> {
    match engine.nodes.get(manager(engine)?) {
        Some(AppNode::MediaManager(manager)) => Ok(manager.instance),
        _ => Err("Medias is missing".into()),
    }
}

pub(crate) fn media(engine: &AppEngine, media: MediaId) -> Result<(NodeId, MediaKind), String> {
    let id = engine.node_id_by_uuid(media.0).ok_or("media reference is missing")?;
    let node = engine.nodes.get(id).ok_or("media is missing")?;
    let kind = match node {
        AppNode::Media(node) => node.kind(),
        AppNode::ReferenceMedia(_) => MediaKind::Reference,
        _ => return Err("reference is not media".into()),
    };
    if !super::folders::contains_parent(engine, node.node_data().parent, manager(engine)?) {
        return Err("media is outside its manager".into());
    }
    Ok((id, kind))
}

pub(super) fn channel_id(engine: &AppEngine, reference: ChannelReference) -> Result<NodeId, String> {
    let (parent, kind) = media(engine, reference.media)?;
    let id = engine
        .node_id_by_uuid(reference.channel.0)
        .ok_or("channel reference is missing")?;
    let node = engine.nodes.get(id).ok_or("channel is missing")?;
    if node.node_data().parent != Some(parent) {
        return Err("channel belongs to another media".into());
    }
    let actual_kind = match node {
        AppNode::MediaChannel(node) => node.kind(),
        AppNode::ReferenceOutput(_) => MediaKind::Reference,
        _ => return Err("reference is not a channel".into()),
    };
    if actual_kind != kind {
        return Err("channel source does not match its media kind".into());
    }
    Ok(id)
}

pub fn channel(engine: &AppEngine, reference: ChannelReference) -> Result<ChannelSnapshot, String> {
    if !super::visibility::exposed(engine, reference) {
        return Err("Matrix output is not exposed".into());
    }
    channel_internal(engine, reference)
}

pub(crate) fn channel_internal(engine: &AppEngine, reference: ChannelReference) -> Result<ChannelSnapshot, String> {
    if media(engine, reference.media)?.1 == MediaKind::Reference {
        return super::reference::channel(engine, reference);
    }
    let id = channel_id(engine, reference)?;
    let AppNode::MediaChannel(node) = engine.nodes.get(id).expect("validated channel") else {
        unreachable!()
    };
    Ok(ChannelSnapshot {
        reference,
        label: node.node_data().meta.label.clone(),
        definition: effective_definition(engine, node.as_ref())?,
    })
}

fn label(engine: &AppEngine, parent: NodeId, requested: Option<String>, fallback: &str) -> Result<String, String> {
    label_with_reserved(engine, parent, requested, fallback, &[])
}

pub fn apply_operation(engine: &mut AppEngine, operation: &str, payload: Value) -> Result<Value, String> {
    if operation != MEDIA_OPERATION {
        return Err("unsupported media operation".into());
    }
    let edit: MediaEdit = serde_json::from_value(payload).map_err(|error| error.to_string())?;
    if let MediaEdit::ExecuteCommand { command } = edit {
        let operation = super::commands::operation(engine, command)?;
        return apply_operation(
            engine,
            MEDIA_OPERATION,
            serde_json::to_value(operation).map_err(|error| error.to_string())?,
        );
    }
    if let MediaEdit::Applications {
        edit: operation @ crate::applications::protocol::ApplicationEdit::DispatchCallbacks { .. },
    } = edit
    {
        return serde_json::to_value(crate::applications::apply(engine, operation)?).map_err(|error| error.to_string());
    }
    if let MediaEdit::Matrix {
        media,
        edit:
            operation @ (crate::matrix::protocol::MatrixEdit::LaunchColumn { .. }
            | crate::matrix::protocol::MatrixEdit::ClearPlayback),
    } = edit
    {
        return serde_json::to_value(crate::matrix::apply(engine, media, operation)?)
            .map_err(|error| error.to_string());
    }
    let pending_before = engine.edits.pending.len();
    let live_geometry = matches!(
        &edit,
        MediaEdit::Composition {
            edit: super::composition::protocol::CompositionEdit::EditInstanceGeometry { .. },
            ..
        } | MediaEdit::Matrix {
            edit: crate::matrix::protocol::MatrixEdit::EditLayerGeometry { .. }
                | crate::matrix::protocol::MatrixEdit::EditCellGeometry { .. },
            ..
        }
    );
    let result = match edit {
        MediaEdit::InspectTargetPicker { query } => MediaEditResult::TargetPicker {
            page: super::target_picker::read(engine, query)?,
        },
        MediaEdit::SelectTargetPickerChoice { parameter, target } => {
            super::target_picker::select(engine, parameter, target)?;
            MediaEditResult::Changed
        }
        MediaEdit::CreateReference { target, label } => {
            super::reference::create(engine, manager(engine)?, target, label)?
        }
        MediaEdit::ConfigureReference { media, target } => {
            super::reference::configure(engine, media, target)?;
            MediaEditResult::Changed
        }
        MediaEdit::SelectReferenceChannel { media: owner, channel } => {
            let (id, _) = media(engine, owner)?;
            let Some(AppNode::ReferenceMedia(node)) = engine.nodes.get(id) else {
                return Err("media is not Reference".into());
            };
            engine.edits.push(Edit::SetParam {
                node: node.channel_parameter(),
                value: golden_core::parameter::ParamValue::Reference(
                    channel.map_or_else(golden_core::node::NodeReference::empty, |id| {
                        golden_core::node::NodeReference::new(id.0)
                    }),
                ),
                behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
            });
            MediaEditResult::Changed
        }
        MediaEdit::Applications { edit } => crate::applications::apply(engine, edit)?,
        MediaEdit::InspectChoices { query } => MediaEditResult::Choices {
            page: super::catalog::choices(engine, query)?,
        },
        MediaEdit::InspectChannels { query } => MediaEditResult::Channels {
            page: super::catalog::channels(engine, query)?,
        },
        MediaEdit::InspectMediaPreview { query } => MediaEditResult::MediaPreview {
            page: super::preview_selection::inspect(engine, query)?,
        },
        MediaEdit::InspectCompositionEditor { query } => MediaEditResult::CompositionEditor {
            page: super::composition::query::inspect(engine, query)?,
        },
        MediaEdit::InspectMatrixEditor { query } => MediaEditResult::MatrixEditor {
            page: crate::matrix::inspect_editor(engine, query)?,
        },
        MediaEdit::InspectMatrixCells { query } => MediaEditResult::MatrixCellLayout {
            page: crate::matrix::inspect_cells(engine, query)?,
        },
        MediaEdit::InspectSpatialEditor { query } => MediaEditResult::SpatialEditor {
            binding: super::spatial::inspect(engine, query)?,
        },
        MediaEdit::Matrix { media, edit } => crate::matrix::apply(engine, media, edit)?,
        MediaEdit::Composition { reference, edit } => super::composition::apply(engine, reference, edit)?,
        MediaEdit::ImportPicture { locator, target, label } => {
            super::importing::picture(engine, locator, target, label)?
        }
        MediaEdit::InspectManager => MediaEditResult::Manager {
            snapshot: super::catalog::inspect(engine)?,
        },
        MediaEdit::InspectCommandCatalog => MediaEditResult::CommandCatalog {
            commands: super::commands::catalog(),
        },
        MediaEdit::CreateCommand {
            command_kind,
            label: requested,
        } => {
            let parent = manager(engine)?;
            let node = super::commands::MediaCommandNode::from_kind(
                label(engine, parent, requested, command_kind.label())?,
                command_kind,
            );
            let command = node.node_data().meta.uuid;
            engine.edits.push(Edit::AddUserItemTree {
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
                tree: NodeTree::new(node),
            });
            MediaEditResult::CommandCreated { command }
        }
        MediaEdit::ExecuteCommand { .. } => unreachable!("command was projected above"),
        MediaEdit::CreateScript { label: requested } => {
            let parent = manager(engine)?;
            let node = golden_core::script::ScriptNode::new(
                label(engine, parent, requested, "Media Script")?,
                super::script_templates::config(),
            );
            let script = node.node_data().meta.uuid;
            engine.edits.push(Edit::AddUserItemTree {
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
                tree: NodeTree::new(node),
            });
            MediaEditResult::ScriptCreated { script }
        }
        MediaEdit::ImportAsset {
            locator,
            label: requested,
        } => {
            let parent = manager(engine)?;
            let fallback = locator_label(&locator);
            let node =
                super::assets::AssetNode::from_locator(label(engine, parent, requested, fallback)?, Some(locator));
            let asset = node.id();
            engine.edits.push(Edit::AddUserItemTree {
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
                tree: NodeTree::new(node),
            });
            MediaEditResult::AssetCreated { asset }
        }
        MediaEdit::RelinkAsset { asset, locator } => {
            let id = super::assets::node_id(engine, asset)?;
            let Some(AppNode::MediaAsset(node)) = engine.nodes.get(id) else {
                unreachable!()
            };
            for edit in node.locator_edits(locator) {
                engine.edits.push(edit);
            }
            MediaEditResult::Changed
        }
        MediaEdit::ReloadAsset { asset } => {
            super::assets::asset(engine, asset)?;
            let id = super::assets::node_id(engine, asset)?;
            let Some(AppNode::MediaAsset(node)) = engine.nodes.get_mut(id) else {
                unreachable!("validated asset");
            };
            node.reload();
            MediaEditResult::Changed
        }
        MediaEdit::RemoveAsset { asset } => {
            let id = super::assets::node_id(engine, asset)?;
            engine.edits.push(Edit::RemoveNode { node: id });
            MediaEditResult::Changed
        }
        MediaEdit::InspectAsset { asset } => MediaEditResult::Asset {
            record: super::assets::asset(engine, asset)?,
        },
        MediaEdit::CreateMedia {
            media_kind,
            label: requested,
        } => {
            let parent = manager(engine)?;
            create_media(engine, parent, media_kind, requested)?
        }
        MediaEdit::CreateMediaInFolder {
            folder,
            media_kind,
            label,
        } => {
            let parent = super::folders::parent(engine, Some(folder))?;
            create_media(engine, parent, media_kind, label)?
        }
        MediaEdit::CreateFolder {
            parent,
            label: requested,
        } => {
            let parent = super::folders::parent(engine, parent)?;
            let folder = super::MediaFolderNode::new(label(engine, parent, requested, "Folder")?);
            let uuid = folder.node_data().meta.uuid;
            engine.edits.push(Edit::AddUserItemTree {
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
                tree: NodeTree::new(folder),
            });
            MediaEditResult::FolderCreated { folder: uuid }
        }
        MediaEdit::RenameFolder { folder, label } => {
            let node = super::folders::parent(engine, Some(folder))?;
            rename(engine, node, label)?;
            MediaEditResult::Changed
        }
        MediaEdit::RemoveFolder { folder } => {
            let node = super::folders::parent(engine, Some(folder))?;
            engine.edits.push(Edit::RemoveNode { node });
            MediaEditResult::Changed
        }
        MediaEdit::MoveEntry { entry, parent } => {
            let node = engine.node_id_by_uuid(entry).ok_or("media entry is missing")?;
            let entry_node = engine.nodes.get(node).ok_or("media entry is missing")?;
            if !matches!(
                entry_node,
                AppNode::Media(_) | AppNode::ReferenceMedia(_) | AppNode::MediaFolder(_)
            ) {
                return Err("only media and folders can be organized".into());
            }
            let manager = super::folders::parent(engine, None)?;
            if !super::folders::contains_parent(engine, entry_node.node_data().parent, manager) {
                return Err("media entry is outside Medias".into());
            }
            let new_parent = super::folders::parent(engine, parent)?;
            super::structure::validate_move(engine, node, new_parent)?;
            if entry_node.node_data().parent != Some(new_parent) {
                engine.edits.push(Edit::MoveNode {
                    node,
                    new_parent,
                    new_prev_sibling: engine
                        .nodes
                        .get(new_parent)
                        .and_then(|node| node.node_data().last_child),
                });
            }
            MediaEditResult::Changed
        }
        MediaEdit::AddChannel {
            media: owner,
            label: requested,
        } => {
            let (parent, kind) = media(engine, owner)?;
            if !kind.has_authored_channel_variants() {
                return Err("this media derives its output channels; only source media can add variants".into());
            }
            let channel = ChannelNode::from_definition(
                label(engine, parent, requested, "Channel")?,
                ChannelDefinition::for_media(kind),
            );
            let reference = ChannelReference {
                media: owner,
                channel: ChannelId(channel.node_data().meta.uuid),
            };
            engine.edits.push(Edit::AddUserItemTree {
                tree: NodeTree::new(channel),
                parent,
                prev_sibling: engine.nodes.get(parent).and_then(|n| n.node_data().last_child),
            });
            MediaEditResult::Created { reference }
        }
        MediaEdit::ConfigureChannel { reference, source } => {
            configure_channel(engine, reference, source)?;
            MediaEditResult::Changed
        }
        MediaEdit::ConfigurePictureFile { reference, file } => {
            super::picture_file::configure(engine, reference, &file)?;
            MediaEditResult::Changed
        }
        MediaEdit::ReloadPicture { reference } => {
            super::picture_file::reload(engine, reference)?;
            MediaEditResult::Changed
        }
        MediaEdit::InspectPictureFile { reference } => MediaEditResult::PictureFile {
            binding: super::picture_file::inspect(engine, reference)?,
        },
        MediaEdit::ConfigureMediaSenderSelection { media: owner, mode } => {
            let (id, _) = media(engine, owner)?;
            let parameter = match engine.nodes.get(id) {
                Some(AppNode::Media(media)) => media.sender_selection_parameter(),
                _ => None,
            }
            .ok_or("sender selection requires Shared Texture media")?;
            engine.edits.push(Edit::SetParam {
                node: parameter,
                value: golden_core::parameter::ParamValue::Enum(mode.name().into()),
                behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
            });
            MediaEditResult::Changed
        }
        MediaEdit::ConfigureMediaFitMode { media: owner, mode } => {
            let (id, _) = media(engine, owner)?;
            let parameter = match engine.nodes.get(id) {
                Some(AppNode::Media(media)) => media.fit_mode_parameter(),
                Some(AppNode::ReferenceMedia(media)) => Some(media.fit_mode_parameter()),
                _ => unreachable!(),
            }
            .ok_or("Fit Mode requires a Picture, Shared Texture or Reference media")?;
            engine.edits.push(Edit::SetParam {
                node: parameter,
                value: golden_core::parameter::ParamValue::Enum(mode.name().into()),
                behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
            });
            MediaEditResult::Changed
        }
        MediaEdit::ConfigureMediaResolution { media: owner, size } => {
            configure_resolution(engine, owner, size)?;
            MediaEditResult::Changed
        }
        MediaEdit::RenameMedia { media: owner, label } => {
            let (id, _) = media(engine, owner)?;
            rename(engine, id, label)?;
            MediaEditResult::Changed
        }
        MediaEdit::RenameChannel { reference, label } => {
            let id = channel_id(engine, reference)?;
            rename(engine, id, label)?;
            MediaEditResult::Changed
        }
        MediaEdit::ReorderChannel { reference, after } => {
            let node = channel_id(engine, reference)?;
            let (parent, _) = media(engine, reference.media)?;
            let previous = after
                .map(|channel| channel_id(engine, ChannelReference { channel, ..reference }))
                .transpose()?;
            if previous == Some(node) {
                return Err("channel cannot follow itself".into());
            }
            engine.edits.push(Edit::MoveNode {
                node,
                new_parent: parent,
                new_prev_sibling: previous,
            });
            MediaEditResult::Changed
        }
        MediaEdit::MoveChannel { reference, direction } => {
            move_channel(engine, reference, direction)?;
            MediaEditResult::Changed
        }
        MediaEdit::RemoveChannel { reference } => {
            if media(engine, reference.media)?.1 == MediaKind::Matrix {
                return Err("remove the Matrix item to remove its export".into());
            }
            let node = channel_id(engine, reference)?;
            let (parent, _) = media(engine, reference.media)?;
            if crate::parameters::domain_children(engine, parent)?.len() == 1 {
                return Err("media must retain at least one channel".into());
            }
            engine.edits.push(Edit::RemoveNode { node });
            MediaEditResult::Changed
        }
        MediaEdit::RemoveMedia { media: owner } => {
            let (node, _) = media(engine, owner)?;
            engine.edits.push(Edit::RemoveNode { node });
            MediaEditResult::Changed
        }
        MediaEdit::InspectChannel { reference } => MediaEditResult::Channel {
            snapshot: super::queries::inspect_channel(engine, reference)?,
        },
    };
    if !live_geometry && engine.edits.pending.len() != pending_before {
        // Published inside the atomic edit boundary, including script/headless callers.
        // Geometry already publishes typed Parameter deltas and its authoritative reply.
        // Catalog discovery must not run for every pointer sample.
        let event = golden_core::events::CustomEvent::from_latest_payload(MEDIA_NAMESPACE, None, &())
            .map_err(|error| error.to_string())?;
        engine.edits.push(Edit::EmitCustomEvent { event });
    }
    serde_json::to_value(result).map_err(|error| error.to_string())
}

pub(super) fn locator_label(locator: &AssetLocator) -> &str {
    match locator {
        AssetLocator::ProjectRelative { path } => path.as_str(),
        AssetLocator::Absolute { path } => path.as_str(),
    }
    .rsplit(['/', '\\'])
    .find(|segment| !segment.is_empty())
    .unwrap_or("Asset")
}

pub(crate) fn media_tree(label: String, definition: ChannelDefinition) -> (NodeTree, ChannelReference) {
    let matrix_canvas = match &definition.source {
        ChannelSource::Matrix {
            output: crate::matrix::protocol::MatrixOutput::Final { definition },
        } => Some(definition.canvas),
        _ => None,
    };
    let paired_export = !definition.media_kind().has_authored_channel_variants();
    let media = MediaNode::from_definition(label, &definition);
    let channel_label = if definition.media_kind() == MediaKind::Matrix {
        "Final"
    } else {
        "Channel 1"
    };
    let channel = ChannelNode::from_definition(channel_label.into(), definition);
    let reference = ChannelReference {
        media: MediaId(media.node_data().meta.uuid),
        channel: ChannelId(channel.node_data().meta.uuid),
    };
    let channel = NodeTree::new(channel);
    let channel = if paired_export { channel } else { channel.as_user_item() };
    let (channel, exports) = match matrix_canvas {
        Some(canvas) => crate::matrix::defaults::initial_tree(reference.media, channel, canvas),
        None => (channel, Vec::new()),
    };
    let mut tree = NodeTree::new(media).with_child(channel);
    tree.children.extend(exports);
    (tree, reference)
}

pub(super) fn configure_channel(
    engine: &mut AppEngine,
    reference: ChannelReference,
    source: ChannelSource,
) -> Result<(), String> {
    let id = channel_id(engine, reference)?;
    let (owner, kind) = media(engine, reference.media)?;
    let definition = ChannelDefinition {
        source,
        ..ChannelDefinition::default()
    };
    definition.validate()?;
    if definition.media_kind() != kind {
        return Err("channel source does not match its media kind".into());
    }
    if !kind.has_authored_channel_variants() {
        return Err("derived output sources are read projections; edit media Resolution or Matrix settings".into());
    }
    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
        unreachable!()
    };
    let mode = match engine.nodes.get(owner) {
        Some(AppNode::Media(media)) => media.sender_selection().unwrap_or_default(),
        _ => SenderSelectionMode::default(),
    };
    for edit in channel.parameter_edits(definition.source, mode)? {
        engine.edits.push(edit);
    }
    Ok(())
}

fn move_channel(
    engine: &mut AppEngine,
    reference: ChannelReference,
    direction: ChannelMoveDirection,
) -> Result<(), String> {
    let (parent, kind) = media(engine, reference.media)?;
    if !kind.has_authored_channel_variants() {
        return Err("derived output channels cannot be reordered".into());
    }
    let id = channel_id(engine, reference)?;
    let node = engine.nodes.get(id).ok_or("channel is missing")?;
    let mut neighbor = match direction {
        ChannelMoveDirection::Earlier => node.node_data().prev_sibling,
        ChannelMoveDirection::Later => node.node_data().next_sibling,
    };
    while let Some(id) = neighbor {
        let node = engine.nodes.get(id).ok_or("media child is missing")?;
        if matches!(node, AppNode::MediaChannel(_)) {
            break;
        }
        neighbor = match direction {
            ChannelMoveDirection::Earlier => node.node_data().prev_sibling,
            ChannelMoveDirection::Later => node.node_data().next_sibling,
        };
    }
    let Some(neighbor) = neighbor else { return Ok(()) };
    let previous = match direction {
        ChannelMoveDirection::Earlier => engine.nodes.get(neighbor).unwrap().node_data().prev_sibling,
        ChannelMoveDirection::Later => Some(neighbor),
    };
    engine.edits.push(Edit::MoveNode {
        node: id,
        new_parent: parent,
        new_prev_sibling: previous,
    });
    Ok(())
}

pub(crate) fn effective_definition(engine: &AppEngine, channel: &dyn Node) -> Result<ChannelDefinition, String> {
    match channel.node_data().parent.and_then(|id| engine.nodes.get(id)) {
        Some(AppNode::Media(media)) => channel
            .as_any()
            .downcast_ref::<ChannelNode>()
            .map(|channel| channel.definition(media, engine))
            .ok_or("invalid source channel".into()),
        Some(AppNode::ReferenceMedia(media)) => Ok(media.definition(engine, ChannelId(channel.node_data().meta.uuid))),
        _ => Err("channel owner is not media".into()),
    }
}

fn configure_resolution(engine: &mut AppEngine, owner: MediaId, size: SizePolicy) -> Result<(), String> {
    let (id, _) = media(engine, owner)?;
    let parameter = match engine.nodes.get(id) {
        Some(AppNode::Media(media)) => media.resolution_parameter(),
        Some(AppNode::ReferenceMedia(media)) => media.resolution_parameter(),
        _ => unreachable!(),
    };
    if let SizePolicy::Fixed { extent } = size {
        if extent.width() > 16384 || extent.height() > 16384 {
            return Err("Resolution must contain whole pixels in 1..16384 per axis".into());
        }
        engine.edits.push(Edit::SetParam {
            node: parameter,
            value: golden_core::parameter::ParamValue::Vec2(extent.width().into(), extent.height().into()),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
        });
    }
    engine.edits.push(Edit::PatchMeta {
        node: parameter,
        patch: golden_core::node::NodeMetaPatch {
            enabled: Some(matches!(size, SizePolicy::Fixed { .. })),
            ..Default::default()
        },
    });
    Ok(())
}

pub fn validate_project(engine: &AppEngine) -> Result<(), String> {
    // Whole-project validation belongs to detached load/preparation, never the frame loop.
    let manager = manager(engine)?;
    let managed = super::folders::entries(engine)?
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    for (id, node) in engine.nodes.iter() {
        if matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_))
            && node
                .node_data()
                .parent
                .and_then(|id| engine.nodes.get(id))
                .is_some_and(|node| matches!(node, AppNode::Media(media) if media.kind() == MediaKind::Composition))
        {
            super::composition::children::placement(engine, id)?;
        }
        match node {
            AppNode::ReferenceMedia(reference) => {
                if reference.target().is_none() && reference.selected_channel().is_some() {
                    return Err("Reference Channel requires a Media target".into());
                }
                if let Some(source) = reference.source(engine) {
                    super::targeting::validate_existing_output(engine, source)?;
                }
                if !managed.contains(&id) {
                    return Err("Reference is outside its media hierarchy".into());
                }
                let mut outputs = 0;
                for child in crate::parameters::domain_children(engine, id)? {
                    match engine.nodes.get(child) {
                        Some(AppNode::ReferenceOutput(_)) => outputs += 1,
                        Some(AppNode::CompositionPlacement(_)) => {}
                        _ => return Err("Reference owns only Parameters, one Output and optional Placement".into()),
                    }
                }
                if outputs != 1 {
                    return Err("Reference requires exactly one output".into());
                }
            }
            AppNode::MediaCommand(node) => {
                if node.node_data().parent != Some(manager) || crate::parameters::has_domain_children(engine, id) {
                    return Err("commands belong to Medias and contain only declared Parameters".into());
                }
            }
            AppNode::MediaFolder(_) if !managed.contains(&id) => {
                return Err("media folder is outside its manager".into());
            }
            AppNode::MediaAsset(node) => {
                if node.node_data().parent != Some(manager) {
                    return Err("asset is outside its catalog".into());
                }
            }
            AppNode::Media(node) => {
                if !managed.contains(&id) {
                    return Err("media is outside its manager".into());
                }
                let accepts_media_children = node.kind() == MediaKind::Composition;
                let mut child = node.node_data().first_child;
                let mut count = 0;
                while let Some(id) = child {
                    let node = engine.nodes.get(id).ok_or("missing media child")?;
                    child = node.node_data().next_sibling;
                    if crate::parameters::is_parameter_folder(node) {
                        continue;
                    }
                    if matches!(node, AppNode::CompositionPlacement(_)) {
                        continue;
                    }
                    if matches!(node, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
                        if !accepts_media_children {
                            return Err("only Composition owns child media".into());
                        }
                        continue;
                    }
                    let Some(AppNode::MediaChannel(channel)) = engine.nodes.get(id) else {
                        return Err("invalid media child".into());
                    };
                    count += 1;
                    child = channel.node_data().next_sibling;
                }
                if count == 0 {
                    return Err("media must retain at least one channel".into());
                }
                if node.kind() == MediaKind::Composition && count != 1 {
                    return Err("Composition must expose exactly one output channel".into());
                }
            }
            AppNode::ReferenceOutput(node) => {
                if !node
                    .node_data()
                    .parent
                    .and_then(|id| engine.nodes.get(id))
                    .is_some_and(|parent| matches!(parent, AppNode::ReferenceMedia(_)))
                    || node.node_data().meta.decl_id.0 != "output"
                    || crate::parameters::has_domain_children(engine, id)
                {
                    return Err("Reference output must occupy its owner's declared output slot".into());
                }
            }
            AppNode::MediaChannel(node) => {
                effective_definition(engine, node.as_ref())?.validate()?;
                let parent = node
                    .node_data()
                    .parent
                    .and_then(|id| engine.nodes.get(id))
                    .ok_or("channel owner is missing")?;
                let kind = match parent {
                    AppNode::Media(parent) => parent.kind(),
                    AppNode::ReferenceMedia(_) => MediaKind::Reference,
                    _ => return Err("channel owner is not media".into()),
                };
                if kind != node.kind() {
                    return Err("channel source does not match its media kind".into());
                }
                if !node.is_matrix_final() && crate::parameters::has_domain_children(engine, id) {
                    return Err("only Matrix final channels may own domain children".into());
                }
            }
            AppNode::CompositionPlacement(node) => {
                let parent = node.node_data().parent.ok_or("Placement has no media owner")?;
                let owner = engine.nodes.get(parent).ok_or("Placement media is missing")?;
                if !matches!(owner, AppNode::Media(_) | AppNode::ReferenceMedia(_)) {
                    return Err("Placement belongs to media".into());
                }
                if !owner
                    .node_data()
                    .parent
                    .and_then(|id| engine.nodes.get(id))
                    .is_some_and(|node| matches!(node, AppNode::Media(media) if media.kind() == MediaKind::Composition))
                {
                    return Err("Placement belongs to a Composition child".into());
                }
                super::composition::children::placement(engine, parent)?;
                node.definition(MediaId(owner.node_data().meta.uuid)).validate()?;
                if let Some(source) = super::composition::children::definition(engine, parent)?.source {
                    super::targeting::validate_existing_output(engine, source)?;
                }
            }
            _ => {}
        }
    }
    crate::matrix::validate_project(engine)?;
    crate::applications::validate_project(engine)?;
    super::dependencies::validate_project(engine)
}

fn create_media(
    engine: &mut AppEngine,
    parent: NodeId,
    media_kind: MediaKind,
    requested: Option<String>,
) -> Result<MediaEditResult, String> {
    if media_kind == MediaKind::Reference {
        return super::reference::create(engine, parent, None, requested);
    }
    let (tree, reference) = super::creation::media(
        engine,
        media_kind,
        label(engine, parent, requested, media_kind.default_label())?,
    );
    engine.edits.push(Edit::AddUserItemTree {
        tree,
        parent,
        prev_sibling: engine.nodes.get(parent).and_then(|n| n.node_data().last_child),
    });
    Ok(MediaEditResult::Created { reference })
}
