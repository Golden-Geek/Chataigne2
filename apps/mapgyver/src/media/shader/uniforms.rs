use golden_core::node::NodeId;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::app::AppEngine;
#[cfg(all(windows, feature = "native-video"))]
use {
    crate::app::AppNode,
    golden_core::{node::Node, parameter::ParamValue},
    std::{collections::BTreeMap, sync::Arc},
};

#[cfg(all(windows, feature = "native-video"))]
mod authoring;
#[cfg(all(windows, feature = "native-video"))]
pub(crate) use authoring::{add, inspect};

#[cfg(not(all(windows, feature = "native-video")))]
pub(crate) fn inspect(
    _: &AppEngine,
    _: super::super::protocol::ChannelReference,
) -> Result<Vec<ShaderUniformRow>, String> {
    Ok(Vec::new())
}

#[cfg(not(all(windows, feature = "native-video")))]
pub(crate) fn add(
    _: &mut AppEngine,
    _: super::super::protocol::ChannelReference,
    _: Option<&str>,
) -> Result<(), String> {
    Err("Shader uniforms require the Windows native video runtime".into())
}

const FOLDER: &str = "parameters/shader_uniforms";
const PREFIX: &str = "parameters/shader_uniforms/";
#[cfg(all(windows, feature = "native-video"))]
const LIMIT: usize = 64;

/// One detected uniform and its parameter when exposed on the Channel.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ShaderUniformRow {
    pub name: String,
    pub label: String,
    #[ts(type = "number | null")]
    pub parameter: Option<NodeId>,
}

#[cfg(all(windows, feature = "native-video"))]
pub(in crate::media) type Values = Arc<BTreeMap<String, ParamValue>>;

#[cfg(all(windows, feature = "native-video"))]
fn children(engine: &AppEngine, parent: NodeId) -> impl Iterator<Item = &AppNode> {
    std::iter::successors(
        engine.nodes.get(parent).and_then(|node| node.node_data().first_child),
        |&id| engine.nodes.get(id).and_then(|node| node.node_data().next_sibling),
    )
    .filter_map(|id| engine.nodes.get(id))
}

#[cfg(all(windows, feature = "native-video"))]
fn parameters(engine: &AppEngine, channel: NodeId) -> Option<NodeId> {
    children(engine, channel)
        .find(|node| crate::parameters::is_parameter_folder(*node))
        .map(|node| node.node_data().id)
}

#[cfg(all(windows, feature = "native-video"))]
fn folder(engine: &AppEngine, parameters: NodeId) -> Option<NodeId> {
    children(engine, parameters)
        .find(|node| node.get_type() == "folder" && node.node_data().meta.decl_id.0 == FOLDER)
        .map(|node| node.node_data().id)
}

#[cfg(all(windows, feature = "native-video"))]
pub(in crate::media) fn capture(engine: &AppEngine, channel: NodeId) -> Values {
    let Some(folder) = parameters(engine, channel).and_then(|parent| folder(engine, parent)) else {
        return Default::default();
    };
    Arc::new(
        children(engine, folder)
            .take(LIMIT)
            .filter_map(|node| {
                let AppNode::Parameter(parameter) = node else {
                    return None;
                };
                let name = parameter.node_data().meta.decl_id.0.strip_prefix(PREFIX)?;
                Some((name.to_owned(), parameter.get().clone()))
            })
            .collect(),
    )
}

pub(in crate::media) fn is_uniform(decl_id: &str) -> bool {
    decl_id.starts_with(PREFIX) || decl_id == FOLDER
}

#[cfg(all(windows, feature = "native-video"))]
pub(in crate::media) fn value(value: &ParamValue) -> Option<golden_video::shader::UniformValue> {
    use golden_video::shader::UniformValue;

    Some(match *value {
        ParamValue::Float(value) => UniformValue::Float(value as f32),
        ParamValue::Int(value) => UniformValue::Int(value),
        ParamValue::Bool(value) => UniformValue::Bool(value),
        ParamValue::Vec2(x, y) => UniformValue::Point([x as f32, y as f32]),
        ParamValue::Color(r, g, b, a) => UniformValue::Color([r as f32, g as f32, b as f32, a as f32]),
        _ => return None,
    })
}
