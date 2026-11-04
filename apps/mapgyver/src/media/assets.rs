//! Asset locators are typed Parameters; prepared bytes and clocks remain runtime state.
use crate::app::{AppEngine, AppNode};
use golden_assets::{AbsoluteAssetPath, AssetId, AssetLocator, AssetRecord, RelativeAssetPath};
use golden_core::{
    edit::Edit,
    model::NodeUuid,
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext},
    parameter::{File, ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

#[node(
    "mapgyver.media.asset.v2",
    label = "Asset",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(
    folder(parameters, label = "Parameters") {
        file: File = File::default() (label = "File", description = "Project-relative or native absolute source path. Empty leaves the asset unconfigured.");
    }
)]
pub struct AssetNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}
impl AssetNode {
    pub(super) fn from_locator(label: String, locator: Option<AssetLocator>) -> Self {
        let mut node = Self::new();
        let permissions = node.node_data().meta.user_permissions.clone();
        node.node_data_mut().meta = golden_core::node::NodeMeta::new(label).with_user_permissions(permissions);
        if let Some(locator) = locator {
            node.file = ParameterHandle::new(File::new(locator_path(&locator)));
        }
        node
    }
    pub fn id(&self) -> AssetId {
        AssetId::from_uuid(self.node_data().meta.uuid.0)
    }
    fn locator(&self) -> Option<AssetLocator> {
        parse_file(self.file.get_ref().as_str()).ok().flatten()
    }
    pub(super) fn snapshot(&self) -> super::protocol::AssetSnapshot {
        super::protocol::AssetSnapshot {
            id: self.id(),
            label: self.node_data().meta.label.clone(),
            locator: self.locator(),
        }
    }
    pub fn record(&self) -> Result<AssetRecord, String> {
        Ok(AssetRecord {
            id: self.id(),
            locator: self.locator().ok_or("asset source is unconfigured")?,
        })
    }
    pub(super) fn locator_edits(&self, locator: AssetLocator) -> Vec<Edit> {
        vec![Edit::SetParam {
            node: self.file.id(),
            value: ParamValue::File(locator_path(&locator).to_owned()),
            behaviour: ParameterEventBehaviour::Append,
        }]
    }
    pub(super) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn reload(&mut self) {
        self.revision.advance();
    }
}
#[node("mapgyver.media.asset.v2", from_struct)]
impl Node for AssetNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.revision.advance();
    }
    fn project_decode_data(&mut self, value: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(value)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        parse_file(self.file.get_ref().as_str()).map(|_| ())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.file.id()
            && let ParamValue::File(path) = change.value
        {
            parse_file(path)?;
        }
        Ok(())
    }
}

fn locator_path(locator: &AssetLocator) -> &str {
    match locator {
        AssetLocator::ProjectRelative { path } => path.as_str(),
        AssetLocator::Absolute { path } => path.as_str(),
    }
}

/// Location policy belongs to asset authoring, shared by generic edits and app intents.
fn parse_file(path: &str) -> Result<Option<AssetLocator>, String> {
    if path.is_empty() {
        return Ok(None);
    }
    if std::path::Path::new(path).is_absolute() {
        AbsoluteAssetPath::try_from(path.to_owned())
            .map(|path| Some(AssetLocator::Absolute { path }))
            .map_err(|error| error.to_string())
    } else {
        RelativeAssetPath::try_from(path.to_owned())
            .map(|path| Some(AssetLocator::ProjectRelative { path }))
            .map_err(|error| error.to_string())
    }
}
pub(super) fn node_id(engine: &AppEngine, asset: AssetId) -> Result<NodeId, String> {
    let id = engine
        .node_id_by_uuid(NodeUuid(asset.get()))
        .ok_or("asset reference is missing")?;
    let Some(AppNode::MediaAsset(node)) = engine.nodes.get(id) else {
        return Err("reference is not an asset".into());
    };
    if node.node_data().parent != Some(super::authoring::manager(engine)?) {
        return Err("asset is outside its catalog".into());
    }
    Ok(id)
}

pub fn asset(engine: &AppEngine, asset: AssetId) -> Result<AssetRecord, String> {
    let id = node_id(engine, asset)?;
    let Some(AppNode::MediaAsset(node)) = engine.nodes.get(id) else {
        unreachable!()
    };
    node.record()
}
