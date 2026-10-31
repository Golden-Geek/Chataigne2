use super::*;
use crate::engine::apply_opaque::AuthoredNodeState;
use crate::node::Folder;

#[crate::node("gesture_scope_probe", impl_node)]
struct ScopeProbe {
    #[state(default = 0, persist)]
    count: i32,
}

#[test]
fn self_data_scope_skips_graph_invalidation_but_full_scope_and_metadata_do_not() {
    use crate::edit::{Edit, NodeMutationScope};
    for (scope, rename, expected_dirty) in [
        (NodeMutationScope::SelfData, false, false),
        (NodeMutationScope::SelfData, true, true),
        (NodeMutationScope::Full, false, true),
    ] {
        let mut engine = Engine::new(ScopeProbe::new());
        engine.apply_edits().unwrap();
        engine.clear_history();
        engine.edits.push(Edit::CallNodeMutationCoalesced {
            node: engine.root,
            needs_tree_snapshot: false,
            scope,
            callback: Box::new(move |node, _| {
                node.as_any_mut().downcast_mut::<ScopeProbe>().unwrap().count = 1;
                if rename {
                    node.node_data_mut().meta.label = "renamed".into();
                }
                Ok(())
            }),
        });
        engine.apply_edits().unwrap();
        let HistoryStep::AuthoredNode(effect) = &engine.undo_stack.last().unwrap().steps[0] else {
            panic!()
        };
        assert_eq!(effect.graph_dirty, expected_dirty);
        assert!(engine.undo().unwrap());
        assert_eq!(engine.nodes.get(engine.root).unwrap().count, 0);
        assert!(engine.redo().unwrap());
        assert_eq!(engine.nodes.get(engine.root).unwrap().count, 1);
    }
}

fn state(node: &mut Folder, label: &str) -> AuthoredNodeState {
    node.node_data_mut().meta.label = label.into();
    AuthoredNodeState::capture(node).unwrap()
}

fn step(node: &mut Folder, from: &str, to: &str, coalesce: bool, graph_dirty: bool) -> HistoryStep<Folder> {
    HistoryStep::AuthoredNode(AuthoredNodeEffect {
        node: node.node_data().id,
        old: state(node, from),
        new: state(node, to),
        coalesce,
        graph_dirty,
    })
}

#[test]
fn a_long_opted_in_gesture_retains_only_original_and_final_state() {
    let mut node = Folder::new("0");
    let before = state(&mut node, "0");
    let mut transaction = HistoryTransaction::new();
    for index in 0..10_000 {
        transaction.push_session_step(step(
            &mut node,
            &index.to_string(),
            &(index + 1).to_string(),
            true,
            index == 1,
        ));
    }
    assert_eq!(transaction.steps.len(), 1);
    let HistoryStep::AuthoredNode(effect) = &transaction.steps[0] else {
        panic!()
    };
    assert!(effect.old == before);
    assert!(effect.new == state(&mut node, "10000"));
    assert!(effect.graph_dirty);
}

#[test]
fn normal_transactions_and_noncontiguous_or_nonopted_in_changes_do_not_merge() {
    let mut node = Folder::new("0");
    let mut normal = HistoryTransaction::new();
    normal.push(step(&mut node, "0", "1", true, false));
    normal.push(step(&mut node, "1", "2", true, false));
    assert_eq!(normal.steps.len(), 2);
    let mut session = HistoryTransaction::new();
    session.push_session_step(step(&mut node, "0", "1", true, false));
    session.push_session_step(step(&mut node, "1", "2", false, false));
    session.push_session_step(step(&mut node, "2", "3", true, false));
    session.push_session_step(step(&mut node, "unrelated", "4", true, false));
    assert_eq!(session.steps.len(), 4);
}
