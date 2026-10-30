//! App-owned asset catalog nodes. Source locators are authored; clocks and decoded bytes are not.
use crate::app::{AppEngine, AppNode};
use golden_assets::{AssetId, AssetLocator, AssetRecord};
use golden_core::model::NodeUuid;
use golden_core::node::{Node, NodeData, NodeId};
use golden_core::process_ctx::ProcessCtx;
use golden_io::{RevisionGuard, RevisionSource};
use serde_json::Value;
use std::any::Any;

pub struct AssetNode {
    data: NodeData,
    locator: Option<AssetLocator>,
    revision: RevisionSource,
}

impl AssetNode {
    pub(super) fn new(label: String, locator: Option<AssetLocator>) -> Self {
        Self {
            data: NodeData::new(label),
            locator,
            revision: RevisionSource::default(),
        }
    }
    pub fn id(&self) -> AssetId {
        AssetId::from_uuid(self.data.meta.uuid.0)
    }
    pub(super) fn snapshot(&self) -> super::protocol::AssetSnapshot {
        super::protocol::AssetSnapshot {
            id: self.id(),
            label: self.data.meta.label.clone(),
            locator: self.locator.clone(),
        }
    }
    pub fn record(&self) -> Result<AssetRecord, String> {
        Ok(AssetRecord {
            id: self.id(),
            locator: self.locator.clone().ok_or("asset source is unconfigured")?,
        })
    }
    pub(super) fn set_locator(&mut self, locator: Option<AssetLocator>) {
        if self.locator != locator {
            self.revision.advance();
            self.locator = locator;
        }
    }
    pub(super) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
}

impl Node for AssetNode {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "mapgyver.media.asset"
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "mapgyver.media.asset").then(|| Self::new("Asset".into(), None))
    }
    fn project_encode_data(&self) -> Result<Value, String> {
        serde_json::to_value(&self.locator).map_err(|error| error.to_string())
    }
    fn project_decode_data(&mut self, value: &Value) -> Result<(), String> {
        self.set_locator(serde_json::from_value(value.clone()).map_err(|error| error.to_string())?);
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
