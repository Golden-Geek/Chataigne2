use super::*;
use chataigne_alchemist::authoring::*;
use golden_core::application::apply_project_intent;
use serde::{Serialize, de::DeserializeOwned};

fn operation<T: DeserializeOwned>(engine: &mut AppEngine, name: &str, payload: impl Serialize) -> T {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: FORMULA_OPERATION_NAMESPACE.into(),
            operation: name.into(),
            payload: serde_json::to_value(payload).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

#[test]
fn builtin_managed_graph_paste_preserves_compressed_workflow_and_external_references() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    let source_uuid = source_param(&mut engine, "Frozen source", 0.42);
    let sink_uuid = source_param(&mut engine, "Frozen destination", 0.0);
    let inputs = region(&engine, processor, "inputs");
    let input = create_item(
        &mut engine,
        inputs,
        &format!("{ANODE_CREATE_PREFIX}chataigne.input_source"),
    );
    set_config(
        &mut engine,
        input,
        "source",
        ParamValue::Reference(NodeReference::new(source_uuid)),
    );
    let outputs = region(&engine, processor, "outputs");
    create_parameter_output(&mut engine, outputs, sink_uuid);
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let before = engine.process_tree_snapshot();
    let frozen = processor_frozen_source(&before, processor).unwrap().unwrap();
    assert!(!frozen.commands.is_empty());
    let archived_roots = frozen
        .authored_regions
        .iter()
        .map(|region| region.document.root.uuid)
        .collect::<Vec<_>>();
    let source = before.node_id_by_uuid(mapping_uuid).unwrap();
    let selected = before
        .child_ids(source)
        .into_iter()
        .filter(|id| before.node(*id).unwrap().node_type == "alchemist_anode")
        .collect::<Vec<_>>();
    let managed_metadata =
        crate::app::systems_alchemist_formula::formula_managed_regions_from_snapshot(&before, source).unwrap();
    assert!(!managed_metadata.is_empty());
    let fragment: FormulaFragment = operation(
        &mut engine,
        FORMULA_COPY_OPERATION,
        FormulaCopyRequest {
            formula: FormulaOwnerDto {
                node: source,
                uuid: mapping_uuid,
            },
            nodes: selected.clone(),
        },
    );
    let mut records = fragment
        .items
        .iter()
        .map(|item| &item.document.root)
        .collect::<Vec<_>>();
    let mut source_ids = std::collections::HashSet::new();
    let mut references = Vec::new();
    while let Some(record) = records.pop() {
        source_ids.insert(record.uuid);
        match record
            .data
            .as_ref()
            .and_then(|data| data.get("value"))
            .and_then(|value| serde_json::from_value::<ParamValue>(value.clone()).ok())
        {
            Some(ParamValue::Reference(reference)) if !reference.is_empty() => {
                references.push((record.uuid, reference.uuid()))
            }
            _ => {}
        }
        records.extend(&record.children);
    }
    let external = references
        .into_iter()
        .filter(|(_, target)| !source_ids.contains(target))
        .collect::<Vec<_>>();
    assert!(
        !external.is_empty(),
        "managed graph fixture must exercise real external references"
    );
    let destination = AlchemistFormulaDefinition::new();
    let uuid = destination.node_data().meta.uuid;
    engine.add_node(destination.into(), None);
    engine.apply_edits().unwrap();
    let node = engine.node_id_by_uuid(uuid).unwrap();
    let destination = FormulaOwnerDto { node, uuid };
    let pasted: FormulaPasteResult = operation(
        &mut engine,
        FORMULA_PASTE_OPERATION,
        FormulaPasteRequest {
            formula: destination.clone(),
            fragment,
            offset: Some([10.0, 10.0]),
        },
    );
    assert_eq!(pasted.selected_nodes.len(), selected.len());
    let snapshot = engine.process_tree_snapshot();
    for identity in &pasted.identities {
        assert_eq!(snapshot.node(identity.node).unwrap().uuid, identity.uuid);
        assert_ne!(identity.source_uuid, identity.uuid);
    }
    for (source, target) in external {
        let imported = pasted
            .identities
            .iter()
            .find(|identity| identity.source_uuid == source)
            .unwrap();
        let ParamValue::Reference(reference) = snapshot.node(imported.node).unwrap().param_value.clone().unwrap()
        else {
            panic!("reference storage changed")
        };
        assert_eq!(reference.uuid(), target);
    }
    assert_eq!(processor_frozen_source(&snapshot, processor).unwrap().unwrap(), frozen);
    assert_eq!(
        crate::app::systems_alchemist_formula::formula_managed_regions_from_snapshot(&snapshot, source).unwrap(),
        managed_metadata
    );
    assert!(archived_roots.iter().all(|id| snapshot.node_id_by_uuid(*id).is_none()));
    assert!(apply_project_intent(&mut engine, UiEditIntent::Undo).success);
    assert!(pasted.identities.iter().all(|id| engine.nodes.get(id.node).is_none()));
    assert!(apply_project_intent(&mut engine, UiEditIntent::Redo).success);
    assert!(pasted.identities.iter().all(|id| {
        engine
            .nodes
            .get(id.node)
            .is_some_and(|node| node.node_data().meta.uuid == id.uuid)
    }));
    set_mapping_compression(&mut engine, processor, false).unwrap();
    let expanded = engine.process_tree_snapshot();
    assert!(archived_roots.iter().all(|id| expanded.node_id_by_uuid(*id).is_some()));
    assert!(expanded.node_id_by_uuid(source_uuid).is_some());
    assert!(expanded.node_id_by_uuid(sink_uuid).is_some());
}

#[test]
fn read_only_mapping_paste_rejects_atomically_with_a_real_compressed_consumer() {
    let (mut engine, mapping_uuid, processor) = mapping_engine();
    set_mapping_compression(&mut engine, processor, true).unwrap();
    let snapshot = engine.process_tree_snapshot();
    let formula = FormulaOwnerDto {
        node: snapshot.node_id_by_uuid(mapping_uuid).unwrap(),
        uuid: mapping_uuid,
    };
    let selected = snapshot
        .child_ids(formula.node)
        .into_iter()
        .filter(|id| snapshot.node(*id).unwrap().node_type == "alchemist_anode")
        .collect();
    let fragment: FormulaFragment = operation(
        &mut engine,
        FORMULA_COPY_OPERATION,
        FormulaCopyRequest {
            formula: formula.clone(),
            nodes: selected,
        },
    );
    let frozen = processor_frozen_source(&engine.process_tree_snapshot(), processor)
        .unwrap()
        .unwrap();
    let count = engine.nodes.len();
    let history = engine.undo_len();
    let events = engine.ui_event_log().len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: FORMULA_OPERATION_NAMESPACE.into(),
            operation: FORMULA_PASTE_OPERATION.into(),
            payload: serde_json::to_value(FormulaPasteRequest {
                formula,
                fragment,
                offset: None,
            })
            .unwrap(),
        },
    );
    assert!(!ack.success);
    assert!(ack.result.is_none());
    assert_eq!(engine.nodes.len(), count);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(engine.ui_event_log().len(), events);
    assert_eq!(
        processor_frozen_source(&engine.process_tree_snapshot(), processor)
            .unwrap()
            .unwrap(),
        frozen
    );
}
