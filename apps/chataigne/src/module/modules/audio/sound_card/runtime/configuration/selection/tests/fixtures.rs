use super::*;
use golden_core::{
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
};

pub(super) fn source(driver: &str, input: &str, output: &str) -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("module").into());
    let module = engine.root;
    engine.add_node(Folder::new("connection").into(), None);
    engine.apply_edits().unwrap();
    let connection = find_path(&engine.process_tree_snapshot(), module, "connection").unwrap();
    for (key, value) in [
        ("audio_driver", driver),
        ("device", output),
        ("input_device", input),
        ("output_device", output),
    ] {
        let mut node = Parameter::new(key, ParamValue::Enum(value.into()), ParameterChangeCheck::ValueChange);
        node.node_data_mut().meta.decl_id = DeclId(key.into());
        engine.add_node(node.into(), Some(connection));
    }
    engine.apply_edits().unwrap();
    (engine, module)
}

pub(super) fn device() -> golden_audio::AudioDeviceDescriptor {
    use golden_audio::AudioBackend;
    golden_audio::NullBackend.discover().unwrap().remove(0)
}

pub(super) fn selected(device: &golden_audio::AudioDeviceDescriptor) -> String {
    serde_json::to_string(&golden_audio::AudioDeviceSelection::from_descriptor(device)).unwrap()
}
