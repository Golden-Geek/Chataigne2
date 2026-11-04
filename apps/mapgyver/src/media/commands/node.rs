use super::MediaCommandKind;
use crate::media::protocol::*;
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, NodeScriptDescriptor, ParameterHandle, ParameterValidationContext},
    parameter::{Color, Enum, File, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_video::Extent2D;
use std::sync::Arc;

#[node("mapgyver.media.command.v5", label = "Media Command", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
})]
#[children(
    folder(parameters, label = "Parameters") {
        command_kind: Enum = "CreateMedia" (label = "Command", read_only = true, persist_read_only_value = true,
            enum_options = super::kind_options());
        trigger: ParamValue = ParamValue::Trigger() (label = "Execute");
        media: NodeReference = NodeReference::empty() (
            label = "Media",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.v7".into(), "mapgyver.media.reference.v3".into()]
        );
        matrix_layer: NodeReference = NodeReference::empty() (label = "Layer", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.layer.v3".into()]);
        matrix_column: NodeReference = NodeReference::empty() (label = "Column", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.column.v2".into()]);
        application_owner: NodeReference = NodeReference::empty() (label = "App control owner", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.column.v2".into(), "mapgyver.matrix.cell.v3".into()]);
        application_file: File = File::default() (label = "App file", file_allowed_extensions = vec!["exe".into()]);
        launch_mode: Enum = "Configured" (label = "Launch mode", enum_options = crate::parameters::enum_options(&["Configured", "Cut"]), constraint_policy = "reject");
        position: Vec2 = Vec2::new(0.0, 0.0) (label = "Position", description = "Composition canvas texels.");
        copy_media: bool = false (label = "Copy media");
        composition_target: NodeReference = NodeReference::empty() (label = "Replace media", reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.media.v7".into(), "mapgyver.media.reference.v3".into()]);
        reference_target: NodeReference = NodeReference::empty() (label = "Target media", reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.v7".into(), "mapgyver.media.reference.v3".into()]);
        channel: NodeReference = NodeReference::empty() (
            label = "Channel",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.channel".into()]
        );
        asset: NodeReference = NodeReference::empty() (
            label = "Asset",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.asset".into()]
        );
        target_folder: NodeReference = NodeReference::empty() (
            label = "Folder",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.folder".into()]
        );
        entry: NodeReference = NodeReference::empty() (
            label = "Entry",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.folder".into(), "mapgyver.media.v7".into(), "mapgyver.media.reference.v3".into()]
        );
        after_channel: NodeReference = NodeReference::empty() (
            label = "After channel",
            reference_target_kind = ReferenceTargetKind::AnyNode,
            reference_allowed_node_types = vec!["mapgyver.media.channel".into()]
        );
        label: String = "" (label = "Label", description = "Empty uses the backend default when creating objects.");
        media_kind: Enum = "SolidColor" (
            label = "Media kind",
            enum_options = crate::parameters::enum_options(&["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix", "Reference"]),
            constraint_policy = "reject"
        );
        resolution_enabled: bool = false (label = "Force resolution");
        resolution: Vec2 = Vec2::new(1920.0, 1080.0) [(1.0, 1.0)..(16384.0, 16384.0)] (
            label = "Resolution",
            step = 1.0,
            constraint_policy = "reject"
        );
        fit_mode: Enum = "Fit" (
            label = "Fit Mode",
            enum_options = crate::parameters::enum_options(&["Fit", "Fill", "Stretch"]),
            constraint_policy = "reject"
        );
        rgba: Color = Color::new(1.0, 1.0, 1.0, 1.0) [0.0..1.0] (label = "Color", constraint_policy = "reject");
        sender: String = "" (label = "Sender");
        locator_kind: Enum = "ProjectRelative" (
            label = "Location",
            enum_options = crate::parameters::enum_options(&["ProjectRelative", "Absolute"]),
            constraint_policy = "reject"
        );
        project_path: File = File::default() (label = "Project path");
        absolute_path: File = File::default() (label = "Absolute path");
        direction: Enum = "Earlier" (
            label = "Direction",
            enum_options = crate::parameters::enum_options(&["Earlier", "Later"]),
            constraint_policy = "reject"
        );
        choice_scope: Enum = "All" (
            label = "Scope",
            enum_options = crate::parameters::enum_options(&["All", "MatrixFinal"]),
            constraint_policy = "reject"
        );
        search: String = "" (label = "Search");
        start: i32 = 0 [0..2147483647] (label = "Start", constraint_policy = "reject");
        count: i32 = 32 [1..128] (label = "Count", constraint_policy = "reject");
    }
)]
pub struct MediaCommandNode {
    #[state(default = Arc::new(()))]
    owner: Arc<()>,
    #[state(default = false)]
    ready: bool,
    #[state(default = 0)]
    sequence: u64,
}

fn choice<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(value.into())).map_err(|error| error.to_string())
}
fn optional(reference: &NodeReference) -> Option<golden_core::node::NodeUuid> {
    (!reference.uuid.0.is_nil()).then_some(reference.uuid)
}
impl MediaCommandNode {
    pub(crate) fn from_kind(label: String, kind: MediaCommandKind) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node.command_kind = ParameterHandle::new(Enum::new(kind.key()));
        node
    }
    fn kind(&self) -> Result<MediaCommandKind, String> {
        choice(self.command_kind.get_ref().as_str())
    }
    pub(crate) fn operation(&self) -> Result<MediaEdit, String> {
        if !self.node_data().effective_enabled {
            return Err("media command is disabled".into());
        }
        let media = MediaId(self.media.get_ref().uuid);
        let reference = ChannelReference {
            media,
            channel: ChannelId(self.channel.get_ref().uuid),
        };
        let label = self.label.get_ref().clone();
        let requested = (!label.is_empty()).then_some(label.clone());
        let asset = AssetId::from_uuid(self.asset.get_ref().uuid.0);
        let folder = optional(self.target_folder.get_ref());
        Ok(match self.kind()? {
            MediaCommandKind::SetApplicationFile => MediaEdit::Applications {
                edit: crate::applications::protocol::ApplicationEdit::SetFile {
                    application: crate::applications::protocol::ApplicationId(self.application_owner.get_ref().uuid),
                    executable: match self.application_file.get_ref().as_str() {
                        "" => None,
                        path => Some(path.into()),
                    },
                },
            },
            MediaCommandKind::LaunchMatrixColumn => MediaEdit::Matrix {
                media,
                edit: crate::matrix::protocol::MatrixEdit::LaunchColumn {
                    column: crate::matrix::protocol::ColumnId(self.matrix_column.get_ref().uuid),
                    mode: choice(self.launch_mode.get_ref().as_str())?,
                },
            },
            MediaCommandKind::LaunchMatrixCell => MediaEdit::Matrix {
                media,
                edit: crate::matrix::protocol::MatrixEdit::LaunchCell {
                    layer: crate::matrix::protocol::LayerId(self.matrix_layer.get_ref().uuid),
                    column: crate::matrix::protocol::ColumnId(self.matrix_column.get_ref().uuid),
                    mode: choice(self.launch_mode.get_ref().as_str())?,
                },
            },
            MediaCommandKind::UnloadMatrix => MediaEdit::Matrix {
                media,
                edit: crate::matrix::protocol::MatrixEdit::ClearPlayback,
            },
            MediaCommandKind::InspectApplication => MediaEdit::Applications {
                edit: crate::applications::protocol::ApplicationEdit::Inspect {
                    application: crate::applications::protocol::ApplicationId(self.application_owner.get_ref().uuid),
                },
            },
            MediaCommandKind::InspectApplicationRuntime => MediaEdit::Applications {
                edit: crate::applications::protocol::ApplicationEdit::InspectRuntime {},
            },
            MediaCommandKind::InspectApplicationCallbacks => MediaEdit::Applications {
                edit: crate::applications::protocol::ApplicationEdit::InspectCallbacks {},
            },
            MediaCommandKind::DropMatrixMedia => MediaEdit::Matrix {
                media,
                edit: crate::matrix::protocol::MatrixEdit::DropSource {
                    layer: crate::matrix::protocol::LayerId(self.matrix_layer.get_ref().uuid),
                    column: crate::matrix::protocol::ColumnId(self.matrix_column.get_ref().uuid),
                    source: self.entry.get_ref().uuid,
                    copy: self.copy_media.get(),
                },
            },
            MediaCommandKind::DropCompositionMedia => MediaEdit::Composition {
                reference,
                edit: super::super::composition::protocol::CompositionEdit::DropSource {
                    source: self.entry.get_ref().uuid,
                    instance: optional(self.composition_target.get_ref())
                        .map(super::super::composition::protocol::InstanceId),
                    copy: self.copy_media.get(),
                    position: golden_video::plan::Point {
                        x: self.position.get().x as f32,
                        y: self.position.get().y as f32,
                    },
                },
            },
            MediaCommandKind::CreateReference => MediaEdit::CreateReference {
                target: optional(self.reference_target.get_ref()).map(MediaId),
                label: requested,
            },
            MediaCommandKind::ConfigureReference => MediaEdit::ConfigureReference {
                media,
                target: optional(self.reference_target.get_ref()).map(MediaId),
            },
            MediaCommandKind::CreateMedia => MediaEdit::CreateMedia {
                media_kind: choice(self.media_kind.get_ref().as_str())?,
                label: requested,
            },
            MediaCommandKind::CreateMediaInFolder => MediaEdit::CreateMediaInFolder {
                folder: self.target_folder.get_ref().uuid,
                media_kind: choice(self.media_kind.get_ref().as_str())?,
                label: requested,
            },
            MediaCommandKind::RenameMedia => MediaEdit::RenameMedia { media, label },
            MediaCommandKind::RemoveMedia => MediaEdit::RemoveMedia { media },
            MediaCommandKind::AddChannel => MediaEdit::AddChannel {
                media,
                label: requested,
            },
            MediaCommandKind::RenameChannel => MediaEdit::RenameChannel { reference, label },
            MediaCommandKind::ReorderChannel => MediaEdit::ReorderChannel {
                reference,
                after: optional(self.after_channel.get_ref()).map(ChannelId),
            },
            MediaCommandKind::MoveChannel => MediaEdit::MoveChannel {
                reference,
                direction: choice(self.direction.get_ref().as_str())?,
            },
            MediaCommandKind::RemoveChannel => MediaEdit::RemoveChannel { reference },
            MediaCommandKind::ConfigureResolution => {
                let value = self.resolution.get();
                super::super::media_node::validate_resolution(value)?;
                MediaEdit::ConfigureMediaResolution {
                    media,
                    size: if self.resolution_enabled.get() {
                        SizePolicy::Fixed {
                            extent: Extent2D::new(value.x as u32, value.y as u32).map_err(|error| error.to_string())?,
                        }
                    } else {
                        SizePolicy::Automatic
                    },
                }
            }
            MediaCommandKind::ConfigureFitMode => MediaEdit::ConfigureMediaFitMode {
                media,
                mode: choice::<MediaFitMode>(self.fit_mode.get_ref().as_str())?,
            },
            MediaCommandKind::SolidColor => {
                let color = self.rgba.get();
                MediaEdit::ConfigureChannel {
                    reference,
                    source: ChannelSource::SolidColor {
                        rgba: [
                            color.red as f32,
                            color.green as f32,
                            color.blue as f32,
                            color.alpha as f32,
                        ],
                    },
                }
            }
            MediaCommandKind::Picture => MediaEdit::ConfigureChannel {
                reference,
                source: ChannelSource::Picture {
                    asset: optional(self.asset.get_ref()).map(|uuid| AssetId::from_uuid(uuid.0)),
                },
            },
            MediaCommandKind::SharedTexture => MediaEdit::ConfigureChannel {
                reference,
                source: ChannelSource::Spout {
                    sender: (!self.sender.get_ref().is_empty()).then(|| self.sender.get_ref().clone()),
                },
            },
            MediaCommandKind::ImportPicture => MediaEdit::ImportPicture {
                locator: self.locator()?,
                target: if optional(self.media.get_ref()).is_some() || optional(self.channel.get_ref()).is_some() {
                    Some(reference)
                } else {
                    None
                },
                label: requested,
            },
            MediaCommandKind::ImportAsset => MediaEdit::ImportAsset {
                locator: self.locator()?,
                label: requested,
            },
            MediaCommandKind::RelinkAsset => MediaEdit::RelinkAsset {
                asset,
                locator: self.locator()?,
            },
            MediaCommandKind::ReloadAsset => MediaEdit::ReloadAsset { asset },
            MediaCommandKind::RemoveAsset => MediaEdit::RemoveAsset { asset },
            MediaCommandKind::InspectAsset => MediaEdit::InspectAsset { asset },
            MediaCommandKind::CreateFolder => MediaEdit::CreateFolder {
                parent: folder,
                label: requested,
            },
            MediaCommandKind::RenameFolder => MediaEdit::RenameFolder {
                folder: self.target_folder.get_ref().uuid,
                label,
            },
            MediaCommandKind::RemoveFolder => MediaEdit::RemoveFolder {
                folder: self.target_folder.get_ref().uuid,
            },
            MediaCommandKind::MoveEntry => MediaEdit::MoveEntry {
                entry: self.entry.get_ref().uuid,
                parent: folder,
            },
            MediaCommandKind::InspectManager => MediaEdit::InspectManager,
            MediaCommandKind::InspectChannel => MediaEdit::InspectChannel { reference },
            MediaCommandKind::InspectChannels => MediaEdit::InspectChannels {
                query: MediaChannelsQuery {
                    media,
                    start: self.start.get() as u32,
                    count: self.count.get() as u16,
                },
            },
            MediaCommandKind::InspectChoices => MediaEdit::InspectChoices {
                query: ChannelChoiceQuery {
                    scope: choice(self.choice_scope.get_ref().as_str())?,
                    search: self.search.get_ref().clone(),
                    start: self.start.get() as u32,
                    count: self.count.get() as u16,
                },
            },
            MediaCommandKind::CreateScript => MediaEdit::CreateScript { label: requested },
        })
    }
    fn locator(&self) -> Result<AssetLocator, String> {
        match self.locator_kind.get_ref().as_str() {
            "Absolute" => Ok(AssetLocator::Absolute {
                path: self
                    .absolute_path
                    .get_ref()
                    .as_str()
                    .to_owned()
                    .try_into()
                    .map_err(|error: golden_assets::LocatorError| error.to_string())?,
            }),
            "ProjectRelative" => Ok(AssetLocator::ProjectRelative {
                path: self
                    .project_path
                    .get_ref()
                    .as_str()
                    .to_owned()
                    .try_into()
                    .map_err(|error: golden_assets::LocatorError| error.to_string())?,
            }),
            _ => Err("unknown command location kind".into()),
        }
    }
    fn enqueue(&self, ctx: &mut ProcessCtx, request_id: &str) -> Result<(), String> {
        super::super::scripting::enqueue(ctx, self.id(), &self.owner, request_id, self.operation()?)
    }
}

