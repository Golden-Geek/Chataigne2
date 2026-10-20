use super::*;
use crate::{
    node::NodeCreationContext,
    parameter::{Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEnumOptions},
    process_ctx::ExecutionPhase,
};

#[crate::node("native_parameter_wrapper")]
pub(super) struct NativeParameterWrapper {
    base: Parameter,
    #[state(default = 0, persist)]
    own: i32,
    runtime_calls: usize,
}

#[crate::node("native_parameter_wrapper", via = base, from_struct)]
impl Node for NativeParameterWrapper {
    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.runtime_calls += 1;
    }
}

#[crate::node("native_encoder_only")]
pub(super) struct NativeEncoderOnly {
    base: OpaqueAuthoredProbe,
    #[state(default = 0, persist)]
    own: i32,
    runtime_calls: usize,
}

#[crate::node("native_encoder_only", via = base, from_struct)]
impl Node for NativeEncoderOnly {
    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::json!({"__golden_via": {"count": self.base.count}, "__golden_state": {"own": self.own}}))
    }
}

#[crate::node("native_decoder_only")]
pub(super) struct NativeDecoderOnly {
    base: OpaqueAuthoredProbe,
    #[state(default = 0, persist)]
    own: i32,
    runtime_calls: usize,
}

#[crate::node("native_decoder_only", via = base, from_struct)]
impl Node for NativeDecoderOnly {
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.base
            .project_decode_data(data.get("__golden_via").unwrap_or(&serde_json::Value::Null))?;
        Self::__golden_node_project_decode_data(self, data.get("__golden_state").unwrap_or(&serde_json::Value::Null))?;
        // An actual project load deliberately resets a runtime service marker.
        self.runtime_calls = 999;
        Ok(())
    }
}

fn options(ids: &[&str]) -> ParameterEnumOptions {
    ids.iter()
        .map(|id| ParameterEnumOption {
            variant_id: (*id).into(),
            value: ParamValue::Enum((*id).into()),
            label: (*id).into(),
            tags: vec![],
            ordering: None,
        })
        .collect()
}

fn wrapper(domain: ParameterEnumOptions) -> NativeParameterWrapper {
    let mut base = Parameter::new(
        "Device",
        ParamValue::Enum("first".into()),
        ParameterChangeCheck::ValueChange,
    );
    base.constraints.enum_options = domain;
    NativeParameterWrapper::new(base, 0)
}

#[test]
fn composed_runtime_callbacks_preserve_native_base_storage_own_fields_and_bindings() {
    let first = options(&["first"]);
    let mut engine = Engine::new(OpaqueTestNode::from(wrapper(first.clone())));
    let root = engine.root;
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let uuid = engine.nodes.get(root).unwrap().node_data().meta.uuid;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    engine
        .run_runtime_callback(root, &mut ctx, |node, _| {
            let wrapper = node.as_any_mut().downcast_mut::<NativeParameterWrapper>().unwrap();
            wrapper.base.value = ParamValue::Enum("second".into());
            wrapper.base.constraints.enum_options = options(&["second"]);
            wrapper.own = 99;
            wrapper.runtime_calls += 1;
            wrapper.node_data_mut().meta.label = "discarded label".into();
            wrapper.node_data_mut().meta.uuid = crate::node::NodeUuid(uuid::Uuid::new_v4());
            wrapper.node_data_mut().parent = Some(NodeId(999));
        })
        .unwrap();
    let wrapper = engine
        .nodes
        .get(root)
        .unwrap()
        .as_any()
        .downcast_ref::<NativeParameterWrapper>()
        .unwrap();
    assert!(wrapper.base.constraints.enum_options.same_storage(&first));
    assert_eq!(wrapper.own, 0);
    assert_eq!(wrapper.runtime_calls, 1);
    assert_eq!(wrapper.node_data().meta.uuid, uuid);
    assert!(wrapper.node_data().parent.is_none());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
}

