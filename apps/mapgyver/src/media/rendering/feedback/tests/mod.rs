use super::*;
use crate::{
    app::AppEngine,
    media::{capture_channels, protocol::*, rendering::tests::*},
};
use golden_video::runtime::{NullConfig, VideoRuntime};
use std::time::Duration;

fn fixture() -> (AppEngine, ChannelReference, Vec<ColumnId>) {
    let mut engine = engine();
    let matrix = create(&mut engine, MediaKind::Matrix);
    edit(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::Configure {
                definition: MatrixDefinition {
                    canvas: golden_video::Extent2D::new(4, 2).unwrap(),
                    ..Default::default()
                },
            },
        },
    );
    let MediaEditResult::MatrixCreated { columns, .. } = edit(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumns { count: 2 },
        },
    ) else {
        panic!()
    };
    (engine, matrix, columns)
}
fn launch(engine: &mut AppEngine, matrix: ChannelReference, column: ColumnId) {
    edit(
        engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::LaunchColumn {
                column,
                mode: LaunchMode::Cut,
            },
        },
    );
}
fn publish(owner: &mut ChannelRenderer, engine: &AppEngine, matrix: ChannelReference) -> RequestId {
    owner
        .publish(capture_channels(engine, &[matrix], 16).unwrap(), routes(matrix))
        .unwrap()
}
fn status(owner: &ChannelRenderer) -> MatrixRenderStatus {
    owner.matrix_statuses().remove(0).1
}

#[test]
fn requests_installation_and_submission_are_distinct_and_pending_launch_preserves_previous_plan() {
    let (mut engine, matrix, columns) = fixture();
    let mut owner = renderer();
    assert!(owner.matrix_statuses().is_empty());
    launch(&mut engine, matrix, columns[0]);
    let request = publish(&mut owner, &engine, matrix);
    // Reliable outcome has not been observed: request acceptance cannot authorize installation.
    let pending = status(&owner);
    assert!(pending.installed.is_none() && pending.submitted.is_none());
    installed(&owner, request);
    wait(|| owner.snapshot().last_frame.is_some());
    let first_frame = owner.snapshot().last_frame.unwrap();
    let rendered = status(&owner);
    assert_eq!(
        rendered.installed.as_ref().unwrap().playback.desired_column,
        Some(columns[0])
    );
    assert_eq!(
        rendered.submitted.as_ref().unwrap().playback.launch_id,
        rendered.desired.playback.launch_id
    );
    let mut no_frame = owner.snapshot();
    no_frame.last_frame = None;
    let only_installed = owner.matrix_statuses_from(no_frame).remove(0).1;
    assert!(only_installed.installed.is_some() && only_installed.submitted.is_none());
    launch(&mut engine, matrix, columns[1]);
    assert_eq!(status(&owner).stage, MatrixPreparationStage::Changed);
    let request = publish(&mut owner, &engine, matrix);
    let pending = status(&owner);
    assert_eq!(pending.desired.playback.desired_column, Some(columns[1]));
    if let Some(previous) = pending.installed {
        assert_eq!(previous.playback.desired_column, Some(columns[0]));
    }
    installed(&owner, request);
    let mut wrong_frame = owner.snapshot();
    wrong_frame.last_frame = Some(first_frame);
    assert!(owner.matrix_statuses_from(wrong_frame).remove(0).1.submitted.is_none());
    wait(|| {
        status(&owner)
            .submitted
            .as_ref()
            .is_some_and(|stamp| stamp.playback.desired_column == Some(columns[1]))
    });
    let stable = owner.matrix_statuses();
    let frames = owner.snapshot().frames_submitted;
    wait(|| owner.snapshot().frames_submitted > frames);
    assert_eq!(owner.matrix_statuses(), stable); // No per-frame UI event churn.
    let foreign_time = renderer();
    let mut stale_time = owner.snapshot();
    stale_time.time = foreign_time.read_time().unwrap();
    assert!(owner.matrix_statuses_from(stale_time).remove(0).1.submitted.is_none());
    close(foreign_time);
    let mut lost = owner.snapshot();
    lost.resources.as_mut().unwrap().device_lost = true;
    let lost = owner.matrix_statuses_from(lost).remove(0).1;
    assert_eq!(lost.stage, MatrixPreparationStage::Recovering);
    assert!(lost.installed.is_none() && lost.submitted.is_none());
    let mut faulted = owner.snapshot();
    faulted.state = RuntimeState::Faulted;
    let faulted = owner.matrix_statuses_from(faulted).remove(0).1;
    assert_eq!(faulted.stage, MatrixPreparationStage::Faulted);
    assert!(faulted.submitted.is_none());
    owner.close();
    assert!(owner.matrix_statuses().is_empty());
    close(owner);
}

