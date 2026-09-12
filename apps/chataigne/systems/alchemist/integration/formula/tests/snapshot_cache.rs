use std::sync::Arc;

use golden_core::{
    engine::EngineTime,
    events::{Event, EventFrame, EventKind},
};

use super::super::{ANodeMaterializationCache, formula_from_snapshot_cached};
use super::*;

fn frame(kind: EventKind) -> EventFrame {
    EventFrame::from_shared(vec![Arc::new(Event {
        time: EngineTime { tick: 1, micro: 0, seq: 0 },
        kind,
    })])
}

#[test]
fn cached_anodes_rebuild_only_dirty_roots_and_retain_layout_invalidation() {
    let (mut engine, formula) = engine_with_formula();
    let changed = create_anode(&mut engine, formula, "constant", 1.0, 2.0);
    let retained = create_anode(&mut engine, formula, "constant", 3.0, 4.0);
    let mut cache = ANodeMaterializationCache::default();

    let snapshot = engine.process_tree_snapshot();
    let initial = formula_from_snapshot_cached(&snapshot, formula, &mut cache)
        .expect("initial Formula should materialize");
    assert_eq!(initial, formula_from_snapshot(&snapshot, formula).unwrap());
    let prior_changed = Arc::clone(cache.cached_instance(changed).unwrap());
    let prior_retained = Arc::clone(cache.cached_instance(retained).unwrap());

    let config = find_child_by_decl(&engine, changed, "config").unwrap();
    let value_type = find_child_by_decl(&engine, config, "config/value__type").unwrap();
    set_constant_value_type(&mut engine, changed, "vec3");
    let snapshot = engine.process_tree_snapshot();
    cache.observe_events(
        &snapshot,
        formula,
        &frame(EventKind::ParamChanged {
            param: value_type,
            old_value: ParamValue::Enum("float".into()),
            new_value: ParamValue::Enum("vec3".into()),
        }),
    );
    let materialized = formula_from_snapshot_cached(&snapshot, formula, &mut cache)
        .expect("changed Formula should materialize");
    assert_eq!(materialized, formula_from_snapshot(&snapshot, formula).unwrap());
    assert!(!Arc::ptr_eq(&prior_changed, cache.cached_instance(changed).unwrap()));
    assert!(Arc::ptr_eq(&prior_retained, cache.cached_instance(retained).unwrap()));

    let prior_retained = Arc::clone(cache.cached_instance(retained).unwrap());
    let position = find_child_by_decl(&engine, retained, "position").unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: position,
        value: ParamValue::Vec2(8.0, 9.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "layout edit should succeed: {ack:?}");
    let snapshot = engine.process_tree_snapshot();
    cache.observe_events(
        &snapshot,
        formula,
        &frame(EventKind::ParamChanged {
            param: position,
            old_value: ParamValue::Vec2(3.0, 4.0),
            new_value: ParamValue::Vec2(8.0, 9.0),
        }),
    );

    engine.edits.push(Edit::RemoveNode { node: changed });
    engine.apply_edits().expect("ANode removal should succeed");
    let snapshot = engine.process_tree_snapshot();
    cache.observe_events(
        &snapshot,
        formula,
        &frame(EventKind::ChildRemoved { parent: formula, child: changed }),
    );
    let materialized = formula_from_snapshot_cached(&snapshot, formula, &mut cache)
        .expect("removed Formula should materialize");
    assert_eq!(materialized, formula_from_snapshot(&snapshot, formula).unwrap());
    assert!(cache.cached_instance(changed).is_none());
    assert!(!Arc::ptr_eq(&prior_retained, cache.cached_instance(retained).unwrap()));
    assert_eq!(cache.cached_instance(retained).unwrap().ui.position, [8.0, 9.0]);
}

#[test]
fn connection_changes_reuse_anodes_but_unknown_deletions_force_refresh() {
    let (mut engine, formula) = engine_with_formula();
    let source = create_anode(&mut engine, formula, "constant", 1.0, 2.0);
    let target = create_anode(&mut engine, formula, "math", 3.0, 4.0);
    let mut cache = ANodeMaterializationCache::default();
    let snapshot = engine.process_tree_snapshot();
    formula_from_snapshot_cached(&snapshot, formula, &mut cache).unwrap();
    let prior_source = Arc::clone(cache.cached_instance(source).unwrap());
    let prior_target = Arc::clone(cache.cached_instance(target).unwrap());

    let before = direct_children(&engine, formula);
    create_connection(&mut engine, formula, source, "value", target, "value1");
    let connection = direct_children(&engine, formula)
        .into_iter()
        .find(|node| !before.contains(node) && engine.nodes.get(*node).unwrap().get_type() == AlchemistConnection::NODE_TYPE)
        .expect("connection should exist");
    let snapshot = engine.process_tree_snapshot();
    cache.observe_events(
        &snapshot,
        formula,
        &frame(EventKind::ChildAdded {
            parent: formula,
            child: connection,
            decl_id: DeclId("connection".into()),
        }),
    );
    let materialized = formula_from_snapshot_cached(&snapshot, formula, &mut cache).unwrap();
    assert_eq!(materialized, formula_from_snapshot(&snapshot, formula).unwrap());
    assert!(Arc::ptr_eq(&prior_source, cache.cached_instance(source).unwrap()));
    assert!(Arc::ptr_eq(&prior_target, cache.cached_instance(target).unwrap()));

    let source_param = find_child_by_decl(&engine, connection, "source_node").unwrap();
    let formula_uuid = engine.nodes.get(formula).unwrap().node_data().meta.uuid;
    let broken = snapshot.with_param_values([(
        source_param,
        ParamValue::Reference(NodeReference::new(formula_uuid)),
    )]);
    assert!(formula_from_snapshot_cached(&broken, formula, &mut cache).is_err());
    assert!(Arc::ptr_eq(&prior_source, cache.cached_instance(source).unwrap()));
    assert!(Arc::ptr_eq(&prior_target, cache.cached_instance(target).unwrap()));
    assert_eq!(
        formula_from_snapshot_cached(&snapshot, formula, &mut cache).unwrap(),
        formula_from_snapshot(&snapshot, formula).unwrap()
    );

    cache.observe_events(
        &snapshot,
        formula,
        &frame(EventKind::NodeDeleted { node: NodeId(u64::MAX) }),
    );
    let materialized = formula_from_snapshot_cached(&snapshot, formula, &mut cache).unwrap();
    assert_eq!(materialized, formula_from_snapshot(&snapshot, formula).unwrap());
    assert!(!Arc::ptr_eq(&prior_source, cache.cached_instance(source).unwrap()));
    assert!(!Arc::ptr_eq(&prior_target, cache.cached_instance(target).unwrap()));
}
