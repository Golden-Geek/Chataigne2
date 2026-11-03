use super::Observations;
use crate::{
    matrix::protocol::{MatrixPlanStamp, MatrixPreparationStage, MatrixRenderStatus},
    media::{
        protocol::{ChannelId, ChannelReference, MediaId},
        resolution::ChannelAvailability,
    },
    preview::protocol::{PreviewSource, PreviewStatus},
};
use golden_core::model::NodeUuid;

fn submitted() -> PreviewStatus {
    let stamp = MatrixPlanStamp {
        playback: Default::default(),
        revision: 7,
        columns: vec![],
        transitioning: false,
        layers: [].into(),
    };
    PreviewStatus {
        source: PreviewSource::Channel {
            reference: ChannelReference {
                media: MediaId(NodeUuid(uuid::Uuid::new_v4())),
                channel: ChannelId(NodeUuid(uuid::Uuid::new_v4())),
            },
        },
        extent: None,
        error: None,
        message: None,
        matrix: Some(MatrixRenderStatus {
            project: uuid::Uuid::new_v4(),
            desired: stamp.clone(),
            stage: MatrixPreparationStage::Installed,
            availability: ChannelAvailability::Ready,
            installed: Some(stamp.clone()),
            submitted: Some(stamp),
            error: None,
        }),
    }
}

#[test]
fn final_consumer_retirement_replaces_prior_matrix_submission_without_a_layout_change_and_reconnect_republishes() {
    let mut owner = Observations::default();
    let current = vec![submitted()];
    let mut events = Vec::new();
    owner
        .publish(4, current.clone(), |statuses| {
            events.push(statuses.to_vec());
            Ok(())
        })
        .unwrap();
    owner
        .publish(4, current, |_| panic!("unchanged frames must not republish"))
        .unwrap();
    owner
        .publish(4, vec![], |statuses| {
            events.push(statuses.to_vec());
            Ok(())
        })
        .unwrap();
    owner
        .publish(4, vec![], |_| panic!("idle must stay event driven"))
        .unwrap();
    owner
        .publish(5, vec![], |statuses| {
            events.push(statuses.to_vec());
            Ok(())
        })
        .unwrap();
    assert!(events[0][0].matrix.as_ref().unwrap().submitted.is_some());
    assert!(
        events[1].is_empty(),
        "retirement clears installed/submitted Matrix observations"
    );
    assert!(
        events[2].is_empty(),
        "a new preview generation receives the current empty snapshot"
    );
    assert_eq!(events.len(), 3);
}

#[test]
fn failed_retirement_delivery_is_retried_instead_of_caching_a_clear_that_the_ui_never_received() {
    let mut owner = Observations::default();
    owner.publish(1, vec![submitted()], |_| Ok(())).unwrap();
    assert!(
        owner
            .publish(1, vec![], |_| Err("window emission failed".into()))
            .is_err()
    );
    let mut delivered = false;
    owner
        .publish(1, vec![], |statuses| {
            assert!(statuses.is_empty());
            delivered = true;
            Ok(())
        })
        .unwrap();
    assert!(delivered);
    owner
        .publish(1, vec![], |_| panic!("confirmed clear is not replayed"))
        .unwrap();
}