#[node("mapgyver.media.command.v5", from_struct)]
impl Node for MediaCommandNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn on_node_ready(&mut self, _: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        self.ready = true;
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if !self.ready || parameter != self.trigger.id() || !self.node_data().effective_enabled {
            return;
        }
        let Some(next) = self.sequence.checked_add(1) else {
            return;
        };
        self.sequence = next;
        let request_id = format!("command-{}-{next}", self.node_data().meta.uuid.0);
        if let Err(error) = self.enqueue(ctx, &request_id) {
            ctx.set_node_warning_with(self.id(), Some("command"), error, None);
        }
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.owner = Arc::new(());
        }
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.owner = Arc::new(());
        self.ready = false;
    }
    fn user_container_rules(&self) -> Option<golden_core::node::UserContainerRules> {
        Some(golden_core::node::UserContainerRules::new(&[]))
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        let mut descriptor = NodeScriptDescriptor::default();
        descriptor.add_method("execute");
        descriptor
    }
    fn engine_call_custom_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if method != "execute" {
            return Ok(false);
        }
        let [ParamValue::Str(request_id)] = args else {
            return Err("execute expects one request id string".into());
        };
        self.enqueue(ctx, request_id)?;
        Ok(true)
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if event.topic == golden_core::script::APP_OPERATION_RESULT_TOPIC && event.origin == Some(self.id()) {
            let reply = &event.payload["args"][0];
            if reply["status"] == "rejected" {
                ctx.set_node_warning_with(
                    self.id(),
                    Some("command"),
                    reply["message"].as_str().unwrap_or("command rejected"),
                    None,
                );
            } else {
                ctx.clear_node_warning(self.id(), Some("command"));
            }
        }
        super::super::scripting::forward_result(ctx, self.id(), event);
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.kind()?;
        choice::<MediaKind>(self.media_kind.get_ref().as_str())?;
        choice::<MediaFitMode>(self.fit_mode.get_ref().as_str())?;
        choice::<ChannelMoveDirection>(self.direction.get_ref().as_str())?;
        choice::<ChannelChoiceScope>(self.choice_scope.get_ref().as_str())?;
        choice::<crate::matrix::protocol::LaunchMode>(self.launch_mode.get_ref().as_str())?;
        if !["ProjectRelative", "Absolute"].contains(&self.locator_kind.get_ref().as_str())
            || self.start.get() < 0
            || !(1..=128).contains(&self.count.get())
        {
            return Err("invalid command query or location settings".into());
        }
        super::super::media_node::validate_resolution(self.resolution.get())
    }
    fn validate_parameter_change(&self, ctx: &ParameterValidationContext<'_>) -> Result<(), String> {
        if ctx.parameter == self.resolution.id()
            && let ParamValue::Vec2(x, y) = ctx.value
        {
            super::super::media_node::validate_resolution(Vec2::new(*x, *y))?;
        }
        Ok(())
    }
}
