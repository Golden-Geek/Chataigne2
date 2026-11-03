//! Reusable app-control settings. Definitions project Parameters, never opaque node data.
use super::protocol::*;
use crate::app::AppEngine;
use golden_core::{
    edit::NodeTree,
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext, ParameterValueType, UserCreatableItem},
    parameter::{Enum, File, ParamValue},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

#[node(
    "mapgyver.app_control.v1", label = "App control",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(folder(parameters, label = "Parameters", can_be_disabled = false) {
    app_file: File = File::default() (
        label = "App file", file_allowed_extensions = vec!["exe".into()]
    );
    directory: File = File::default() (label = "Working directory");
    window: Enum = "Hidden" (
        label = "Window", enum_options = crate::parameters::enum_options(&["Hidden", "Normal"])
    );
    readiness: Enum = "ProcessStarted" (
        label = "Ready when", enum_options = crate::parameters::enum_options(&["ProcessStarted", "Spout"])
    );
    sender: String = "Sender" (label = "Spout sender");
    first_frame: bool = false (label = "Wait for first frame");
    timeout_ms: f64 = 5000.0 [1.0..600000.0] (
        label = "Ready timeout (ms)", step = 1.0, constraint_policy = "reject"
    );
    inherit_environment: bool = true (label = "Inherit environment");
    graceful_ms: f64 = 2000.0 [0.0..600000.0] (
        label = "Graceful stop (ms)", step = 1.0, constraint_policy = "reject"
    );
    retirement_ms: f64 = 5000.0 [1.0..600000.0] (
        label = "Retirement timeout (ms)", step = 1.0, constraint_policy = "reject"
    );
    restart_enabled: bool = true (label = "Restart on failure");
    initial_ms: f64 = 500.0 [1.0..600000.0] (
        label = "Initial restart delay (ms)", step = 1.0, constraint_policy = "reject"
    );
    maximum_ms: f64 = 30000.0 [1.0..600000.0] (
        label = "Maximum restart delay (ms)", step = 1.0, constraint_policy = "reject"
    );
    pre: i32 = 0 [0..65535] (label = "Preload columns before", constraint_policy = "reject");
    post: i32 = 0 [0..65535] (label = "Keep columns after", constraint_policy = "reject");
})]
pub struct AppControlNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
    #[state(default = uuid::Uuid::new_v4())]
    definition_instance: uuid::Uuid,
    #[state(default = OnceLock::new())]
    invalidation: OnceLock<Arc<dyn Fn() + Send + Sync>>,
    #[state(default = AtomicBool::new(false))]
    was_configured: AtomicBool,
}

