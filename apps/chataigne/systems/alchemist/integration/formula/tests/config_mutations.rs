use super::*;
use golden_core::{engine::EngineTime, node::NodeCreationContext, process_ctx::ProcessCtx};

#[test]
fn config_reconciliation_uses_target_local_mutations_and_preserves_authored_values() {
    for read_only in [false, true] {
        for type_id in ["math", "constant"] {
            let (mut engine, formula) = engine_with_formula();
            let anode = create_anode(&mut engine, formula, type_id, 2.0, 3.0);
            if read_only {
                engine
                    .nodes
                    .get_mut(formula)
                    .unwrap()
                    .node_data_mut()
                    .meta
                    .tags
                    .push(super::super::FORMULA_EXTERNAL_READ_ONLY_TAG.into());
            }
            let snapshot = engine.process_tree_snapshot();
            let before = anode_from_snapshot(&snapshot, anode).unwrap();
            let mut ctx = ProcessCtx::new(
                ExecutionPhase::EngineTick,
                EngineTime {
                    tick: 1,
                    micro: 0,
                    seq: 0,
                },
            );
            ctx.set_tree_snapshot(snapshot);
            engine
                .nodes
                .get_mut(anode)
                .unwrap()
                .on_node_ready(&mut ctx, NodeCreationContext::ProjectLoad);
            let requests = ctx
                .edits
                .pending
                .iter()
                .filter_map(|request| match &request.edit {
                    Edit::CallNodeMutation {
                        needs_tree_snapshot, ..
                    } => Some(*needs_tree_snapshot),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(!requests.is_empty(), "real config refresh must exercise callbacks");
            assert!(
                requests.iter().all(|needed| !needed),
                "target-local config refresh cannot capture the whole graph"
            );
            engine.edits.pending.extend(ctx.edits.drain());
            engine.apply_edits().unwrap();
            let snapshot = engine.process_tree_snapshot();
            assert_eq!(anode_from_snapshot(&snapshot, anode).unwrap(), before);
            let config = find_child_by_decl(&engine, anode, "config").unwrap();
            for child in snapshot.child_ids(config) {
                if let Some(parameter) = engine
                    .nodes
                    .get(child)
                    .and_then(|node| node.as_any().downcast_ref::<Parameter>())
                {
                    assert_eq!(parameter.read_only, read_only);
                }
            }
        }
    }
}
