use super::*;
use golden_core::{node::Node, process_ctx::ExecutionPhase};

fn fixture() -> (crate::app::AppEngine, NodeId) {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let parameter = Parameter::new(
        "Port",
        ParamValue::Enum("first".into()),
        ParameterChangeCheck::ValueChange,
    );
    engine.add_node(parameter.into(), None);
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}
fn delivery(selection: &str) -> CatalogDelivery {
    let port = PortCatalog {
        selection: selection.into(),
        options: vec![ParameterEnumOption {
            variant_id: selection.into(),
            value: ParamValue::Enum(selection.into()),
            label: format!("Missing: {selection}"),
            tags: vec!["missing".into()],
            ordering: Some(5),
        }]
        .into(),
    };
    CatalogDelivery::new(CatalogEvent {
        generation: 0,
        result: Ok(midi_runtime::PreparedCatalog {
            input: port.clone(),
            output: port,
        }),
    })
}
fn service(engine: &mut crate::app::AppEngine, id: NodeId, delivery: &mut CatalogDelivery) -> ProcessCtx {
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut ctx, Some(&snapshot), Some(id), None));
    assert!(ctx.edits.pending.len() <= 3);
    ctx
}

#[test]
fn queued_choices_preserve_newer_selection_and_authored_metadata() {
    let (mut engine, id) = fixture();
    let mut delivery = delivery("first");
    let mut ctx = service(&mut engine, id, &mut delivery);
    engine.edits.push(Edit::SetParam {
        node: id,
        value: ParamValue::Enum("newer".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.edits.push(Edit::PatchMeta {
        node: id,
        patch: NodeMetaPatch {
            label: Some("Authored port".into()),
            ..Default::default()
        },
    });
    engine.absorb_edits(&mut ctx).unwrap();
    // The guarded enum operation must never replace a later value or metadata.
    assert!(engine.apply_edits().is_err());
    let node = engine.nodes.get(id).unwrap();
    assert_eq!(node.node_data().meta.label, "Authored port");
    assert_eq!(
        node.engine_param_snapshot().unwrap().value,
        ParamValue::Enum("newer".into())
    );
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(delivery.service(&mut ctx, Some(&snapshot), Some(id), None));
    assert!(ctx.edits.pending.is_empty());
}

#[test]
fn pending_receipts_do_not_repeat_publication_or_require_snapshots() {
    let (mut engine, id) = fixture();
    let mut delivery = delivery("first");
    let mut first = service(&mut engine, id, &mut delivery);
    assert!(!delivery.requires_snapshot());
    let mut waiting = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut waiting, None, Some(id), None));
    assert!(waiting.edits.pending.is_empty());
    engine.absorb_edits(&mut first).unwrap();
    engine.apply_edits().unwrap();
    assert!(delivery.service(&mut waiting, None, Some(id), None));
    let snapshot = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(snapshot.value, ParamValue::Enum("first".into()));
    assert!(snapshot.constraints.enum_options[0].tags.iter().any(|t| t == "missing"));
}

#[test]
fn dropped_requests_retry_the_same_canonical_options() {
    let (mut engine, id) = fixture();
    let mut delivery = delivery("first");
    let first = service(&mut engine, id, &mut delivery);
    drop(first);
    assert!(delivery.requires_snapshot());
    let mut retry = service(&mut engine, id, &mut delivery);
    engine.absorb_edits(&mut retry).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(delivery.service(&mut ctx, None, Some(id), None));
    assert!(ctx.edits.pending.is_empty());
}
