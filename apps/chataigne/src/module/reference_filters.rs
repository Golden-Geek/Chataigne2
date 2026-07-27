use golden_core::{
    engine::Engine,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::ProcessTreeSnapshot,
};

use super::{
    constants::{MODULE_VALUES_DECL_ID, MODULE_VALUES_REFERENCE_FILTER_KEY},
    MODULE_ITEM_KIND,
};

pub(crate) fn resolve_enclosing_module_root(snapshot: &ProcessTreeSnapshot, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);

    while let Some(node_id) = current {
        if snapshot.find_child(node_id, super::constants::MODULE_CONNECTION_DECL_ID).is_some()
            && snapshot
                .find_child(node_id, super::constants::MODULE_PARAMETERS_DECL_ID)
                .is_some()
            && snapshot.find_child(node_id, MODULE_VALUES_DECL_ID).is_some()
        {
            return Some(node_id);
        }

        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    None
}

pub(crate) fn register_module_reference_filters<T: Node>(engine: &mut Engine<T>) {
    engine.register_reference_filter(
        MODULE_VALUES_REFERENCE_FILTER_KEY,
        |engine, _param_node, _root, candidate| candidate_is_module_values_parameter(engine, candidate),
    );
    engine.register_reference_filter(
        crate::app::module_modules_audio_sound_card_schema::SOUND_CARD_INPUT_GAIN_FILTER_KEY,
        |engine, param, _root, candidate| {
            candidate_is_same_sound_card_gain(
                engine,
                param,
                candidate,
                crate::app::module_modules_audio_sound_card_schema::SoundCardInputChannelList::NODE_TYPE,
            )
        },
    );
    engine.register_reference_filter(
        crate::app::module_modules_audio_sound_card_schema::SOUND_CARD_OUTPUT_GAIN_FILTER_KEY,
        |engine, param, _root, candidate| {
            candidate_is_same_sound_card_gain(
                engine,
                param,
                candidate,
                crate::app::module_modules_audio_sound_card_schema::SoundCardOutputChannelList::NODE_TYPE,
            )
        },
    );
}

fn candidate_is_module_values_parameter<T: Node>(engine: &Engine<T>, candidate: NodeId) -> bool {
    let Some(candidate_node) = engine.nodes.get(candidate) else {
        return false;
    };
    if candidate_node.engine_param_snapshot().is_none() {
        return false;
    }

    let Some(mut current) = candidate_node.node_data().parent else {
        return false;
    };

    let mut has_values_ancestor = false;
    let mut has_module_ancestor = false;
    let mut has_module_manager_ancestor = false;
    loop {
        let Some(node) = engine.nodes.get(current) else {
            return false;
        };
        if node.node_data().meta.decl_id.0 == MODULE_VALUES_DECL_ID {
            has_values_ancestor = true;
        }
        if node.user_item_kind() == MODULE_ITEM_KIND {
            has_module_ancestor = true;
        }
        if node.get_type() == crate::app::ModuleManager::NODE_TYPE {
            has_module_manager_ancestor = true;
        }
        let Some(parent) = node.node_data().parent else {
            break;
        };
        current = parent;
    }

    has_values_ancestor && has_module_ancestor && has_module_manager_ancestor
}

fn candidate_is_same_sound_card_gain<T: Node>(
    engine: &Engine<T>,
    parameter: NodeId,
    candidate: NodeId,
    expected_container_type: &str,
) -> bool {
    let Some(candidate_node) = engine.nodes.get(candidate) else {
        return false;
    };
    if candidate_node.get_type() != "float" {
        return false;
    }
    let Some(parent) = candidate_node.node_data().parent else {
        return false;
    };
    if engine
        .nodes
        .get(parent)
        .is_none_or(|node| node.get_type() != expected_container_type)
    {
        return false;
    }

    let parameter_module = reference_owner_sound_card_module(engine, parameter);
    let candidate_module = enclosing_sound_card_module(engine, candidate);
    parameter_module.is_some() && parameter_module == candidate_module
}

fn reference_owner_sound_card_module<T: Node>(
    engine: &Engine<T>,
    start: NodeId,
) -> Option<NodeId> {
    let mut current = Some(start);
    let mut command = None;
    let mut enclosing_module = None;

    while let Some(node_id) = current {
        let node = engine.nodes.get(node_id)?;
        if node.get_type()
            == crate::app::module_modules_audio_sound_card::SoundCardModule::NODE_TYPE
        {
            enclosing_module = Some(node_id);
            break;
        }
        if node.user_item_kind()
            == crate::app::module_command::MODULE_COMMAND_ITEM_KIND
        {
            command = Some(node_id);
        }
        current = node.node_data().parent;
    }

    command
        .and_then(|command| linked_sound_card_module(engine, command))
        .or(enclosing_module)
}

fn linked_sound_card_module<T: Node>(
    engine: &Engine<T>,
    command: NodeId,
) -> Option<NodeId> {
    let target_param = find_descendant_by_decl_id(
        engine,
        command,
        crate::app::module_command::MODULE_COMMAND_TARGET_MODULE_PATH,
    )?;
    let parameter = engine.nodes.get(target_param)?.engine_param_snapshot()?;
    let ParamValue::Reference(reference) = parameter.value else {
        return None;
    };
    let target = reference
        .cached_id()
        .filter(|node_id| engine.nodes.contains(*node_id))
        .or_else(|| engine.node_id_by_uuid(reference.uuid()))?;
    engine.nodes.get(target).and_then(|node| {
        (node.get_type()
            == crate::app::module_modules_audio_sound_card::SoundCardModule::NODE_TYPE)
            .then_some(target)
    })
}

fn find_descendant_by_decl_id<T: Node>(
    engine: &Engine<T>,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = child {
        let child_node = engine.nodes.get(child_id)?;
        if child_node.node_data().meta.decl_id.0 == decl_id {
            return Some(child_id);
        }
        if let Some(found) =
            find_descendant_by_decl_id(engine, child_id, decl_id)
        {
            return Some(found);
        }
        child = child_node.node_data().next_sibling;
    }
    None
}

fn enclosing_sound_card_module<T: Node>(engine: &Engine<T>, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);
    while let Some(node_id) = current {
        let node = engine.nodes.get(node_id)?;
        if node.get_type()
            == crate::app::module_modules_audio_sound_card::SoundCardModule::NODE_TYPE
        {
            return Some(node_id);
        }
        current = node.node_data().parent;
    }
    None
}
