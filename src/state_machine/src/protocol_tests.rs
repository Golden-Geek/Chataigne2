use golden_alchemist::{
    ANodeId, ANodeInstance, ANodeTypeId, ContextKey, ExecNodeId, FormulaId, FormulaSurface, ManagedItemId,
    ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance,
    ManagedRegionInstances, ManagedRegionKind, ManagedSocketRef, OutputPreviewStatus, RuntimeValue, SocketId,
    SurfaceItemKind, ValueTypeId,
};

use crate::{
    ANodeOutputPreviewSample, ProcessorFormulaUiState, ProcessorId, ProcessorUiModel,
    protocol::{
        ANodeOutputPreviewSampleDto, ContextKeyDto, ManagedRegionDefinitionDto, ManagedRegionInstanceDto,
        ManagedRegionKindDto, ProcessorFormulaSourceKindDto, ProcessorUiDto, RuntimeValueDto,
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
fn processor_ui_dto_preserves_builtin_formula_source_state() {
    let model = ProcessorUiModel {
        id: ProcessorId::new(),
        label: "Formula".into(),
        active: true,
        formula_id: "example.formula@1".into(),
        formula_label: "Formula".into(),
        formula_source_key: Some("state_processor:builtin:example.formula@1".into()),
        surface: FormulaSurface {
            sections: Vec::new(),
            managed_regions: Vec::new(),
        },
        managed_region_instances: ManagedRegionInstances::default(),
        diagnostics: Vec::new(),
        formula_source: ProcessorFormulaUiState::builtin(true, true),
    };

    let dto = ProcessorUiDto::from(&model);

    assert!(matches!(
        dto.formula_source_kind,
        ProcessorFormulaSourceKindDto::Builtin
    ));
    assert_eq!(
        dto.formula_source_key.as_deref(),
        Some("state_processor:builtin:example.formula@1")
    );
    assert!(dto.formula_open_readonly_from_processor);
    assert!(dto.formula_can_duplicate_to_library);
}

#[test]
fn managed_region_definition_dto_preserves_role_and_socket_contract() {
    let boundary = ANodeId::new();
    let definition = ManagedRegionDefinition {
        id: ManagedRegionId::new("filters"),
        kind: ManagedRegionKind::FilterPipeline,
        label: "Filters".into(),
        input_socket: Some(ManagedSocketRef::new(boundary, "value")),
        output_socket: Some(ManagedSocketRef::new(boundary, "result")),
        accepted_roles: vec![SurfaceItemKind::Filter, SurfaceItemKind::Condition],
    };

    let dto = ManagedRegionDefinitionDto::from(&definition);

    assert_eq!(dto.id, "filters");
    assert!(matches!(dto.kind, ManagedRegionKindDto::FilterPipeline));
    assert_eq!(dto.label, "Filters");
    assert_eq!(dto.input_socket.as_ref().unwrap().node_id, boundary.to_string());
    assert_eq!(dto.input_socket.as_ref().unwrap().socket_id, "value");
    assert_eq!(dto.output_socket.as_ref().unwrap().socket_id, "result");
    assert_eq!(dto.accepted_roles.len(), 2);
}

#[test]
fn managed_region_instance_dto_preserves_item_identity_and_ui_state() {
    let mut anode = ANodeInstance::new(ANodeTypeId::new("remap"), "Remap");
    anode.enabled = false;
    let anode_id = anode.id;
    let item_id = ManagedItemId::new();
    let instance = ManagedRegionInstance {
        region_id: ManagedRegionId::new("filters"),
        items: vec![ManagedItemInstance {
            id: item_id,
            anode,
            enabled: true,
            ui_state: ManagedItemUiState { collapsed: true },
        }],
    };

    let dto = ManagedRegionInstanceDto::from(&instance);

    assert_eq!(dto.region_id, "filters");
    assert_eq!(dto.items.len(), 1);
    assert_eq!(dto.items[0].id, item_id.to_string());
    assert_eq!(dto.items[0].anode_id, anode_id.to_string());
    assert_eq!(dto.items[0].anode_type_id, "remap");
    assert_eq!(dto.items[0].label, "Remap");
    assert!(dto.items[0].enabled);
    assert!(!dto.items[0].anode_enabled);
    assert!(dto.items[0].ui_state.collapsed);
}
