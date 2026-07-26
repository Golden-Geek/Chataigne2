use std::{collections::HashSet, time::Duration};

use golden_audio::{
    AudioBackend, AudioBackendState, AudioBackendStatus, AudioChannelId,
    AudioDeviceInspectorState, AudioDeviceTargetId, BackendId, NullBackend,
};
use golden_core::{
    app::ProjectNode,
    edit::{Edit, NodeTree},
    node::{Folder, Node, NodeId, NodeMetaPatch, NodeReference, NodeUuid},
    parameter::{ParamValue, ParameterEnumOption, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::{
    meter_uuid, runtime, SoundCardModule, ANALYSIS_PATH, INPUT_LEVELS_PATH,
    INPUT_PROFILES_PATH, OUTPUT_LEVELS_PATH, OUTPUT_PROFILES_PATH,
    PITCH_RESULTS_PATH, SPECTRUM_RESULTS_PATH, VIRTUAL_INPUTS_PATH,
    VIRTUAL_OUTPUTS_PATH,
};
use crate::app::{
    module_modules_audio_sound_card_commands::{
        SoundCardSetChannelVolumeCommand, SOUND_CARD_COMMAND_TYPES,
    },
    module_modules_audio_sound_card_schema::{
        SoundCardChannelMeter, SoundCardInputPatchRoute, SoundCardInputProfile,
        SoundCardMonitorRoute, SoundCardOutputPatchRoute, SoundCardOutputProfile,
        SoundCardPitchAnalyzer, SoundCardPlaybackRoute, SoundCardSpectrumAnalyzer,
        SoundCardSpectrumBand, SoundCardVirtualInput, SoundCardVirtualOutput,
        SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY, SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY,
    },
};

mod phase10;
mod phase11;

#[test]
fn generated_catalog_contains_sound_card_and_only_its_five_tester_commands() {
    let item = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
        .into_iter()
        .find(|item| item.node_type == SoundCardModule::NODE_TYPE)
        .expect("Sound Card should be generated into the module catalog");
    assert_eq!(item.label, "Sound Card");
    assert_eq!(item.menu_path, vec!["Audio".to_string()]);

    let (mut engine, module) = create_sound_card_module();
    let tester = find_path(&engine, module, "command_tester").expect("command tester");
    let tester_node = engine.nodes.get(tester).expect("command tester node");
    let actual = tester_node
        .user_creatable_items()
        .into_iter()
        .map(|item| item.node_type)
        .collect::<HashSet<_>>();
    let expected = SOUND_CARD_COMMAND_TYPES
        .iter()
        .map(|node_type| (*node_type).to_string())
        .collect::<HashSet<_>>();
    assert_eq!(actual, expected);
    let generated_commands = crate::app::declared_user_creatable_items(
        crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
    )
    .into_iter()
    .map(|item| item.node_type)
    .collect::<HashSet<_>>();
    assert!(
        expected.is_subset(&generated_commands),
        "every Sound Card command should come from the generated app-node catalog"
    );

    engine.add_node(
        SoundCardSetChannelVolumeCommand::create().into(),
        Some(tester),
    );
    stabilize(&mut engine);
    let command = direct_children_of_type(
        &engine,
        tester,
        SoundCardSetChannelVolumeCommand::NODE_TYPE,
    )[0];
    let target = find_path(&engine, command, "virtual_output")
        .expect("channel-volume target");
    assert_eq!(
        reference_filter_key(&engine, target),
        Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY)
    );
}

#[test]
fn fresh_module_materializes_authored_defaults_and_derived_values() {
    let (engine, module) = create_sound_card_module();

    assert_eq!(param_value(&engine, module, "connection/input_enabled"), Some(&ParamValue::Bool(false)));
    assert_eq!(param_value(&engine, module, "connection/output_enabled"), Some(&ParamValue::Bool(true)));
    assert_eq!(param_value(&engine, module, "connection/engine_sample_rate"), Some(&ParamValue::Int(48_000)));
    assert_eq!(param_value(&engine, module, "parameters/master_volume_db"), Some(&ParamValue::Float(0.0)));

    let inputs = typed_children(&engine, module, VIRTUAL_INPUTS_PATH, SoundCardVirtualInput::NODE_TYPE);
    let outputs = typed_children(&engine, module, VIRTUAL_OUTPUTS_PATH, SoundCardVirtualOutput::NODE_TYPE);
    assert_eq!(inputs.len(), 2);
    assert_eq!(outputs.len(), 2);
    let module_permissions = &engine
        .nodes
        .get(module)
        .expect("Sound Card module")
        .node_data()
        .meta
        .user_permissions;
    assert!(module_permissions.can_edit_name);
    assert!(module_permissions.can_remove_and_duplicate);
    let input_list = find_path(&engine, module, VIRTUAL_INPUTS_PATH).expect("input list");
    let list_permissions = &engine
        .nodes
        .get(input_list)
        .expect("input list")
        .node_data()
        .meta
        .user_permissions;
    assert!(!list_permissions.can_edit_name);
    assert!(!list_permissions.can_remove_and_duplicate);
    assert!(inputs.iter().all(|id| {
        engine
            .nodes
            .get(*id)
            .expect("virtual input")
            .node_data()
            .meta
            .user_permissions
            .can_remove_and_duplicate
    }));

    let input_profiles =
        typed_children(&engine, module, INPUT_PROFILES_PATH, SoundCardInputProfile::NODE_TYPE);
    let output_profiles =
        typed_children(&engine, module, OUTPUT_PROFILES_PATH, super::SoundCardOutputProfile::NODE_TYPE);
    assert_eq!(input_profiles.len(), 1);
    assert_eq!(output_profiles.len(), 1);
    assert_eq!(
        direct_children_of_type(&engine, input_profiles[0], SoundCardInputPatchRoute::NODE_TYPE).len(),
        2
    );
    assert_eq!(
        direct_children_of_type(
            &engine,
            output_profiles[0],
            super::SoundCardOutputPatchRoute::NODE_TYPE,
        )
        .len(),
        2
    );

    let monitoring = find_path(&engine, module, "parameters/monitoring_routes").expect("monitoring routes");
    assert!(direct_children_of_type(&engine, monitoring, SoundCardMonitorRoute::NODE_TYPE).is_empty());

    let analyzers = find_path(&engine, module, ANALYSIS_PATH).expect("analysis list");
    assert_eq!(
        direct_children_of_type(&engine, analyzers, SoundCardPitchAnalyzer::NODE_TYPE).len(),
        1
    );
    assert_eq!(
        direct_children_of_type(&engine, analyzers, SoundCardSpectrumAnalyzer::NODE_TYPE).len(),
        1
    );
    assert_eq!(
        typed_children(&engine, module, INPUT_LEVELS_PATH, SoundCardChannelMeter::NODE_TYPE).len(),
        2
    );
    assert_eq!(
        typed_children(&engine, module, OUTPUT_LEVELS_PATH, SoundCardChannelMeter::NODE_TYPE).len(),
        2
    );
    assert_eq!(child_count(&engine, module, PITCH_RESULTS_PATH), 1);
    let spectrum_result =
        first_child(&engine, find_path(&engine, module, SPECTRUM_RESULTS_PATH).expect("spectrum results"))
            .expect("spectrum result");
    assert_eq!(
        direct_children_of_type(&engine, spectrum_result, SoundCardSpectrumBand::NODE_TYPE).len(),
        64
    );
}

#[test]
fn sparse_round_trip_preserves_authored_identity_routes_and_missing_device() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/authored_project.json"
    ))
    .expect("authored Sound Card project fixture should parse");
    let (mut engine, module) = create_sound_card_module();
    let input_list = find_path(&engine, module, VIRTUAL_INPUTS_PATH).expect("input list");
    let output_list = find_path(&engine, module, VIRTUAL_OUTPUTS_PATH).expect("output list");
    let inputs = direct_children_of_type(&engine, input_list, SoundCardVirtualInput::NODE_TYPE);
    let outputs = direct_children_of_type(&engine, output_list, SoundCardVirtualOutput::NODE_TYPE);
    let input_uuids = node_uuids(&engine, inputs.as_slice());
    let output_uuids = node_uuids(&engine, outputs.as_slice());

    engine.edits.push(Edit::PatchMeta {
        node: inputs[0],
        patch: NodeMetaPatch {
            label: Some(fixture_str(&fixture, "input_label").to_string()),
            ..Default::default()
        },
    });
    let input_order = fixture["input_order"]
        .as_array()
        .expect("input_order should be an array")
        .iter()
        .map(|value| {
            value
                .as_u64()
                .expect("input_order entries should be non-negative integers")
                as usize
        })
        .collect::<Vec<_>>();
    assert_eq!(input_order.len(), inputs.len());
    engine.edits.push(Edit::MoveNode {
        node: inputs[input_order[0]],
        new_parent: input_list,
        new_prev_sibling: None,
    });
    let output_volume = find_path(&engine, outputs[0], "volume_db").expect("output volume");
    let output_volume_db = fixture["output_volume_db"]
        .as_f64()
        .expect("output_volume_db should be a number");
    set_param(
        &mut engine,
        output_volume,
        ParamValue::Float(output_volume_db),
    );

    let monitoring =
        find_path(&engine, module, "parameters/monitoring_routes").expect("monitoring routes");
    let monitor_input = fixture_index(&fixture, "monitor_route", "input");
    let monitor_output = fixture_index(&fixture, "monitor_route", "output");
    let input_ref = node_reference(&engine, inputs[monitor_input]);
    let output_ref = node_reference(&engine, outputs[monitor_output]);
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(SoundCardMonitorRoute::with_targets(input_ref, output_ref)),
        parent: monitoring,
        prev_sibling: None,
    });
    let playback =
        find_path(&engine, module, "parameters/playback_routes").expect("playback routes");
    let playback_source =
        fixture_index(&fixture, "playback_route", "source_channel") as i32;
    let playback_output = fixture_index(&fixture, "playback_route", "output");
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(SoundCardPlaybackRoute::with_target(
            playback_source,
            node_reference(&engine, outputs[playback_output]),
        )),
        parent: playback,
        prev_sibling: None,
    });
    engine.apply_edits().expect("authored edits should apply");
    stabilize(&mut engine);

    let input_device =
        find_path(&engine, module, "connection/input_device").expect("input device");
    let missing_target =
        fixture_str(&fixture["missing_input_device"], "id").to_string();
    let missing_label =
        fixture_str(&fixture["missing_input_device"], "label").to_string();
    let crate::app::AppNode::Parameter(parameter) = engine
        .nodes
        .get_mut(input_device)
        .expect("input device parameter")
    else {
        panic!("input device should be a parameter");
    };
    parameter.value = ParamValue::Enum(missing_target.clone());
    parameter.constraints.enum_options.push(ParameterEnumOption {
        variant_id: missing_target.clone(),
        value: ParamValue::Enum(missing_target.clone()),
        label: missing_label,
        tags: vec!["missing".to_string()],
        ordering: Some(1),
    });

    let json =
        golden_core::app::to_sparse_project_json_pretty(&engine).expect("Sound Card project should encode");
    assert!(json.contains("sound_card_monitor_route"));
    assert!(json.contains(missing_target.as_str()));

    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("Sound Card project should decode");
    crate::app::module::register_module_reference_filters(&mut loaded);
    golden_core::app::prepare_engine_for_runtime(&mut loaded)
        .expect("loaded Sound Card project should prepare");
    stabilize(&mut loaded);
    let loaded_module = first_child(&loaded, loaded.root).expect("loaded module");
    let loaded_inputs =
        typed_children(&loaded, loaded_module, VIRTUAL_INPUTS_PATH, SoundCardVirtualInput::NODE_TYPE);
    let loaded_outputs =
        typed_children(&loaded, loaded_module, VIRTUAL_OUTPUTS_PATH, SoundCardVirtualOutput::NODE_TYPE);
    let expected_input_uuids = input_order
        .iter()
        .map(|index| input_uuids[*index])
        .collect::<Vec<_>>();
    assert_eq!(
        node_uuids(&loaded, loaded_inputs.as_slice()),
        expected_input_uuids
    );
    assert_eq!(node_uuids(&loaded, loaded_outputs.as_slice()), output_uuids);
    let renamed_input_index = input_order
        .iter()
        .position(|index| *index == 0)
        .expect("fixture order should retain the renamed input");
    assert_eq!(
        loaded
            .nodes
            .get(loaded_inputs[renamed_input_index])
            .expect("renamed input")
            .node_data()
            .meta
            .label,
        fixture_str(&fixture, "input_label")
    );
    assert_eq!(
        param_value(&loaded, loaded_outputs[0], "volume_db"),
        Some(&ParamValue::Float(output_volume_db))
    );
    let loaded_monitoring =
        find_path(&loaded, loaded_module, "parameters/monitoring_routes").expect("monitoring routes");
    assert_eq!(
        direct_children_of_type(&loaded, loaded_monitoring, SoundCardMonitorRoute::NODE_TYPE).len(),
        1
    );
    let loaded_playback =
        find_path(&loaded, loaded_module, "parameters/playback_routes").expect("playback routes");
    assert_eq!(
        direct_children_of_type(&loaded, loaded_playback, SoundCardPlaybackRoute::NODE_TYPE).len(),
        1
    );

    let loaded_device =
        find_path(&loaded, loaded_module, "connection/input_device").expect("loaded input device");
    let crate::app::AppNode::Parameter(parameter) =
        loaded.nodes.get(loaded_device).expect("loaded device parameter")
    else {
        panic!("loaded device should be a parameter");
    };
    assert_eq!(parameter.value, ParamValue::Enum(missing_target.clone()));
    assert!(
        parameter.constraints.enum_options.iter().any(|option| {
            option.variant_id == missing_target
                && option.tags.iter().any(|tag| tag == "missing")
        }),
        "missing choice should remain tagged: {:?}",
        parameter.constraints.enum_options
    );
}