#[test]
fn composed_authored_edits_undo_redo_without_rebuilding_native_base_domains() {
    let first = options(&["first"]);
    let second = options(&["first", "second"]);
    let next = second.clone();
    let mut engine = Engine::new(OpaqueTestNode::from(wrapper(first.clone())));
    let root = engine.root;
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.edits.push(Edit::CallNodeMutation {
        node: root,
        needs_tree_snapshot: false,
        callback: Box::new(move |node, _| {
            let wrapper = node.as_any_mut().downcast_mut::<NativeParameterWrapper>().unwrap();
            wrapper.base.constraints.enum_options = next;
            wrapper.base.read_only = true;
            wrapper.own = 7;
            wrapper.runtime_calls += 1;
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    let wrapper = engine
        .nodes
        .get(root)
        .unwrap()
        .as_any()
        .downcast_ref::<NativeParameterWrapper>()
        .unwrap();
    assert!(wrapper.base.constraints.enum_options.same_storage(&first));
    assert_eq!(wrapper.own, 0);
    assert_eq!(wrapper.runtime_calls, 1);
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    let wrapper = engine
        .nodes
        .get(root)
        .unwrap()
        .as_any()
        .downcast_ref::<NativeParameterWrapper>()
        .unwrap();
    assert!(wrapper.base.constraints.enum_options.same_storage(&second));
    assert_eq!(wrapper.own, 7);
    assert!(wrapper.base.read_only);
    assert_eq!(wrapper.runtime_calls, 1);
}

#[test]
fn custom_project_encoder_uses_its_project_schema_for_authored_restore() {
    let mut node = NativeEncoderOnly::new(OpaqueAuthoredProbe::new(0), 0);
    let captured = node.engine_capture_authored_data().unwrap();
    assert!(captured.as_json().is_ok());
    node.base.count = 9;
    node.own = 7;
    node.runtime_calls = 1;
    node.engine_restore_authored_data(captured).unwrap();
    assert_eq!(node.base.count, 0);
    assert_eq!(node.own, 0);
    assert_eq!(node.runtime_calls, 1);
}

#[test]
fn custom_project_decoder_is_not_used_to_restore_complete_declared_history_fields() {
    let mut node = NativeDecoderOnly::new(OpaqueAuthoredProbe::new(0), 0);
    let captured = node.engine_capture_authored_data().unwrap();
    assert!(captured.inherited_parts().is_ok());
    node.base.count = 9;
    node.own = 7;
    node.runtime_calls = 1;
    node.engine_restore_authored_data(captured).unwrap();
    assert_eq!(node.base.count, 0);
    assert_eq!(node.own, 0);
    assert_eq!(
        node.runtime_calls, 1,
        "history must not perform project-load runtime resets"
    );
    node.project_decode_data(&serde_json::json!({"__golden_via": {"count": 3}, "__golden_state": {"own": 4}}))
        .unwrap();
    assert_eq!((node.base.count, node.own, node.runtime_calls), (3, 4, 999));
}

#[test]
fn opaque_parameter_reference_hint_edits_are_undoable_and_runtime_ids_do_not_add_history() {
    // A missing persistent target keeps its recovery hint; an empty reference
    // deliberately clears hints during the engine's normal cache refresh.
    let missing = crate::node::NodeUuid(uuid::Uuid::new_v4());
    let mut engine = Engine::new(Parameter::new(
        "Reference",
        ParamValue::Reference(crate::node::NodeReference::new(missing)),
        ParameterChangeCheck::ValueChange,
    ));
    let root = engine.root;
    engine.edits.push(Edit::CallNodeMutation {
        node: root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            let parameter = node.as_any_mut().downcast_mut::<Parameter>().unwrap();
            let ParamValue::Reference(reference) = &mut parameter.value else {
                unreachable!()
            };
            reference.cached_id = Some(NodeId(42));
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.undo_len(), 0);
    engine.edits.push(Edit::CallNodeMutation {
        node: root,
        needs_tree_snapshot: false,
        callback: Box::new(|node, _| {
            let parameter = node.as_any_mut().downcast_mut::<Parameter>().unwrap();
            let ParamValue::Reference(reference) = &mut parameter.value else {
                unreachable!()
            };
            reference.cached_name = Some("persisted display".into());
            Ok(())
        }),
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    let ParamValue::Reference(reference) = engine.nodes.get(root).unwrap().get() else {
        unreachable!()
    };
    assert!(reference.cached_name.is_none());
    assert!(engine.redo().unwrap());
    let ParamValue::Reference(reference) = engine.nodes.get(root).unwrap().get() else {
        unreachable!()
    };
    assert_eq!(reference.cached_name.as_deref(), Some("persisted display"));
}
