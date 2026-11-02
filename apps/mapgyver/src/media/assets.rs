//! Asset locators are typed Parameters; prepared bytes and clocks remain runtime state.
use crate::app::{AppEngine, AppNode};
use golden_assets::{AbsoluteAssetPath, AssetId, AssetLocator, AssetRecord, RelativeAssetPath};
use golden_core::{
    edit::Edit,
    model::NodeUuid,
    node,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext},
    parameter::{Enum, File, ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

#[node(
    "mapgyver.media.asset",
    label = "Asset",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(
    folder(parameters, label = "Parameters") {
        locator_kind: Enum = "Unconfigured" (label = "Location", enum_options = crate::parameters::enum_options(&["Unconfigured", "ProjectRelative", "Absolute"]));
        project_path: File = File::default() (label = "Project path");
        absolute_path: File = File::default() (label = "Absolute path");
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
        match locator {
            Some(AssetLocator::ProjectRelative { path }) => {
                node.locator_kind = ParameterHandle::new(Enum::new("ProjectRelative"));
                node.project_path = ParameterHandle::new(File::new(path.as_str()));
            }
            Some(AssetLocator::Absolute { path }) => {
                node.locator_kind = ParameterHandle::new(Enum::new("Absolute"));
                node.absolute_path = ParameterHandle::new(File::new(path.as_str()));
            }
            None => {}
        }
        node
    }
    pub fn id(&self) -> AssetId {
        AssetId::from_uuid(self.node_data().meta.uuid.0)
    }
    fn locator(&self) -> Option<AssetLocator> {
        match self.locator_kind.get_ref().as_str() {
            "ProjectRelative" => RelativeAssetPath::try_from(self.project_path.get_ref().as_str().to_owned())
                .ok()
                .map(|path| AssetLocator::ProjectRelative { path }),
            "Absolute" => AbsoluteAssetPath::try_from(self.absolute_path.get_ref().as_str().to_owned())
                .ok()
                .map(|path| AssetLocator::Absolute { path }),
            _ => None,
        }
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
        let (node, path, kind) = match locator {
            AssetLocator::ProjectRelative { path } => {
                (self.project_path.id(), path.as_str().to_owned(), "ProjectRelative")
            }
            AssetLocator::Absolute { path } => (self.absolute_path.id(), path.as_str().to_owned(), "Absolute"),
        };
        vec![
            Edit::SetParam {
                node,
                value: ParamValue::File(path),
                behaviour: ParameterEventBehaviour::Append,
            },
            Edit::SetParam {
                node: self.locator_kind.id(),
                value: ParamValue::Enum(kind.into()),
                behaviour: ParameterEventBehaviour::Append,
            },
        ]
    }
    pub(super) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn reload(&mut self) {
        self.revision.advance();
    }
}
#[node("mapgyver.media.asset", from_struct)]
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
        if !["Unconfigured", "ProjectRelative", "Absolute"].contains(&self.locator_kind.get_ref().as_str()) {
            return Err("unknown asset location kind".into());
        }
        if !self.project_path.get_ref().as_str().is_empty() {
            RelativeAssetPath::try_from(self.project_path.get_ref().as_str().to_owned())
                .map_err(|error| error.to_string())?;
        }
        if !self.absolute_path.get_ref().as_str().is_empty() {
            AbsoluteAssetPath::try_from(self.absolute_path.get_ref().as_str().to_owned())
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if let ParamValue::File(path) = change.value
            && !path.is_empty()
        {
            if change.parameter == self.project_path.id() {
                RelativeAssetPath::try_from(path.clone()).map_err(|error| error.to_string())?;
            }
            if change.parameter == self.absolute_path.id() {
                AbsoluteAssetPath::try_from(path.clone()).map_err(|error| error.to_string())?;
            }
        }
        Ok(())
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