#[test]
fn route_reference_filters_reject_channels_from_another_sound_card() {
    let (mut engine, module) = create_sound_card_module();
    engine.add_node(SoundCardModule::create().into(), None);
    stabilize(&mut engine);
    let modules = direct_children_of_type(
        &engine,
        engine.root,
        SoundCardModule::NODE_TYPE,
    );
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0], module);

    let local_inputs = typed_children(
        &engine,
        modules[0],
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    );
    let local_outputs = typed_children(
        &engine,
        modules[0],
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );
    let foreign_inputs = typed_children(
        &engine,
        modules[1],
        VIRTUAL_INPUTS_PATH,
        SoundCardVirtualInput::NODE_TYPE,
    );
    let foreign_outputs = typed_children(
        &engine,
        modules[1],
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );

    let monitoring = find_path(&engine, modules[0], "parameters/monitoring_routes")
        .expect("monitoring routes");
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(SoundCardMonitorRoute::with_targets(
            node_reference(&engine, local_inputs[0]),
            node_reference(&engine, local_outputs[0]),
        )),
        parent: monitoring,
        prev_sibling: None,
    });
    engine.apply_edits().expect("local route should apply");
    stabilize(&mut engine);
    let route = direct_children_of_type(
        &engine,
        monitoring,
        SoundCardMonitorRoute::NODE_TYPE,
    )[0];
    let input_param = find_path(&engine, route, "virtual_input")
        .expect("monitor input parameter");
    let output_param = find_path(&engine, route, "virtual_output")
        .expect("monitor output parameter");
    assert_eq!(
        reference_filter_key(&engine, input_param),
        Some(SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY)
    );
    assert_eq!(
        reference_filter_key(&engine, output_param),
        Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY)
    );

    let foreign_input = node_reference(&engine, foreign_inputs[0]);
    set_param(
        &mut engine,
        input_param,
        ParamValue::Reference(foreign_input),
    );
    assert!(
        engine.apply_edits().is_err(),
        "foreign input should fail the same-module filter"
    );
    let foreign_output = node_reference(&engine, foreign_outputs[0]);
    set_param(
        &mut engine,
        output_param,
        ParamValue::Reference(foreign_output),
    );
    assert!(
        engine.apply_edits().is_err(),
        "foreign output should fail the same-module filter"
    );
}

