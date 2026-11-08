//! Variant settings are real Parameters; media-owned settings join them only in runtime projections.
use super::protocol::{ChannelDefinition, ChannelSource, SizePolicy};
use crate::matrix::protocol::{GroupId, LayerId, MatrixDefinition, MatrixOutput};
use golden_core::{
    edit::Edit,
    node,
    node::{
        Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext, UserContainerRules, UserCreatableItem,
    },
    parameter::{Color, Enum, File, ParamValue, ParameterEventBehaviour, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

use std::sync::OnceLock;

#[node("mapgyver.media.channel.v2", label = "Channel", collapsed = false)]
#[children(
    folder(parameters, label = "Parameters") {
        source_kind: Enum = "SolidColor" (label = "Source kind", read_only = true, persist_read_only_value = true,
            enum_options = crate::parameters::enum_options(&["Picture", "Video", "SolidColor", "SharedTexture", "Composition", "Matrix"]));
        rgba: Color = Color::new(1.0, 1.0, 1.0, 1.0) [0.0..1.0] (label = "Color", description = "Straight linear-light RGBA.", constraint_policy = "reject");
        file: File = File::default() (label = "File", description = "This channel's project-relative or native absolute source path.", dependency = |node: &Self| matches!(node.kind(), super::protocol::MediaKind::Picture | super::protocol::MediaKind::Video));
        sender: String = String::new() (label = "Spout sender");
        sender_choice: Enum = "" (label = "Sender", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::SharedTexture,
            enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
            enum_options = self.sender_options.clone());
        output_kind: Enum = "Final" (label = "Matrix output", read_only = true, persist_read_only_value = true, enum_options = crate::parameters::enum_options(&["Final", "Layer", "Group"]));
        output_item: NodeReference = NodeReference::empty() (label = "Output item", read_only = true, persist_read_only_value = true, reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.layer.v3".into(), "mapgyver.matrix.group.v2".into()]);
    }
)]
pub struct ChannelNode {
    #[state(default = super::sender_catalog::choices(&[]))]
    sender_options: golden_core::parameter::ParameterEnumOptions,
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
    #[state(default = 0)]
    picture_reload: u64,
    #[state(default = OnceLock::new())]
    matrix_playback: OnceLock<crate::matrix::playback::PlaybackControl>,
}
impl ChannelNode {
    pub(super) fn initialize_sender_options(&mut self, options: golden_core::parameter::ParameterEnumOptions) {
        assert!(
            !self.sender_choice.is_bound(),
            "sender defaults require a detached channel"
        );
        self.sender_options = options;
    }
    pub(crate) fn sender_parameter(&self, mode: super::protocol::SenderSelectionMode) -> NodeId {
        match mode {
            super::protocol::SenderSelectionMode::Manual => self.sender.id(),
            super::protocol::SenderSelectionMode::Auto => self.sender_choice.id(),
        }
    }
    pub(crate) fn sender_name(&self, mode: super::protocol::SenderSelectionMode) -> String {
        match mode {
            super::protocol::SenderSelectionMode::Manual => self.sender.get_ref().clone(),
            super::protocol::SenderSelectionMode::Auto => self.sender_choice.get_ref().as_str().to_owned(),
        }
    }
    pub(crate) fn from_definition(label: String, definition: ChannelDefinition) -> Self {
        let mut node = Self::new();
        let presentation = node.node_data().meta.presentation.clone();
        node.node_data_mut().meta = golden_core::node::NodeMeta::new(label).with_presentation(presentation);
        if definition.media_kind().has_authored_channel_variants() {
            let permissions = &mut node.node_data_mut().meta.user_permissions;
            permissions.can_edit_name = true;
            permissions.can_remove_and_duplicate = true;
        }
        macro_rules! seed {
            ($field:ident, $value:expr) => {
                node.$field = ParameterHandle::new($value);
            };
        }
        seed!(
            source_kind,
            Enum::new(super::media_node::kind_name(definition.media_kind()))
        );
        match definition.source {
            ChannelSource::Reference { .. } => {
                unreachable!("Reference outputs use an identity-only node")
            }
            ChannelSource::SolidColor { rgba } => {
                seed!(
                    rgba,
                    Color::new(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into())
                );
            }
            ChannelSource::Picture { file } | ChannelSource::Video { file } => {
                seed!(
                    file,
                    File::new(file.as_ref().map(super::assets::locator_path).unwrap_or_default())
                );
            }
            ChannelSource::Composition { .. } => {}
            ChannelSource::Spout { sender } => {
                let name = sender.unwrap_or_default();
                seed!(sender, name.clone());
                seed!(sender_choice, Enum::new(name));
            }
            ChannelSource::Matrix { output } => match output {
                MatrixOutput::Final { .. } => {
                    let _ = node.matrix_playback.set(Default::default());
                }
                MatrixOutput::Layer { layer } => {
                    seed!(output_kind, Enum::new("Layer"));
                    seed!(output_item, NodeReference::new(layer.0));
                }
                MatrixOutput::Group { group } => {
                    seed!(output_kind, Enum::new("Group"));
                    seed!(output_item, NodeReference::new(group.0));
                }
            },
        }
        node
    }
    pub(crate) fn kind(&self) -> super::protocol::MediaKind {
        super::media_node::parse_kind(self.source_kind.get_ref().as_str())
    }
    pub(crate) fn is_matrix_final(&self) -> bool {
        self.kind() == super::protocol::MediaKind::Matrix && self.output_kind.get_ref().as_str() == "Final"
    }
    pub(crate) fn matrix_item(&self) -> Option<crate::matrix::protocol::MatrixItemId> {
        if self.kind() != super::protocol::MediaKind::Matrix {
            return None;
        }
        match self.output_kind.get_ref().as_str() {
            "Layer" => Some(crate::matrix::protocol::MatrixItemId::Layer {
                id: LayerId(self.output_item.get_ref().uuid),
            }),
            "Group" => Some(crate::matrix::protocol::MatrixItemId::Group {
                id: GroupId(self.output_item.get_ref().uuid),
            }),
            _ => None,
        }
    }
    pub(crate) fn definition(&self, media: &super::MediaNode, engine: &crate::app::AppEngine) -> ChannelDefinition {
        let size = media.size_policy(engine);
        let canvas = match size {
            SizePolicy::Automatic => super::resolution::default_canvas(),
            SizePolicy::Fixed { extent } => extent,
        };
        let settings = media.matrix_settings();
        let source = match self.source_kind.get_ref().as_str() {
            "Video" => ChannelSource::Video {
                file: super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Video File"),
            },
            "Picture" => ChannelSource::Picture {
                file: super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Picture File"),
            },
            "Composition" => ChannelSource::Composition { canvas },
            "SharedTexture" => ChannelSource::Spout {
                sender: {
                    let name = self.sender_name(media.sender_selection().unwrap());
                    (!name.is_empty()).then_some(name)
                },
            },
            "Matrix" => ChannelSource::Matrix {
                output: match self.output_kind.get_ref().as_str() {
                    "Layer" => MatrixOutput::Layer {
                        layer: LayerId(self.output_item.get_ref().uuid),
                    },
                    "Group" => MatrixOutput::Group {
                        group: GroupId(self.output_item.get_ref().uuid),
                    },
                    _ => MatrixOutput::Final {
                        definition: MatrixDefinition {
                            canvas,
                            empty_cell: settings.empty_cell,
                        },
                    },
                },
            },
            _ => {
                let c = self.rgba.get();
                ChannelSource::SolidColor {
                    rgba: [c.red as f32, c.green as f32, c.blue as f32, c.alpha as f32],
                }
            }
        };
        ChannelDefinition {
            source,
            size,
            fit_mode: media.fit_mode(),
        }
    }
    pub(crate) fn parameter_edits(
        &self,
        source: ChannelSource,
        mode: super::protocol::SenderSelectionMode,
    ) -> Result<Vec<Edit>, String> {
        let mut values = Vec::new();
        match source {
            ChannelSource::SolidColor { rgba } => values.push((
                self.rgba.id(),
                ParamValue::Color(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into()),
            )),
            ChannelSource::Picture { file } | ChannelSource::Video { file } => values.push((
                self.file.id(),
                ParamValue::File(
                    file.as_ref()
                        .map(super::assets::locator_path)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            )),
            ChannelSource::Reference { .. } | ChannelSource::Composition { .. } | ChannelSource::Matrix { .. } => {
                return Err("derived outputs have no authored channel settings".into());
            }
            ChannelSource::Spout { sender } => {
                let sender = sender.unwrap_or_default();
                values.push((
                    self.sender_parameter(mode),
                    match mode {
                        super::protocol::SenderSelectionMode::Manual => ParamValue::Str(sender),
                        super::protocol::SenderSelectionMode::Auto => ParamValue::Enum(sender),
                    },
                ))
            }
        }
        Ok(values
            .into_iter()
            .map(|(node, value)| Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            })
            .collect())
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn file_parameter(&self) -> NodeId {
        self.file.id()
    }
    pub(crate) fn picture_reload_generation(&self) -> u64 {
        self.picture_reload
    }
    pub(super) fn reload_picture(&mut self) {
        self.picture_reload = self
            .picture_reload
            .checked_add(1)
            .expect("Picture reload generation exhausted");
        self.revision.advance();
    }
    pub(crate) fn matrix_playback(&self) -> Option<&crate::matrix::playback::PlaybackControl> {
        (self.source_kind.get_ref().as_str() == "Matrix" && self.output_kind.get_ref().as_str() == "Final")
            .then(|| self.matrix_playback.get_or_init(Default::default))
    }
    fn invalidate(&self) {
        self.revision.advance();
        if let Some(control) = self.matrix_playback.get() {
            control.invalidate();
            control.invalidate_applications();
        }
    }
}
#[node("mapgyver.media.channel.v2", from_struct)]
impl Node for ChannelNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_structure_changed(&mut self, ctx: &mut ProcessCtx) {
        use golden_core::events::EventKind;
        // Params DSL observes two child levels to bind its own folder. That must
        // not broaden logical application membership to descendant Matrix cells.
        let owner = self.id();
        if ctx.events.iter().any(|event| match event.kind {
            EventKind::ChildAdded { parent, .. }
            | EventKind::ChildRemoved { parent, .. }
            | EventKind::ChildReplaced { parent, .. }
            | EventKind::ChildReordered { parent, .. } => parent == owner,
            EventKind::ChildMoved {
                old_parent, new_parent, ..
            } => old_parent == owner || new_parent == owner,
            EventKind::NodeCreated { node } | EventKind::NodeDeleted { node } => node == owner,
            _ => false,
        }) {
            self.invalidate();
        }
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if [
            self.source_kind.id(),
            self.rgba.id(),
            self.file.id(),
            self.sender.id(),
            self.output_kind.id(),
            self.output_item.id(),
        ]
        .contains(&parameter)
            || (self.kind() == super::protocol::MediaKind::SharedTexture && parameter == self.sender_choice.id())
        {
            self.revision.advance();
            if let Some(control) = self.matrix_playback.get() {
                control.invalidate();
            }
        }
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        if let Some(control) = self.matrix_playback.get() {
            control.clear();
        }
        self.revision.advance();
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            crate::matrix::node::LAYER_TYPE,
            crate::matrix::node::GROUP_TYPE,
            crate::matrix::node::COLUMN_TYPE,
            crate::matrix::column_group_node::COLUMN_GROUP_TYPE,
        ]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        Vec::new()
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if self.is_matrix_final() {
            if node_type == crate::matrix::column_node::COLUMN_TYPE {
                return Some(Box::new(crate::matrix::ColumnNode::new()));
            }
            if node_type == crate::matrix::column_group_node::COLUMN_GROUP_TYPE {
                return Some(Box::new(crate::matrix::ColumnGroupNode::new()));
            }
            crate::matrix::node::create(node_type).map(|node| Box::new(node) as Box<dyn Node>)
        } else {
            None
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        super::assets::parse_file(self.file.get_ref().as_str())?;
        for (value, allowed) in [
            (
                self.source_kind.get_ref().as_str(),
                &[
                    "Picture",
                    "Video",
                    "SolidColor",
                    "SharedTexture",
                    "Composition",
                    "Matrix",
                ][..],
            ),
            (self.output_kind.get_ref().as_str(), &["Final", "Layer", "Group"][..]),
        ] {
            if !allowed.contains(&value) {
                return Err(format!("unknown channel setting '{value}'"));
            }
        }
        if self.kind() == super::protocol::MediaKind::SolidColor {
            let color = self.rgba.get();
            golden_video::LinearColor::from_straight(
                color.red as f32,
                color.green as f32,
                color.blue as f32,
                color.alpha as f32,
            )
            .map_err(|error| error.to_string())?;
        }
        if self.sender.get_ref().len() > 255 || self.sender.get_ref().contains('\0') {
            return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
        }
        if self.kind() == super::protocol::MediaKind::SharedTexture {
            let name = self.sender_choice.get_ref().as_str();
            if name.len() > 255 || name.contains('\0') {
                return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
            }
        }
        Ok(())
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.file.id()
            && let ParamValue::File(path) = change.value
        {
            super::assets::parse_file(path)?;
        }
        for (id, current) in [
            (
                self.source_kind.id(),
                ParamValue::Enum(self.source_kind.get_ref().0.clone()),
            ),
            (
                self.output_kind.id(),
                ParamValue::Enum(self.output_kind.get_ref().0.clone()),
            ),
            (
                self.output_item.id(),
                ParamValue::Reference(self.output_item.get_ref().clone()),
            ),
        ] {
            if change.parameter == id && change.value != &current {
                return Err("Channel source/output identity is owned by structural operations".into());
            }
        }
        if change.parameter == self.sender.id()
            || (self.kind() == super::protocol::MediaKind::SharedTexture && change.parameter == self.sender_choice.id())
        {
            let value = match change.value {
                ParamValue::Str(value) | ParamValue::Enum(value) => value,
                _ => return Err("sender selector must be a String or Enum".into()),
            };
            if value.len() > 255 || value.contains('\0') {
                return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
            }
        }
        Ok(())
    }
}
