use super::*;
use crate::{matrix::protocol::*, media::capture_channels_at};
use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeSnapshot};
use golden_video::plan::{BlendMode, Layer, RenderOperation};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Default)]
struct Clock(AtomicU64);
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::Relaxed)))
    }
}
fn at(anchor: TimeSnapshot, millis: u64) -> TimeSnapshot {
    anchor
        .at_monotonic(MonotonicTime::from_nanos(millis * 1_000_000))
        .unwrap()
}
fn matrix_edit(engine: &mut AppEngine, media: MediaId, operation: MatrixEdit) -> MediaEditResult {
    edit(engine, MediaEdit::Matrix { media, edit: operation })
}
fn launch(engine: &mut AppEngine, media: MediaId, column: ColumnId, mode: LaunchMode) {
    matrix_edit(engine, media, MatrixEdit::LaunchColumn { column, mode });
}
fn fixture() -> (
    AppEngine,
    ChannelReference,
    Vec<ChannelReference>,
    ChannelReference,
    Vec<ColumnId>,
) {
    let mut engine = engine();
    let sources = (0..3)
        .map(|_| {
            let channel = create(&mut engine, MediaKind::SolidColor);
            resize(&mut engine, channel, 4);
            channel
        })
        .collect::<Vec<_>>();
    let matrix = create(&mut engine, MediaKind::Matrix);
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::Configure {
            definition: MatrixDefinition {
                canvas: Extent2D::new(4, 2).unwrap(),
                transition: TransitionDefinition {
                    duration_ms: 1000,
                    curve: TransitionCurve::Linear,
                },
                ..Default::default()
            },
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    );
    matrix_edit(&mut engine, matrix.media, MatrixEdit::AddColumns { count: 4 });
    let MediaEditResult::Matrix { snapshot } = matrix_edit(&mut engine, matrix.media, MatrixEdit::Inspect) else {
        panic!()
    };
    let layer = snapshot.items[0].channel;
    let MatrixItemId::Layer { id } = snapshot.items[0].id else {
        panic!()
    };
    let columns = snapshot.columns.iter().map(|column| column.id).collect::<Vec<_>>();
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: sources
                .iter()
                .enumerate()
                .map(|(i, source)| CellAssignment {
                    layer: id,
                    column: columns[i],
                    source: Some(*source),
                })
                .collect(),
        },
    );
    (engine, matrix, sources, layer, columns)
}
fn prepared(
    capture: &CapturedChannels,
    matrix: ChannelReference,
    owner: &ChannelRenderer,
) -> super::super::plan::Prepared {
    super::super::plan::build(
        capture,
        super::super::plan::Targets {
            routes: &routes(matrix),
            preview: None,
            screens: &[],
        },
        super::super::plan::Previous {
            channels: &Default::default(),
            screens: &Default::default(),
        },
        &mut 3,
        config().plan_limits,
        &owner.source_observations(),
        None,
    )
    .unwrap()
}
fn layers(prepared: &super::super::plan::Prepared, reference: ChannelReference) -> &[Layer] {
    let pass = prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == prepared.entries[&reference].target)
        .unwrap();
    let RenderOperation::Composite { layers } = &pass.operation else {
        panic!()
    };
    layers
}