#[test]
fn external_channel_volume_filter_follows_its_linked_sound_card() {
    let (mut engine, first_module) = create_sound_card_module();
    engine.add_node(SoundCardModule::create().into(), None);
    engine.add_node(
        SoundCardSetChannelVolumeCommand::create().into(),
        None,
    );
    stabilize(&mut engine);
    let modules = direct_children_of_type(
        &engine,
        engine.root,
        SoundCardModule::NODE_TYPE,
    );
    assert_eq!(modules.len(), 2);
    assert_eq!(modules[0], first_module);
    let command = direct_children_of_type(
        &engine,
        engine.root,
        SoundCardSetChannelVolumeCommand::NODE_TYPE,
    )[0];
    let target_module =
        find_path(&engine, command, "target_module").expect("target module");
    let virtual_output =
        find_path(&engine, command, "virtual_output").expect("virtual output");
    let first_outputs = typed_children(
        &engine,
        modules[0],
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );
    let second_outputs = typed_children(
        &engine,
        modules[1],
        VIRTUAL_OUTPUTS_PATH,
        SoundCardVirtualOutput::NODE_TYPE,
    );

    let first_module_reference = node_reference(&engine, modules[0]);
    set_param(
        &mut engine,
        target_module,
        ParamValue::Reference(first_module_reference),
    );
    engine
        .apply_edits()
        .expect("external command should accept its target module");
    let local_output = node_reference(&engine, first_outputs[0]);
    set_param(
        &mut engine,
        virtual_output,
        ParamValue::Reference(local_output),
    );
    engine
        .apply_edits()
        .expect("external command should accept a channel from its target module");

    let foreign_output = node_reference(&engine, second_outputs[0]);
    set_param(
        &mut engine,
        virtual_output,
        ParamValue::Reference(foreign_output),
    );
    assert!(
        engine.apply_edits().is_err(),
        "external command should reject a channel from another module"
    );
}