#[test]
fn rejected_preparation_does_not_replace_observed_output_and_recovery_clears_current_error() {
    let (mut engine, matrix, columns) = fixture();
    let mut runtime_config = config();
    let authoring_limits = runtime_config.plan_limits;
    // App preparation admits two routes; the stricter runtime rejects them during CPU compilation.
    runtime_config.plan_limits.max_outputs = 1;
    let mut owner = ChannelRenderer::new(
        VideoRuntime::spawn_null(runtime_config, NullConfig::default()).unwrap(),
        authoring_limits,
    );
    wait(|| owner.snapshot().resources.is_some());
    let one_route = vec![routes(matrix)[0]];
    launch(&mut engine, matrix, columns[0]);
    let request = owner
        .publish(capture_channels(&engine, &[matrix], 16).unwrap(), one_route.clone())
        .unwrap();
    installed(&owner, request);
    wait(|| status(&owner).submitted.is_some());
    launch(&mut engine, matrix, columns[1]);
    let request = publish(&mut owner, &engine, matrix);
    wait(|| {
        let mut outcomes = vec![];
        owner.drain_outcomes(&mut outcomes, std::num::NonZeroUsize::new(16).unwrap());
        outcomes
            .iter()
            .any(|outcome| outcome.request == request && matches!(outcome.result, OutcomeKind::Rejected(_)))
    });
    let rejected = status(&owner);
    assert_eq!(rejected.stage, MatrixPreparationStage::Rejected);
    assert!(rejected.error.is_some());
    assert_eq!(rejected.installed.unwrap().playback.desired_column, Some(columns[0]));
    let request = owner
        .publish(capture_channels(&engine, &[matrix], 16).unwrap(), one_route)
        .unwrap();
    installed(&owner, request);
    wait(|| {
        status(&owner)
            .submitted
            .as_ref()
            .is_some_and(|stamp| stamp.playback.desired_column == Some(columns[1]))
    });
    assert!(status(&owner).error.is_none());
    close(owner);
}

#[test]
fn reload_with_identical_persisted_ids_fences_old_plan_observations() {
    use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
    let (mut engine, matrix, columns) = fixture();
    let mut owner = renderer();
    launch(&mut engine, matrix, columns[0]);
    let request = publish(&mut owner, &engine, matrix);
    installed(&owner, request);
    wait(|| status(&owner).submitted.is_some());
    let before = status(&owner);
    let restored =
        from_sparse_project_json::<crate::app::AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    let request = publish(&mut owner, &restored, matrix);
    let after = status(&owner);
    assert_ne!(after.project, before.project);
    assert!(after.installed.is_none() && after.submitted.is_none());
    installed(&owner, request);
    close(owner);
}

#[test]
fn burst_correlation_is_bounded_by_runtime_credits_and_drained_outcomes_release_all_metadata() {
    let (mut engine, matrix, columns) = fixture();
    let config = config();
    let bound = config.max_outstanding.get();
    let mut owner = ChannelRenderer::new(
        VideoRuntime::spawn_null(
            config.clone(),
            NullConfig {
                preparation_delay: Duration::from_millis(30),
                ..Default::default()
            },
        )
        .unwrap(),
        config.plan_limits,
    );
    wait(|| owner.snapshot().resources.is_some());
    for i in 0..bound {
        launch(&mut engine, matrix, columns[i % 2]);
        publish(&mut owner, &engine, matrix);
        assert!(owner.feedback.lock().unwrap().pending.len() <= bound);
    }
    assert!(
        owner
            .publish(capture_channels(&engine, &[matrix], 16).unwrap(), routes(matrix))
            .is_err()
    );
    wait(|| {
        let mut outcomes = vec![];
        owner.drain_outcomes(&mut outcomes, std::num::NonZeroUsize::new(bound).unwrap());
        owner.feedback.lock().unwrap().pending.is_empty()
    });
    assert_eq!(owner.snapshot().outstanding, 0);
    close(owner);
}
