//! Screen output settings are typed Parameters; definitions are validated projections.
use super::protocol::{PhysicalRectangle, ScreenAntiAliasing, ScreenDefinition, ScreenFrameRate, ScreenTarget};
use golden_core::{
    node,
    node::{
        Node, NodeId, NodeMeta, ParameterHandle, ParameterValidationContext, ParameterValueType, UserContainerRules,
    },
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use golden_video::Extent2D;

const MAX_SCREEN_DIMENSION: i32 = 8192;
mod hints;
mod selection;

#[node(
    "mapgyver.screen",
    label = "Screen",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(
    folder(parameters, label = "Parameters") {
        target_kind: Enum = "Shared Texture" (label = "Target", enum_options = crate::parameters::enum_options(&["Fullscreen", "Rectangle", "Shared Texture"]));
        display: Enum = "" (label = "Display");
        left: i32 = 0 (label = "Left", description = "Physical desktop pixels.");
        top: i32 = 0 (label = "Top", description = "Physical desktop pixels.");
        width: i32 = 1920 [1..8192] (label = "Width", description = "Physical output pixels.", constraint_policy = "reject");
        height: i32 = 1080 [1..8192] (label = "Height", description = "Physical output pixels.", constraint_policy = "reject");
        sender: String = "Screen".to_owned() (label = "Sender name");
        anti_aliasing: Enum = "Raw" (label = "Anti-aliasing", enum_options = crate::parameters::enum_options(&["Raw", "Smooth (2×)"]), constraint_policy = "reject");
        frame_rate_mode: Enum = "Follow display" (label = "Frame rate", enum_options = crate::parameters::enum_options(&["Follow display", "Explicit"]), constraint_policy = "reject", description = "Follow the resolved display refresh. Shared Texture or unknown display refresh uses 60 FPS. Editing and previews retain their independent cadence.");
        frames_per_second: f64 = 60.0 [1.0..1000.0] (label = "FPS", constraint_policy = "reject");
        display_edid: String = String::new() (label = "Display EDID", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_name: String = String::new() (label = "Display name hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_os_index: i32 = 0 [0..2147483647] (label = "Display number hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_left: i32 = 0 (label = "Display left hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_top: i32 = 0 (label = "Display top hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_width: i32 = 0 [0..2147483647] (label = "Display width hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
        display_height: i32 = 0 [0..2147483647] (label = "Display height hint", read_only = true, persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false);
    }
)]
pub struct ScreenNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}

impl ScreenNode {
    pub(crate) fn from_definition(label: String, definition: ScreenDefinition) -> Self {
        let mut node = Self::new();
        let permissions = node.node_data().meta.user_permissions.clone();
        node.node_data_mut().meta = NodeMeta::new(label).with_user_permissions(permissions);
        node.node_data_mut().meta.enabled = definition.enabled;
        node.anti_aliasing = ParameterHandle::new(Enum::new(definition.anti_aliasing.name()));
        match definition.frame_rate {
            ScreenFrameRate::FollowDisplay => {}
            ScreenFrameRate::Explicit { fps } => {
                node.frame_rate_mode = ParameterHandle::new(Enum::new("Explicit"));
                node.frames_per_second = ParameterHandle::new(fps);
            }
        }
        match definition.target {
            Some(ScreenTarget::Fullscreen { display }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Fullscreen"));
                node.display = ParameterHandle::new(Enum::new(display));
            }
            Some(ScreenTarget::Rectangle { bounds }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Rectangle"));
                node.left = ParameterHandle::new(bounds.left);
                node.top = ParameterHandle::new(bounds.top);
                node.width = ParameterHandle::new(bounds.extent.width() as i32);
                node.height = ParameterHandle::new(bounds.extent.height() as i32);
            }
            Some(ScreenTarget::Spout { sender, extent }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Shared Texture"));
                node.sender = ParameterHandle::new(sender);
                node.width = ParameterHandle::new(extent.width() as i32);
                node.height = ParameterHandle::new(extent.height() as i32);
            }
            None => {
                node.sender = ParameterHandle::new(String::new());
            }
        }
        node
    }

    pub fn definition(&self) -> ScreenDefinition {
        self.values(None).definition()
    }

    pub(crate) fn parameter_values(&self, definition: ScreenDefinition) -> Vec<(NodeId, ParamValue)> {
        let mut values = Vec::new();
        let kind = match definition.target {
            Some(ScreenTarget::Fullscreen { display }) => {
                values.push((self.display.id(), ParamValue::Enum(display)));
                "Fullscreen"
            }
            Some(ScreenTarget::Rectangle { bounds }) => {
                values.extend([
                    (self.left.id(), ParamValue::Int(bounds.left)),
                    (self.top.id(), ParamValue::Int(bounds.top)),
                    (self.width.id(), ParamValue::Int(bounds.extent.width() as i32)),
                    (self.height.id(), ParamValue::Int(bounds.extent.height() as i32)),
                ]);
                "Rectangle"
            }
            Some(ScreenTarget::Spout { sender, extent }) => {
                values.extend([
                    (self.sender.id(), ParamValue::Str(sender)),
                    (self.width.id(), ParamValue::Int(extent.width() as i32)),
                    (self.height.id(), ParamValue::Int(extent.height() as i32)),
                ]);
                "Shared Texture"
            }
            None => {
                values.push((self.sender.id(), ParamValue::Str(String::new())));
                "Shared Texture"
            }
        };
        values.push((self.target_kind.id(), ParamValue::Enum(kind.into())));
        let mode = match definition.frame_rate {
            ScreenFrameRate::FollowDisplay => "Follow display",
            ScreenFrameRate::Explicit { fps } => {
                values.push((self.frames_per_second.id(), ParamValue::Float(fps)));
                "Explicit"
            }
        };
        values.push((self.frame_rate_mode.id(), ParamValue::Enum(mode.into())));
        values.push((
            self.anti_aliasing.id(),
            ParamValue::Enum(definition.anti_aliasing.name().into()),
        ));
        values
    }

    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }

    fn parameter_ids(&self) -> [NodeId; 17] {
        [
            self.target_kind.id(),
            self.display.id(),
            self.left.id(),
            self.top.id(),
            self.width.id(),
            self.height.id(),
            self.sender.id(),
            self.anti_aliasing.id(),
            self.frame_rate_mode.id(),
            self.frames_per_second.id(),
            self.display_edid.id(),
            self.display_name.id(),
            self.display_os_index.id(),
            self.display_left.id(),
            self.display_top.id(),
            self.display_width.id(),
            self.display_height.id(),
        ]
    }

    fn values(&self, change: Option<&ParameterValidationContext<'_>>) -> ScreenValues {
        ScreenValues {
            kind: read(&self.target_kind, change),
            display: read(&self.display, change),
            left: read(&self.left, change),
            top: read(&self.top, change),
            width: read(&self.width, change),
            height: read(&self.height, change),
            sender: read(&self.sender, change),
            anti_aliasing: read(&self.anti_aliasing, change),
            frame_rate_mode: read(&self.frame_rate_mode, change),
            frames_per_second: read(&self.frames_per_second, change),
            enabled: self.node_data().meta.enabled,
        }
    }

    fn catalog_changed(&self, ctx: &mut ProcessCtx) {
        if let Some(parent) = self.node_data().parent {
            ctx.call_node_mutation_without_snapshot(parent, |node, _| {
                if let Some(manager) = node.as_any_mut().downcast_mut::<super::ScreenManagerNode>() {
                    manager.mark_catalog_dirty();
                }
                Ok(())
            });
        }
    }
}

#[node("mapgyver.screen", from_struct)]
impl Node for ScreenNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["mapgyver.surface"]))
    }
    fn user_creatable_items(&self) -> Vec<golden_core::node::UserCreatableItem> {
        vec![golden_core::node::UserCreatableItem::new(
            "mapgyver.surface",
            "mapgyver.surface",
            "Surface",
        )]
    }
    fn create_user_item_tree(&self, node_type: &str) -> Option<golden_core::edit::NodeTree> {
        (node_type == "mapgyver.surface").then(|| {
            golden_core::edit::NodeTree::new(super::SurfaceNode::from_definition(
                "Surface".into(),
                super::protocol::SurfaceDefinition::default(),
            ))
        })
    }
    fn needs_update(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }

    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }

    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, _: bool) {
        self.revision.advance();
        self.catalog_changed(ctx);
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: NodeId, old: ParamValue) {
        self.log_selection_change(parameter, &old);
        if self.parameter_ids().contains(&parameter) || self.is_display_hint_parameter(parameter) {
            self.revision.advance();
        }
        if parameter == self.display.id()
            || parameter == self.target_kind.id()
            || self.is_display_hint_parameter(parameter)
        {
            self.catalog_changed(ctx);
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }

    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.validate_display_hint(None)?;
        self.values(None).validate()
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if !self.parameter_ids().contains(&change.parameter) {
            return Ok(());
        }
        let candidate = self.values(Some(change));
        self.validate_display_hint(Some(change))?;
        candidate.validate()?;
        let definition = candidate.definition();
        let Some(ScreenTarget::Spout { sender, .. }) = definition.target.filter(|_| definition.enabled) else {
            return Ok(());
        };
        let mut child = self
            .node_data()
            .parent
            .and_then(|parent| change.node(parent))
            .and_then(|node| node.node_data().first_child);
        while let Some(id) = child {
            let Some(node) = change.node(id) else { break };
            child = node.node_data().next_sibling;
            if id == self.id() {
                continue;
            }
            let Some(screen) = node.as_any().downcast_ref::<ScreenNode>() else {
                continue;
            };
            let other = screen.values(Some(change)).definition();
            if other.enabled
                && matches!(other.target, Some(ScreenTarget::Spout { sender: other, .. }) if other == sender)
            {
                return Err("enabled Screens must have distinct Shared Texture sender names".into());
            }
        }
        Ok(())
    }

    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        self.selection_changes(change)
    }
}

