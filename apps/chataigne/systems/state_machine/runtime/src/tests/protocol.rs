use chataigne_alchemist::{ANodeId, ContextKey, ExecNodeId, FormulaId, OutputPreviewStatus, SocketId, ValueTypeId};
use golden_values::Value as RuntimeValue;

use crate::{
    ANodeOutputPreviewSample, ProcessorFormulaUiState, ProcessorId, ProcessorUiModel,
    protocol::{
        ANodeOutputPreviewSampleDto, ContextKeyDto, ProcessorFormulaSourceKindDto, ProcessorUiDto, RuntimeValueDto,
    },
};

#[test]
fn context_key_dto_preserves_stable_axis_and_item_ids() {
    let key = ContextKey::single("device", "keyboard-1");

    let dto = ContextKeyDto::from(&key);

    assert_eq!(dto.parts.len(), 1);
    assert_eq!(dto.parts[0].axis_id, "device");
    assert_eq!(dto.parts[0].axis_label, "device");
    assert_eq!(dto.parts[0].item_id, "keyboard-1");
    assert_eq!(dto.parts[0].item_label, "keyboard-1");
    assert_eq!(dto.parts[0].index, None);
}

#[test]
fn runtime_value_dto_preserves_nested_values() {
    let value = RuntimeValue::Array(vec![
        RuntimeValue::Bool(true),
        RuntimeValue::Float(0.5),
        RuntimeValue::String("ready".into()),
    ]);

    let dto = RuntimeValueDto::from(&value);

    let RuntimeValueDto::Array { values } = dto else {
        panic!("expected array dto");
    };
    assert!(matches!(values[0], RuntimeValueDto::Bool { value: true }));
    assert!(matches!(values[1], RuntimeValueDto::Float { value } if value == 0.5));
    assert!(matches!(values[2], RuntimeValueDto::String { ref value } if value == "ready"));
}

#[test]
fn output_preview_sample_dto_keeps_formula_processor_lane_and_exec_identity() {
    let processor_id = ProcessorId::new();
    let formula_id = FormulaId::new("formula-a");
    let context_key = ContextKey::single("device", "deck-1");
    let author_node_id = ANodeId::new();
    let sample = ANodeOutputPreviewSample {
        formula_id: formula_id.clone(),
        processor_id: Some(processor_id),
        context_key: Some(context_key),
        author_node_id,
        exec_node: ExecNodeId::new(7),
        output_socket: SocketId::new("value"),
        value_type: ValueTypeId::new("float"),
        value: RuntimeValue::Float(0.75),
        logical_tick: 99,
        status: OutputPreviewStatus::Live,
    };

    let dto = ANodeOutputPreviewSampleDto::from(&sample);

    assert_eq!(dto.formula_id, formula_id.to_string());
    assert_eq!(dto.processor_id, Some(processor_id.to_string()));
    assert_eq!(dto.context_key.as_ref().unwrap().parts[0].item_id, "deck-1");
    assert_eq!(dto.node_id, author_node_id.to_string());
    assert_eq!(dto.exec_node_id, "7");
    assert_eq!(dto.output_socket_id, "value");
    assert_eq!(dto.value_type, "float");
    assert_eq!(dto.logical_tick, 99);
    assert!(matches!(dto.value, RuntimeValueDto::Float { value } if value == 0.75));
}

#[test]
fn processor_ui_dto_preserves_read_only_external_formula_source_state() {
    let model = ProcessorUiModel {
        id: ProcessorId::new(),
        label: "Formula".into(),
        active: true,
        formula_id: "11111111-2222-4333-8444-000000000001".into(),
        formula_label: "Formula".into(),
        formula_source_key: Some("state_processor:project:11111111-2222-4333-8444-000000000001".into()),
        formula_source: ProcessorFormulaUiState::builtin(true, false),
    };

    let dto = ProcessorUiDto::from(&model);

    assert!(matches!(
        dto.formula_source_kind,
        ProcessorFormulaSourceKindDto::Builtin
    ));
    assert_eq!(
        dto.formula_source_key.as_deref(),
        Some("state_processor:project:11111111-2222-4333-8444-000000000001")
    );
    assert!(dto.formula_open_readonly_from_processor);
    assert!(!dto.formula_can_duplicate_to_library);
    let json = serde_json::to_value(dto).expect("processor UI metadata should serialize");
    for removed_inventory_field in [
        "surface",
        "managed_regions",
        "managed_region_instances",
        "mapping_pipeline",
        "mapping_outputs",
        "mapping_diagnostics",
    ] {
        assert!(json.get(removed_inventory_field).is_none());
    }
}
