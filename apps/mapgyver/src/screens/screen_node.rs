//! Screen output settings are typed Parameters; definitions are validated projections.
use super::protocol::{PhysicalRectangle, ScreenDefinition, ScreenTarget};
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
        target_kind: Enum = "Unconfigured" (label = "Target", enum_options = crate::parameters::enum_options(&["Unconfigured", "Fullscreen", "Rectangle", "Spout"]));
        display: String = String::new() (label = "Display identity");
        left: i32 = 0 (label = "Left", description = "Physical desktop pixels.");
        top: i32 = 0 (label = "Top", description = "Physical desktop pixels.");
        width: i32 = 1920 [1..8192] (label = "Width", description = "Physical output pixels.", constraint_policy = "reject");
        height: i32 = 1080 [1..8192] (label = "Height", description = "Physical output pixels.", constraint_policy = "reject");
        sender: String = String::new() (label = "Sender name");
        render_enabled: bool = false (label = "Enabled");
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
        node.render_enabled = ParameterHandle::new(definition.enabled);
        match definition.target {
            Some(ScreenTarget::Fullscreen { display }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Fullscreen"));
                node.display = ParameterHandle::new(display);
            }
            Some(ScreenTarget::Rectangle { bounds }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Rectangle"));
                node.left = ParameterHandle::new(bounds.left);
                node.top = ParameterHandle::new(bounds.top);
                node.width = ParameterHandle::new(bounds.extent.width() as i32);
                node.height = ParameterHandle::new(bounds.extent.height() as i32);
            }
            Some(ScreenTarget::Spout { sender, extent }) => {
                node.target_kind = ParameterHandle::new(Enum::new("Spout"));
                node.sender = ParameterHandle::new(sender);
                node.width = ParameterHandle::new(extent.width() as i32);
                node.height = ParameterHandle::new(extent.height() as i32);
            }
            None => {}
        }
        node
    }

    pub fn definition(&self) -> ScreenDefinition {
        self.values(None).definition()
    }

    pub(crate) fn parameter_values(&self, definition: ScreenDefinition) -> Vec<(NodeId, ParamValue)> {
        let mut values = vec![(self.render_enabled.id(), ParamValue::Bool(definition.enabled))];
        let kind = match definition.target {
            Some(ScreenTarget::Fullscreen { display }) => {
                values.push((self.display.id(), ParamValue::Str(display)));
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
                "Spout"
            }
            None => "Unconfigured",
        };
        values.push((self.target_kind.id(), ParamValue::Enum(kind.into())));
        values
    }

    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }

    fn parameter_ids(&self) -> [NodeId; 8] {
        [
            self.target_kind.id(),
            self.display.id(),
            self.left.id(),
            self.top.id(),
            self.width.id(),
            self.height.id(),
            self.sender.id(),
            self.render_enabled.id(),
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
            enabled: read(&self.render_enabled, change),
        }
    }
}

#[node("mapgyver.screen", from_struct)]
impl Node for ScreenNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["mapgyver.surface"]))
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

    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }

    fn on_param_change(&mut self, _: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if self.parameter_ids().contains(&parameter) {
            self.revision.advance();
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }

    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.values(None).validate()
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if !self.parameter_ids().contains(&change.parameter) {
            return Ok(());
        }
        let candidate = self.values(Some(change));
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
                return Err("enabled Screens must have distinct Spout sender names".into());
            }
        }
        Ok(())
    }
}

struct ScreenValues {
    kind: Enum,
    display: String,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    sender: String,
    enabled: bool,
}

impl ScreenValues {
    fn definition(&self) -> ScreenDefinition {
        // Empty endpoint text is an authored, visibly unconfigured state. It must
        // not select a primary display or invent a sender during ordinary editing.
        let target = match self.kind.as_str() {
            "Fullscreen" if !self.display.is_empty() => Some(ScreenTarget::Fullscreen {
                display: self.display.clone(),
            }),
            "Rectangle" => Some(ScreenTarget::Rectangle {
                bounds: PhysicalRectangle {
                    left: self.left,
                    top: self.top,
                    extent: projection_extent(self.width, self.height),
                },
            }),
            "Spout" if !self.sender.is_empty() => Some(ScreenTarget::Spout {
                sender: self.sender.clone(),
                extent: projection_extent(self.width, self.height),
            }),
            _ => None,
        };
        ScreenDefinition {
            target,
            enabled: self.enabled,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if !["Unconfigured", "Fullscreen", "Rectangle", "Spout"].contains(&self.kind.as_str()) {
            return Err("unknown Screen target kind".into());
        }
        if !(1..=MAX_SCREEN_DIMENSION).contains(&self.width) || !(1..=MAX_SCREEN_DIMENSION).contains(&self.height) {
            return Err("screen extent must contain 1..8192 physical pixels per axis".into());
        }
        validate_endpoint(&self.display, 1024, "display identity")?;
        validate_endpoint(&self.sender, 239, "Spout sender")?;
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
