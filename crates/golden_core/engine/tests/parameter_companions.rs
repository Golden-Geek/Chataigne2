extern crate golden_engine as golden_core;

use golden_engine::{
    edit::Edit,
    engine::Engine,
    node::{Folder, Node, NodeId, ParameterValidationContext},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

#[golden_engine::node("catalog_selection")]
#[children(
    selected: i32 = 0;
    hint: i32 = 0;
    peer: i32 = 0;
    point: golden_engine::parameter::Vec2 = (0.0, 0.0);
)]
struct Selection {
    #[state(default = Arc::new(AtomicI32::new(10)))]
    catalog: Arc<AtomicI32>,
    #[state(default = false)]
    inconsistent: bool,
}

#[golden_engine::node("catalog_selection", from_struct)]
impl Node for Selection {
    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        if change.parameter == self.selected.id() || change.parameter == self.point.id() {
            let selected = change.parameter_value(self.selected.id()).unwrap();
            let hint = if selected == ParamValue::Int(0) {
                0
            } else {
                self.catalog.load(Ordering::SeqCst)
            };
            let mut values = vec![(self.hint.id(), ParamValue::Int(hint))];
            if self.inconsistent {
                values.push((self.hint.id(), ParamValue::Int(hint + 1)));
            }
            return Ok(values);
        }
        if change.parameter == self.hint.id() {
            assert!(change.is_proposed(self.hint.id()));
            return Ok(vec![(self.peer.id(), ParamValue::Int(20))]);
        }
        Ok(Vec::new())
    }
    fn normalize_parameter_change(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ParamValue>, String> {
        if change.parameter == self.hint.id()
            && let ParamValue::Int(value) = change.value
            && *value > 0
        {
            return Ok(Some(ParamValue::Int(self.catalog.load(Ordering::SeqCst))));
        }
        Ok(None)
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        let selected = change.parameter_value(self.selected.id()).unwrap();
        let hint = change.parameter_value(self.hint.id()).unwrap();
        if (selected == ParamValue::Int(0)) != (hint == ParamValue::Int(0)) {
            return Err("selection and its stored hint must agree".into());
        }
        Ok(())
    }
}

golden_engine::define_node_enum!(
    enum TestNode {
        Selection(Selection),
    }
);

fn fixture() -> (Engine<TestNode>, NodeId, [NodeId; 4], Arc<AtomicI32>) {
    let mut engine = Engine::<TestNode>::new(Folder::new("Root").into());
    engine.add_node(Selection::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let TestNode::Selection(node) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    let ids = [node.selected.id(), node.hint.id(), node.peer.id(), node.point.id()];
    let catalog = node.catalog.clone();
    engine.clear_history();
    engine.clear_ui_event_log();
    (engine, id, ids, catalog)
}
fn value(engine: &Engine<TestNode>, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
fn set(node: NodeId, value: i32) -> Edit {
    Edit::SetParam {
        node,
        value: ParamValue::Int(value),
        behaviour: ParameterEventBehaviour::Append,
    }
}

#[test]
fn scalar_companions_validate_commit_and_replay_as_one_complete_canonical_operation() {
    let (mut engine, _, [selected, hint, peer, _], catalog) = fixture();
    engine.edits.push(set(selected, 1));
    engine.apply_edits().unwrap();
    assert_eq!(
        (value(&engine, selected), value(&engine, hint), value(&engine, peer)),
        (ParamValue::Int(1), ParamValue::Int(10), ParamValue::Int(0)),
        "derived writes never recursively expand"
    );
    assert_eq!(engine.undo_len(), 1);
    engine.edits.push(set(selected, 1));
    engine.apply_edits().unwrap();
    assert_eq!(engine.undo_len(), 1, "unchanged companions create no history");
    catalog.store(99, Ordering::SeqCst);
    assert!(engine.undo().unwrap());
    assert_eq!(
        (value(&engine, selected), value(&engine, hint)),
        (ParamValue::Int(0), ParamValue::Int(0))
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        (value(&engine, selected), value(&engine, hint)),
        (ParamValue::Int(1), ParamValue::Int(10)),
        "catalog changes cannot rewrite captured hints on replay"
    );
}

#[test]
fn explicit_and_derived_conflicts_reject_without_any_partial_change() {
    let (mut engine, owner, [selected, hint, _, _], _) = fixture();
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![(selected, ParamValue::Int(1)), (hint, ParamValue::Int(77))],
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(
        (value(&engine, selected), value(&engine, hint)),
        (ParamValue::Int(0), ParamValue::Int(0))
    );
    assert_eq!(engine.undo_len(), 0);
    assert!(engine.ui_event_log().is_empty());
    let TestNode::Selection(node) = engine.nodes.get_mut(owner).unwrap() else {
        panic!()
    };
    node.inconsistent = true;
    engine.edits.push(set(selected, 1));
    assert!(engine.apply_edits().is_err());
    assert_eq!(
        (value(&engine, selected), value(&engine, hint)),
        (ParamValue::Int(0), ParamValue::Int(0))
    );
    assert!(engine.ui_event_log().is_empty());
}

#[test]
fn identical_explicit_companions_deduplicate_and_expose_original_membership() {
    let (mut engine, _, [selected, hint, peer, _], _) = fixture();
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![(selected, ParamValue::Int(1)), (hint, ParamValue::Int(10))],
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        (value(&engine, selected), value(&engine, hint), value(&engine, peer)),
        (ParamValue::Int(1), ParamValue::Int(10), ParamValue::Int(20))
    );
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(
        (value(&engine, selected), value(&engine, hint), value(&engine, peer)),
        (ParamValue::Int(0), ParamValue::Int(0), ParamValue::Int(0))
    );
}

#[test]
fn component_edits_include_owner_companions_in_exact_undo_and_redo() {
    let (mut engine, _, [selected, hint, _, point], catalog) = fixture();
    engine.edits.push(set(selected, 1));
    engine.apply_edits().unwrap();
    engine.clear_history();
    catalog.store(20, Ordering::SeqCst);
    engine.edits.push(Edit::SetParamComponent {
        node: point,
        component: 0,
        value: 0.3,
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        (value(&engine, point), value(&engine, hint)),
        (ParamValue::Vec2(0.3, 0.0), ParamValue::Int(20))
    );
    catalog.store(30, Ordering::SeqCst);
    assert!(engine.undo().unwrap());
    assert_eq!(
        (value(&engine, point), value(&engine, hint)),
        (ParamValue::Vec2(0.0, 0.0), ParamValue::Int(10))
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        (value(&engine, point), value(&engine, hint)),
        (ParamValue::Vec2(0.3, 0.0), ParamValue::Int(20))
    );
}