impl AppControlNode {
    pub(crate) fn embedded() -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.can_be_disabled = false;
        node.node_data_mut().meta.user_permissions = Default::default();
        node
    }
    fn from_definition(definition: &ApplicationDefinition) -> Self {
        let mut node = Self::new();
        node.app_file = ParameterHandle::new(File::new(definition.executable.as_deref().unwrap_or_default()));
        node.directory = ParameterHandle::new(File::new(definition.directory.as_deref().unwrap_or_default()));
        node.window = ParameterHandle::new(Enum::new(match definition.window {
            ApplicationWindow::Hidden => "Hidden",
            ApplicationWindow::Normal => "Normal",
        }));
        if let ApplicationReadiness::Spout {
            sender,
            first_frame,
            timeout_ms,
        } = &definition.readiness
        {
            node.readiness = ParameterHandle::new(Enum::new("Spout"));
            node.sender = ParameterHandle::new(sender.clone());
            node.first_frame = ParameterHandle::new(*first_frame);
            node.timeout_ms = ParameterHandle::new(f64::from(*timeout_ms));
        }
        node.inherit_environment = ParameterHandle::new(definition.environment.is_none());
        node.graceful_ms = ParameterHandle::new(f64::from(definition.stop.graceful_ms));
        node.retirement_ms = ParameterHandle::new(f64::from(definition.stop.retirement_ms));
        node.restart_enabled = ParameterHandle::new(definition.restart.enabled);
        node.initial_ms = ParameterHandle::new(f64::from(definition.restart.initial_ms));
        node.maximum_ms = ParameterHandle::new(f64::from(definition.restart.maximum_ms));
        node
    }
    pub(super) fn tree(label: String, definition: ApplicationDefinition, enabled: bool) -> NodeTree {
        let mut node = Self::from_definition(&definition);
        node.node_data_mut().meta.label = label;
        node.node_data_mut().meta.enabled = enabled;
        let mut tree = NodeTree::new(node);
        for value in definition.arguments {
            tree = tree.with_child(NodeTree::new(super::ArgumentNode::from_value(value)).as_user_item());
        }
        for (name, value) in definition.environment.into_iter().flatten() {
            tree = tree.with_child(NodeTree::new(super::EnvironmentNode::from_entry(name, value)).as_user_item());
        }
        tree
    }
    pub(crate) fn configured(&self) -> bool {
        !self.app_file.get_ref().as_str().is_empty()
    }
    pub(crate) fn availability(&self) -> super::demand::DefinitionAvailability {
        use super::demand::DefinitionAvailability;
        if !self.node_data().effective_enabled {
            DefinitionAvailability::Disabled
        } else if !self.configured() {
            DefinitionAvailability::Unconfigured
        } else {
            DefinitionAvailability::Configured
        }
    }
    pub fn snapshot(&self, engine: &AppEngine) -> Result<ApplicationSnapshot, String> {
        let owner = self.node_data().parent.and_then(|id| engine.nodes.get(id));
        let id = match owner {
            Some(crate::app::AppNode::MatrixCell(cell)) => cell.node_data().meta.uuid,
            _ => self.node_data().meta.uuid,
        };
        Ok(ApplicationSnapshot {
            id: ApplicationId(id),
            label: self.node_data().meta.label.clone(),
            enablement: if !self.node_data().meta.enabled {
                ApplicationEnablement::Disabled
            } else if !self.node_data().effective_enabled {
                ApplicationEnablement::DisabledByParent
            } else {
                ApplicationEnablement::Enabled
            },
            revision: self.revision.reader().current(),
            definition_instance: self.definition_instance,
            definition: self.definition(engine)?,
        })
    }
    pub fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn invalidate(&self) {
        self.revision.advance();
        // Inactive, default Cell settings do not change demand when their declared
        // Parameters materialize. Both activation and clearing a configured file do.
        let configured = self.configured();
        let previous = self.was_configured.swap(configured, Ordering::AcqRel);
        if (configured || previous)
            && let Some(notify) = self.invalidation.get()
        {
            notify();
        }
    }
    pub(crate) fn bind_invalidation(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.was_configured.store(self.configured(), Ordering::Release);
        let _ = self.invalidation.set(Arc::new(notify));
    }
    pub(crate) fn distances(&self) -> (u16, u16) {
        (self.pre.get() as u16, self.post.get() as u16)
    }
    pub(crate) fn distance_values(&self, pre: u16, post: u16) -> [(NodeId, ParamValue); 2] {
        [
            (self.pre.id(), ParamValue::Int(pre.into())),
            (self.post.id(), ParamValue::Int(post.into())),
        ]
    }
    fn scalar_values(&self) -> Vec<(NodeId, ParamValue)> {
        vec![
            (
                self.app_file.id(),
                ParamValue::File(self.app_file.get_ref().as_str().into()),
            ),
            (
                self.directory.id(),
                ParamValue::File(self.directory.get_ref().as_str().into()),
            ),
            (
                self.window.id(),
                ParamValue::Enum(self.window.get_ref().as_str().into()),
            ),
            (
                self.readiness.id(),
                ParamValue::Enum(self.readiness.get_ref().as_str().into()),
            ),
            (self.sender.id(), ParamValue::Str(self.sender.get_ref().clone())),
            (self.first_frame.id(), ParamValue::Bool(self.first_frame.get())),
            (self.timeout_ms.id(), ParamValue::Float(self.timeout_ms.get())),
            (
                self.inherit_environment.id(),
                ParamValue::Bool(self.inherit_environment.get()),
            ),
            (self.graceful_ms.id(), ParamValue::Float(self.graceful_ms.get())),
            (self.retirement_ms.id(), ParamValue::Float(self.retirement_ms.get())),
            (self.restart_enabled.id(), ParamValue::Bool(self.restart_enabled.get())),
            (self.initial_ms.id(), ParamValue::Float(self.initial_ms.get())),
            (self.maximum_ms.id(), ParamValue::Float(self.maximum_ms.get())),
        ]
    }
    fn scalar_definition(
        &self,
        change: Option<&ParameterValidationContext<'_>>,
    ) -> Result<ApplicationDefinition, String> {
        let optional = |value: String| (!value.is_empty()).then_some(value);
        let window = match read(&self.window, change)?.as_str() {
            "Hidden" => ApplicationWindow::Hidden,
            "Normal" => ApplicationWindow::Normal,
            _ => return Err("unknown app-control window mode".into()),
        };
        let readiness = match read(&self.readiness, change)?.as_str() {
            "ProcessStarted" => ApplicationReadiness::ProcessStarted,
            "Spout" => ApplicationReadiness::Spout {
                sender: read(&self.sender, change)?,
                first_frame: read(&self.first_frame, change)?,
                timeout_ms: deadline(&self.timeout_ms, change)?,
            },
            _ => return Err("unknown app-control readiness mode".into()),
        };
        Ok(ApplicationDefinition {
            executable: optional(read(&self.app_file, change)?.as_str().into()),
            directory: optional(read(&self.directory, change)?.as_str().into()),
            arguments: vec![],
            environment: (!read(&self.inherit_environment, change)?).then(Vec::new),
            window,
            readiness,
            stop: ApplicationStop {
                graceful_ms: deadline(&self.graceful_ms, change)?,
                retirement_ms: deadline(&self.retirement_ms, change)?,
            },
            restart: ApplicationRestart {
                enabled: read(&self.restart_enabled, change)?,
                initial_ms: deadline(&self.initial_ms, change)?,
                maximum_ms: deadline(&self.maximum_ms, change)?,
            },
        })
    }
    pub(super) fn definition(&self, engine: &AppEngine) -> Result<ApplicationDefinition, String> {
        self.with_entries(
            self.scalar_definition(None)?,
            &|id| engine.nodes.get(id).map(|node| node as &dyn Node),
            None,
        )
    }
    fn with_entries<'a>(
        &self,
        mut definition: ApplicationDefinition,
        lookup: &dyn Fn(NodeId) -> Option<&'a dyn Node>,
        change: Option<&ParameterValidationContext<'_>>,
    ) -> Result<ApplicationDefinition, String> {
        let mut environments = Vec::new();
        let mut next = self.node_data().first_child;
        let mut count = 0;
        while let Some(id) = next {
            let child = lookup(id).ok_or("app-control child is missing")?;
            next = child.node_data().next_sibling;
            if crate::parameters::is_parameter_folder(child) {
                continue;
            }
            count += 1;
            if count > 384 {
                return Err("app control exceeds 256 Arguments or 128 Environment entries".into());
            }
            if let Some(node) = child.as_any().downcast_ref::<super::ArgumentNode>() {
                definition.arguments.push(node.project_value(change)?);
            } else if let Some(node) = child.as_any().downcast_ref::<super::EnvironmentNode>() {
                environments.push(node.project_entry(change)?);
            } else {
                return Err("app control only owns Argument and Environment children".into());
            }
            if definition.arguments.len() > 256 || environments.len() > 128 {
                return Err("app control exceeds 256 Arguments or 128 Environment entries".into());
            }
        }
        if definition.environment.is_some() {
            definition.environment = Some(environments);
        }
        definition.validate()?;
        Ok(definition)
    }
    pub(super) fn configuration_values(&self, definition: &ApplicationDefinition) -> Vec<(NodeId, ParamValue)> {
        self.scalar_values()
            .into_iter()
            .zip(Self::from_definition(definition).scalar_values())
            .map(|((node, _), (_, value))| (node, value))
            .collect()
    }
}