#[test]
fn matrix_fades_share_sources_preserve_interrupted_weights_and_withdraw_outgoing_only_at_completion() {
    let (mut engine, matrix, sources, layer, columns) = fixture();
    let mut authority =
        TimeAuthority::new(Clock::default(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let time = authority.sample().unwrap();
    let owner = renderer();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    capture_channels_at(&engine, &[matrix], 16, time).unwrap();
    launch(&mut engine, matrix.media, columns[1], LaunchMode::Configured);
    let capture = capture_channels_at(&engine, &[matrix, layer], 16, time).unwrap();
    assert_eq!(capture.channels.len(), 4);
    let first = prepared(&capture, matrix, &owner);
    let row = layers(&first, layer);
    assert_eq!(row.len(), 2);
    assert!(row.iter().all(|layer| layer.blend == BlendMode::Add));
    assert_eq!(
        row.iter()
            .map(|layer| layer.evaluated(at(time, 500)).opacity.get())
            .collect::<Vec<_>>(),
        [0.5, 0.5]
    );
    launch(&mut engine, matrix.media, columns[2], LaunchMode::Configured);
    assert!(!capture.is_current());
    let capture = capture_channels_at(&engine, &[matrix], 16, at(time, 500)).unwrap();
    assert_eq!(capture.channels.len(), 5);
    let interrupted = prepared(&capture, matrix, &owner);
    assert_eq!(
        layers(&interrupted, layer)
            .iter()
            .map(|layer| layer.evaluated(at(time, 1000)).opacity.get())
            .collect::<Vec<_>>(),
        [0.25, 0.25, 0.5]
    );
    let still_outgoing = capture_channels_at(&engine, &[matrix], 16, at(time, 1499)).unwrap();
    assert!(sources.iter().all(|reference| {
        still_outgoing
            .channels
            .iter()
            .any(|channel| channel.reference == *reference)
    }));
    let completed = capture_channels_at(&engine, &[matrix], 16, at(time, 1500)).unwrap();
    assert_eq!(completed.channels.len(), 3);
    assert!(completed.channels.iter().any(|channel| channel.reference == sources[2]));
    launch(&mut engine, matrix.media, columns[3], LaunchMode::Configured);
    let empty = capture_channels_at(&engine, &[matrix], 16, at(time, 1500)).unwrap();
    let fading_out = prepared(&empty, matrix, &owner);
    assert_eq!(
        layers(&fading_out, layer)[0].evaluated(at(time, 2000)).opacity.get(),
        0.5
    );
    assert_eq!(
        capture_channels_at(&engine, &[matrix], 16, at(time, 2500))
            .unwrap()
            .channels
            .len(),
        1
    );
    close(owner);
}

#[test]
fn renderer_refresh_publishes_once_at_fade_completion_without_per_frame_plan_churn() {
    let (mut engine, matrix, sources, _, columns) = fixture();
    let clock = Arc::new(Clock::default());
    let config = config();
    let mut owner = ChannelRenderer::new(
        VideoRuntime::spawn_null_with_clock(config.clone(), NullConfig::default(), clock.clone()).unwrap(),
        config.plan_limits,
    );
    wait(|| owner.snapshot().resources.is_some());
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let time = owner.read_time().unwrap();
    owner
        .publish(
            capture_channels_at(&engine, &[matrix], 16, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    launch(&mut engine, matrix.media, columns[1], LaunchMode::Configured);
    let time = owner.read_time().unwrap();
    owner
        .publish(
            capture_channels_at(&engine, &[matrix], 16, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    let publication = owner.latest_request();
    for millis in [0, 100, 500, 999] {
        clock.0.store(millis * 1_000_000, Ordering::Relaxed);
        assert!(owner.refresh().unwrap().is_none());
        assert_eq!(owner.latest_request(), publication);
    }
    clock.0.store(1_000_000_000, Ordering::Relaxed);
    assert!(owner.refresh().unwrap().is_some());
    assert!(!owner.channels().iter().any(|channel| channel.reference == sources[0]));
    assert!(owner.channels().iter().any(|channel| channel.reference == sources[1]));
    assert!(owner.refresh().unwrap().is_none());
    close(owner);
}

#[test]
fn topology_discovery_defers_fades_until_bound_and_shared_presentation_reads_do_not_restart_them() {
    use crate::preview::{
        capture::{capture_presentation, capture_presentation_at},
        protocol::PreviewSource,
    };
    let (mut engine, matrix, _, layer, columns) = fixture();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Configured);
    let sources = [PreviewSource::Channel { reference: matrix }];
    let topology = capture_presentation(&engine, &sources, 8, 128, &[]).unwrap();
    assert_eq!(topology.channels.channels.len(), 1);
    assert!(topology.channels.channels[0].matrix.is_none());
    let time = TimeAuthority::new(Clock::default(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap();
    let first = capture_presentation_at(&engine, &sources, 8, 128, &[], time).unwrap();
    let owner = renderer();
    let first = prepared(&first.channels, matrix, &owner);
    assert_eq!(layers(&first, layer)[0].evaluated(at(time, 500)).opacity.get(), 0.5);
    let repeated = capture_presentation_at(&engine, &sources, 8, 128, &[], at(time, 250)).unwrap();
    let repeated = prepared(&repeated.channels, matrix, &owner);
    assert_eq!(layers(&repeated, layer)[0].evaluated(at(time, 500)).opacity.get(), 0.5);
    close(owner);
}

#[test]
fn configured_launch_bursts_preserve_fixed_publication_credits_and_recover_after_outcome_drain() {
    let (mut engine, matrix, _, _, columns) = fixture();
    let mut owner = renderer();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    capture_channels_at(&engine, &[matrix], 16, owner.read_time().unwrap()).unwrap();
    let history = engine.undo_len();
    let mut accepted = 0;
    for index in 0..1000 {
        launch(&mut engine, matrix.media, columns[index % 3], LaunchMode::Configured);
        let capture = capture_channels_at(&engine, &[matrix], 16, owner.read_time().unwrap()).unwrap();
        if owner.publish(capture, routes(matrix)).is_ok() {
            accepted += 1;
        }
    }
    assert_eq!(accepted, config().max_outstanding.get());
    assert_eq!(engine.undo_len(), history);
    let mut outcomes = Vec::new();
    wait(|| {
        owner.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        outcomes.len() == accepted
    });
    launch(&mut engine, matrix.media, columns[2], LaunchMode::Cut);
    let capture = capture_channels_at(&engine, &[matrix], 16, owner.read_time().unwrap()).unwrap();
    assert!(owner.publish(capture, routes(matrix)).is_ok());
    let resources = owner.snapshot().resources.unwrap();
    assert!(resources.bytes_used <= resources.byte_limit);
    assert!(resources.in_flight <= 2 && resources.retiring_generations <= 2 && resources.prepared_generations <= 1);
    close(owner);
}

#[test]
#[ignore = "M07 gate 2: retain outgoing resolved bindings across column removal/reassignment until final use"]
fn outgoing_cell_bindings_survive_column_removal_until_transition_end() {
    let (mut engine, matrix, sources, _, columns) = fixture();
    let time = TimeAuthority::new(Clock::default(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    capture_channels_at(&engine, &[matrix], 16, time).unwrap();
    launch(&mut engine, matrix.media, columns[1], LaunchMode::Configured);
    capture_channels_at(&engine, &[matrix], 16, time).unwrap();
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RemoveColumn { column: columns[0] },
    );
    let halfway = capture_channels_at(&engine, &[matrix], 16, at(time, 500)).unwrap();
    assert!(
        halfway.channels.iter().any(|channel| channel.reference == sources[0]),
        "outgoing source was withdrawn before the fade ended"
    );
    assert!(halfway.channels.iter().any(|channel| channel.reference == sources[1]));
    let completed = capture_channels_at(&engine, &[matrix], 16, at(time, 1000)).unwrap();
    assert!(!completed.channels.iter().any(|channel| channel.reference == sources[0]));
}