struct ScreenValues {
    kind: Enum,
    display: Enum,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    sender: String,
    anti_aliasing: Enum,
    frame_rate_mode: Enum,
    frames_per_second: f64,
    enabled: bool,
}

impl ScreenValues {
    fn definition(&self) -> ScreenDefinition {
        // Empty endpoint text is an authored, visibly unconfigured state. It must
        // not select a primary display or invent a sender during ordinary editing.
        let target = match self.kind.as_str() {
            "Fullscreen" if !self.display.as_str().is_empty() => Some(ScreenTarget::Fullscreen {
                display: self.display.as_str().to_owned(),
            }),
            "Rectangle" => Some(ScreenTarget::Rectangle {
                bounds: PhysicalRectangle {
                    left: self.left,
                    top: self.top,
                    extent: projection_extent(self.width, self.height),
                },
            }),
            "Shared Texture" if !self.sender.is_empty() => Some(ScreenTarget::Spout {
                sender: self.sender.clone(),
                extent: projection_extent(self.width, self.height),
            }),
            _ => None,
        };
        ScreenDefinition {
            target,
            enabled: self.enabled,
            frame_rate: if self.frame_rate_mode.as_str() == "Explicit" {
                ScreenFrameRate::Explicit {
                    fps: self.frames_per_second,
                }
            } else {
                ScreenFrameRate::FollowDisplay
            },
            anti_aliasing: ScreenAntiAliasing::parse(self.anti_aliasing.as_str())
                .expect("validated Screen anti-aliasing"),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !["Follow display", "Explicit"].contains(&self.frame_rate_mode.as_str()) {
            return Err("unknown Screen frame rate mode".into());
        }
        ScreenFrameRate::explicit_rate(self.frames_per_second)?;
        if ScreenAntiAliasing::parse(self.anti_aliasing.as_str()).is_none() {
            return Err("unknown Screen anti-aliasing mode".into());
        }
        if !["Fullscreen", "Rectangle", "Shared Texture"].contains(&self.kind.as_str()) {
            return Err("unknown Screen target kind".into());
        }
        if !(1..=MAX_SCREEN_DIMENSION).contains(&self.width) || !(1..=MAX_SCREEN_DIMENSION).contains(&self.height) {
            return Err("screen extent must contain 1..8192 physical pixels per axis".into());
        }
        validate_endpoint(self.display.as_str(), 1024, "display identity")?;
        validate_endpoint(&self.sender, 239, "Shared Texture sender")?;
        self.definition().validate()
    }
}

fn read<T: ParameterValueType + PartialEq>(
    handle: &ParameterHandle<T>,
    change: Option<&ParameterValidationContext<'_>>,
) -> T {
    change
        .and_then(|change| change.parameter_value(handle.id()))
        .and_then(|value| T::from_param_value(&value))
        .unwrap_or_else(|| handle.get_ref().clone())
}

fn validate_endpoint(value: &str, maximum: usize, name: &str) -> Result<(), String> {
    if value.len() > maximum || value.contains('\0') || (!value.is_empty() && value.trim().is_empty()) {
        return Err(format!(
            "{name} must be empty or contain 1..{maximum} UTF-8 bytes without NUL"
        ));
    }
    Ok(())
}

fn projection_extent(width: i32, height: i32) -> Extent2D {
    // A detached malformed record may be projected before candidate validation;
    // the validator rejects its raw values instead of panicking during binding.
    Extent2D::new(width as u32, height as u32).unwrap_or_else(|_| crate::media::resolution::default_canvas())
}
