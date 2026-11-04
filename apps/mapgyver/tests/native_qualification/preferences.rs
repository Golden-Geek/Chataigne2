//! Generates a previously selected, now unavailable GPU preference through the real typed codec.
use super::config::Arguments;
use golden_core::{
    app::{
        ProjectLifecycle, ensure_preferences_defaults, prepare_engine_for_runtime, to_sparse_preferences_json_pretty,
    },
    edit::Edit,
    node::Node,
    parameter::{ParamValue, ParameterEnumOption},
};
use mapgyver::app::AppNode;
use std::{fs, io};

pub fn write(mut args: Arguments) -> io::Result<()> {
    let file = args.required("file")?;
    let selected = args.required("selected")?;
    args.done()?;
    if golden_video::native::AdapterId::from_key(&selected).is_none() {
        return Err(io::Error::other("fixture requires an exact GPU identity"));
    }
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().map_err(io::Error::other)?;
    ensure_preferences_defaults(&mut engine, AppNode::preferences_tree(String::new()));
    prepare_engine_for_runtime(&mut engine).map_err(io::Error::other)?;
    let snapshot = engine.process_tree_snapshot();
    let root =
        golden_core::app::live_preferences_root(&engine).ok_or_else(|| io::Error::other("Preferences missing"))?;
    let video = snapshot
        .find_child_by_decl_id(root, "mapgyver_video")
        .ok_or_else(|| io::Error::other("Video missing"))?;
    let parameter = snapshot
        .find_child_by_decl_id(video, "adapter")
        .ok_or_else(|| io::Error::other("GPU missing"))?;
    let leaf = engine
        .nodes
        .get(parameter)
        .ok_or_else(|| io::Error::other("GPU missing"))?;
    engine.edits.push(Edit::SetParamEnumOptions {
        node: parameter,
        expected_uuid: leaf.node_data().meta.uuid,
        expected_value: leaf
            .engine_param_snapshot()
            .ok_or_else(|| io::Error::other("GPU not a Parameter"))?
            .value,
        selected_value: Some(ParamValue::Enum(selected.clone())),
        options: vec![ParameterEnumOption {
            variant_id: selected.clone(),
            value: ParamValue::Enum(selected),
            label: "Previously selected GPU".into(),
            tags: vec![],
            ordering: None,
        }]
        .into(),
    });
    engine.apply_edits().map_err(io::Error::other)?;
    fs::write(
        file,
        to_sparse_preferences_json_pretty(&engine)
            .map_err(io::Error::other)?
            .ok_or_else(|| io::Error::other("Preferences not encoded"))?,
    )
}