pub(super) fn read<T: ParameterValueType + PartialEq>(
    handle: &ParameterHandle<T>,
    change: Option<&ParameterValidationContext<'_>>,
) -> Result<T, String> {
    match change.and_then(|change| change.parameter_value(handle.id())) {
        Some(value) => T::from_param_value(&value).ok_or_else(|| "app-control Parameter type is invalid".into()),
        None => Ok(handle.get_ref().clone()),
    }
}

fn deadline(handle: &ParameterHandle<f64>, change: Option<&ParameterValidationContext<'_>>) -> Result<u32, String> {
    let value = read(handle, change)?;
    if value.is_finite() && value.fract() == 0.0 && (0.0..=600_000.0).contains(&value) {
        Ok(value as u32)
    } else {
        Err("app-control deadlines must be whole milliseconds in 0..600000".into())
    }
}

#[node("mapgyver.app_control.v1", from_struct)]
impl Node for AppControlNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.invalidate();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.invalidate();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.invalidate();
    }
    fn project_decode_data(&mut self, value: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(value)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        self.scalar_definition(None)?.validate()
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        self.with_entries(
            self.scalar_definition(Some(change))?,
            &|id| change.node(id),
            Some(change),
        )
        .map(|_| ())
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            super::entries::ARGUMENT_TYPE => Some(Box::new(super::ArgumentNode::new())),
            super::entries::ENVIRONMENT_TYPE => Some(Box::new(super::EnvironmentNode::new())),
            _ => None,
        }
    }
    fn create_user_item_tree_with_context(
        &self,
        node_type: &str,
        context: &golden_core::node::UserItemCreationContext<'_>,
    ) -> Option<NodeTree> {
        let limit = match node_type {
            super::entries::ARGUMENT_TYPE => 256,
            super::entries::ENVIRONMENT_TYPE => 128,
            _ => return None,
        };
        let mut next = self.node_data().first_child;
        let mut count = 0;
        while let Some(id) = next {
            let child = context.node(id)?;
            next = child.node_data().next_sibling;
            if child.get_type() == node_type {
                count += 1;
            }
            if count == limit {
                return None;
            }
        }
        self.create_user_item_tree(node_type)
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(super::entries::ARGUMENT_TYPE, super::entries::ARGUMENT_TYPE, "Argument")
                .with_select_when_created(false),
            UserCreatableItem::new(
                super::entries::ENVIRONMENT_TYPE,
                super::entries::ENVIRONMENT_TYPE,
                "Environment",
            )
            .with_select_when_created(false),
        ]
    }
    fn user_container_rules(&self) -> Option<golden_core::node::UserContainerRules> {
        Some(golden_core::node::UserContainerRules::new(&[
            super::entries::ARGUMENT_TYPE,
            super::entries::ENVIRONMENT_TYPE,
        ]))
    }
}