#[test]
fn meter_repair_is_idempotent_and_duplicate_channels_get_fresh_ids() {
    let (mut engine, module) = create_sound_card_module();
    let input = typed_children(&engine, module, VIRTUAL_INPUTS_PATH, SoundCardVirtualInput::NODE_TYPE)[0];
    let expected_meter_uuid = meter_uuid(
        engine.nodes.get(input).expect("input").node_data().meta.uuid,
    );
    let meter = typed_children(&engine, module, INPUT_LEVELS_PATH, SoundCardChannelMeter::NODE_TYPE)[0];
    engine.edits.push(Edit::RemoveNode { node: meter });
    engine.apply_edits().expect("meter removal should apply");
    stabilize(&mut engine);
    let meters =
        typed_children(&engine, module, INPUT_LEVELS_PATH, SoundCardChannelMeter::NODE_TYPE);
    assert_eq!(meters.len(), 2);
    assert!(node_uuids(&engine, meters.as_slice()).contains(&expected_meter_uuid));

    let original_inputs = node_uuids(
        &engine,
        typed_children(&engine, module, VIRTUAL_INPUTS_PATH, SoundCardVirtualInput::NODE_TYPE)
            .as_slice(),
    )
    .into_iter()
    .collect::<HashSet<_>>();
    let duplicate = engine
        .duplicate_subtree_with(
            module,
            engine.root,
            Some(module),
            None,
            |node| node.project_encode_data(),
            crate::app::AppNode::project_decode_node,
        )
        .expect("Sound Card should duplicate");
    stabilize(&mut engine);
    let duplicate_inputs = node_uuids(
        &engine,
        typed_children(
            &engine,
            duplicate,
            VIRTUAL_INPUTS_PATH,
            SoundCardVirtualInput::NODE_TYPE,
        )
        .as_slice(),
    )
    .into_iter()
    .collect::<HashSet<_>>();
    assert_eq!(duplicate_inputs.len(), 2);
    assert!(original_inputs.is_disjoint(&duplicate_inputs));

    engine.edits.push(Edit::RemoveNode { node: duplicate });
    engine.apply_edits().expect("duplicate removal should apply");
    assert!(!engine.nodes.contains(duplicate));
}

