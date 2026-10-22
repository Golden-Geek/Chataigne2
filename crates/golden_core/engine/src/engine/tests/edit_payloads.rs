use super::*;
use crate::{
    node::{Folder, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use golden_io::PayloadPool;
use std::time::Duration;

#[test]
fn admitted_source_survives_its_producer_until_queued_edits_apply_or_fail() {
    for fail in [false, true] {
        let pool = PayloadPool::new("edit-source", 1, 1024).unwrap();
        let source = pool.try_retain(vec![1_u8; 256], 512).unwrap();
        let mut engine = Engine::new(Folder::new("root"));
        engine.edits.push(Edit::PatchMeta {
            node: engine.root,
            patch: NodeMetaPatch {
                label: Some("changed".into()),
                ..Default::default()
            },
        });
        if fail {
            engine.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
            engine.edits.push(Edit::PatchMeta {
                node: engine.root,
                patch: NodeMetaPatch {
                    label: Some("discarded".into()),
                    ..Default::default()
                },
            });
        }
        let receipt = engine.edits.completion();
        engine.edits.retain_payload(source);
        assert!(
            pool.try_reserve(1).is_err(),
            "source must remain charged after the producer releases it"
        );
        assert_eq!(engine.apply_edits().is_err(), fail);
        assert!(
            pool.wait_for_idle(Duration::from_secs(2)),
            "an ordinary observer must not extend the source lease"
        );
        assert_eq!(
            receipt.status(),
            if fail {
                crate::edit::EditCompletionStatus::Failed
            } else {
                crate::edit::EditCompletionStatus::Applied
            }
        );
    }
}

#[test]
fn retained_source_follows_ingress_atomic_completion_and_queue_discard() {
    let pool = PayloadPool::new("edit-source-atomic", 1, 1024).unwrap();
    let mut engine = Engine::new(Folder::new("root"));
    let source = pool.try_retain(vec![1_u8; 256], 512).unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch {
            label: Some("atomic".into()),
            ..Default::default()
        },
    });
    ctx.edits.retain_payload(source);
    engine
        .apply_atomic_edit_operation(|engine| {
            engine.absorb_edits(&mut ctx)?;
            engine.apply_edits()?;
            assert!(
                pool.try_reserve(1).is_err(),
                "application inside an uncommitted operation must retain admission"
            );
            Ok::<(), EngineEditError>(())
        })
        .unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let source = pool.try_retain(vec![2_u8; 256], 512).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch::default(),
    });
    engine.edits.retain_payload(source);
    engine.edits.pending.clear();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
