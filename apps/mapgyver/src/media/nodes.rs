use std::{any::Any, sync::Arc};

use golden_core::events::CustomEvent;
use golden_core::node::{Node, NodeData, NodeScriptDescriptor, UserContainerRules, UserCreatableItem};
use golden_core::parameter::ParamValue;
use golden_core::process_ctx::ProcessCtx;
use golden_core::script::ScriptHostPolicy;
use golden_io::{RevisionGuard, RevisionSource};

pub struct MediaManagerNode {
    data: NodeData,
    pub(crate) instance: uuid::Uuid,
    pub(crate) sources: Arc<super::retention::RuntimeSources>,
    pub(crate) resolutions: Arc<super::resolution::RuntimeResolutions>,
    pub(crate) applications: crate::applications::runtime::RuntimeView,
    pub(crate) application_revision: Arc<RevisionSource>,
    project: RevisionSource,
    membership: RevisionSource,
    operation_owner: Arc<()>,
}

impl MediaManagerNode {
    pub(crate) fn new() -> Self {
        let mut data = NodeData::new("Media Manager".into());
        data.meta.decl_id = golden_core::node::DeclId("mapgyver_media_manager".into());
        Self {
            data,
            instance: uuid::Uuid::new_v4(),
            sources: Default::default(),
            resolutions: Default::default(),
            applications: Default::default(),
            application_revision: Default::default(),
            project: RevisionSource::default(),
            membership: RevisionSource::default(),
            operation_owner: Arc::new(()),
        }
    }
    pub(crate) fn project_guard(&self) -> RevisionGuard {
        let reader = self.project.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn membership_guard(&self) -> RevisionGuard {
        let reader = self.membership.reader();
        reader.guard_at(reader.current())
    }
}

impl Node for MediaManagerNode {
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
        self.project.advance();
        self.membership.advance();
        self.instance = uuid::Uuid::new_v4();
        self.sources = Default::default();
        self.resolutions = Default::default();
        self.applications = Default::default();
        self.application_revision.advance();
        self.operation_owner = Arc::new(());
    }
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media.manager"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.media.manager").then(Self::new)
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }

    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        Some(ScriptHostPolicy::default_scriptable())
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            "mapgyver.media.v5",
            super::reference::NODE_TYPE,
            "mapgyver.media.folder",
            "mapgyver.media.asset",
            "mapgyver.application",
            "script",
            super::commands::NODE_TYPE,
        ]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        super::creation::items()
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<golden_core::edit::NodeTree> {
        super::creation::tree(node_type)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        super::scripting::manager_descriptor()
    }

    fn engine_call_custom_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        super::scripting::manager_method(ctx, self.id(), &self.operation_owner, method, args)
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        super::scripting::forward_result(ctx, self.id(), event);
    }

    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.operation_owner = Arc::new(());
        }
    }
}