fn create_sound_card_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    crate::app::module::register_module_reference_filters(&mut engine);
    engine.add_node(SoundCardModule::create().into(), None);
    stabilize(&mut engine);
    let module = first_child(&engine, engine.root).expect("Sound Card module");
    (engine, module)
}

fn stabilize(engine: &mut crate::app::AppEngine) {
    for _ in 0..24 {
        engine.apply_edits().expect("edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("stabilization inbox should dispatch");
    }
    engine.resolve().expect("schedule should resolve");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("Sound Card update should run");
    for _ in 0..4 {
        engine.apply_edits().expect("update edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("update stabilization inbox should dispatch");
    }
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn find_path(
    engine: &crate::app::AppEngine,
    start: NodeId,
    path: &str,
) -> Option<NodeId> {
    path.split('/').try_fold(start, |parent, segment| {
        engine.nodes.get(parent).and_then(|parent_node| {
            let mut child = parent_node.node_data().first_child;
            while let Some(id) = child {
                let node = engine.nodes.get(id)?;
                let meta = &node.node_data().meta;
                if meta.decl_id.0 == segment
                    || meta.decl_id.0.rsplit('/').next() == Some(segment)
                    || meta.short_name == segment
                    || meta.label == segment
                {
                    return Some(id);
                }
                child = node.node_data().next_sibling;
            }
            None
        })
    })
}

fn typed_children(
    engine: &crate::app::AppEngine,
    module: NodeId,
    path: &str,
    node_type: &str,
) -> Vec<NodeId> {
    find_path(engine, module, path)
        .map(|parent| direct_children_of_type(engine, parent, node_type))
        .unwrap_or_default()
}

fn direct_children_of_type(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    node_type: &str,
) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let node = engine.nodes.get(id).expect("child node");
        if node.get_type() == node_type {
            result.push(id);
        }
        child = node.node_data().next_sibling;
    }
    result
}

fn first_child(engine: &crate::app::AppEngine, parent: NodeId) -> Option<NodeId> {
    engine.nodes.get(parent)?.node_data().first_child
}

fn child_count(engine: &crate::app::AppEngine, module: NodeId, path: &str) -> usize {
    let Some(parent) = find_path(engine, module, path) else {
        return 0;
    };
    let mut count = 0;
    let mut child = first_child(engine, parent);
    while let Some(id) = child {
        count += 1;
        child = engine.nodes.get(id).and_then(|node| node.node_data().next_sibling);
    }
    count
}

fn param_value<'a>(
    engine: &'a crate::app::AppEngine,
    start: NodeId,
    path: &str,
) -> Option<&'a ParamValue> {
    find_path(engine, start, path).and_then(|id| engine.nodes.get(id)?.engine_param_snapshot().map(|_| id)).and_then(
        |id| match engine.nodes.get(id)? {
            crate::app::AppNode::Parameter(parameter) => Some(&parameter.value),
            _ => None,
        },
    )
}

