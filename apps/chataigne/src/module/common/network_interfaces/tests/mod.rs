use super::*;
use golden_core::{
    edit::Edit,
    node::{Folder, NodeMetaPatch},
    parameter::ParameterChangeCheck,
    process_ctx::ExecutionPhase,
};

fn option(variant: &str) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant.into(),
        value: ParamValue::Enum(variant.into()),
        label: variant.into(),
        tags: Vec::new(),
        ordering: None,
    }
}

fn fixture(selected: &str) -> (crate::app::AppEngine, golden_core::node::NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut parameter = Parameter::new(
        "Network Interface",
        ParamValue::Enum(selected.into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = vec![option("any"), option("10.0.0.8")].into();
    engine.add_node(parameter.into(), None);
    engine.apply_edits().unwrap();
    let param = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, param)
}

#[test]
fn a_missing_interface_does_not_retarget_the_configured_endpoint() {
    let (mut engine, param) = fixture("10.0.0.8");
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    sync_interface_enum_options(&mut ctx, param, vec![option("any")]);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let parameter = engine.nodes.get(param).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(parameter.value, ParamValue::Enum("10.0.0.8".into()));
}

#[test]
fn queued_network_catalog_cannot_overwrite_a_later_selection_or_label() {
    let (mut engine, param) = fixture("any");
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    sync_interface_enum_options(&mut ctx, param, vec![option("any")]);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.edits.push(Edit::SetParam {
        node: param,
        value: ParamValue::Enum("10.0.0.8".into()),
        behaviour: Default::default(),
    });
    engine.edits.push(Edit::PatchMeta {
        node: param,
        patch: NodeMetaPatch {
            label: Some("User label".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    let parameter = engine.nodes.get(param).unwrap();
    assert_eq!(
        parameter.engine_param_snapshot().unwrap().value,
        ParamValue::Enum("10.0.0.8".into())
    );
    assert_eq!(parameter.node_data().meta.label, "User label");
}