fn node_reference(engine: &crate::app::AppEngine, node: NodeId) -> NodeReference {
    let data = engine.nodes.get(node).expect("reference target").node_data();
    let mut reference = NodeReference::with_cached_id(data.meta.uuid, Some(node));
    reference.cached_name = Some(data.meta.label.clone());
    reference
}

fn node_uuids(engine: &crate::app::AppEngine, nodes: &[NodeId]) -> Vec<NodeUuid> {
    nodes
        .iter()
        .map(|id| engine.nodes.get(*id).expect("node").node_data().meta.uuid)
        .collect()
}

fn reference_filter_key(
    engine: &crate::app::AppEngine,
    node: NodeId,
) -> Option<&str> {
    let crate::app::AppNode::Parameter(parameter) =
        engine.nodes.get(node).expect("reference parameter")
    else {
        panic!("node should be a reference parameter");
    };
    parameter.constraints.reference.custom_filter_key.as_deref()
}

fn fixture_str<'a>(fixture: &'a serde_json::Value, key: &str) -> &'a str {
    fixture[key].as_str().expect("fixture value should be a string")
}

fn fixture_index(
    fixture: &serde_json::Value,
    section: &str,
    key: &str,
) -> usize {
    fixture[section][key]
        .as_u64()
        .expect("fixture index should be a non-negative integer") as usize
}
