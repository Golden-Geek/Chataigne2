use super::*;
use crate::matrix_controls::{Caller, Scene, Scope};
use golden_core::{
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::NodeMetaPatch,
};
use mapgyver::applications::{
    demand::{ApplicationDemand, DemandReason},
    protocol::{ApplicationId, ApplicationPhase},
    runtime::ApplicationRuntime,
};

fn set_transition(scene: &mut Scene, scope: Scope, value: TransitionDefinition, enabled: bool) {
    let duration = scene.parameter_field(scope, "duration_ms");
    let curve = scene.parameter_field(scope, "curve");
    for (node, value) in [
        (duration, ParamValue::Float(f64::from(value.duration_ms))),
        (curve, ParamValue::Enum(format!("{:?}", value.curve))),
    ] {
        let ack = apply_project_intent(
            &mut scene.engine,
            UiEditIntent::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
    }
    let ack = apply_project_intent(
        &mut scene.engine,
        UiEditIntent::PatchMeta {
            node: duration,
            patch: NodeMetaPatch {
                enabled: Some(enabled),
                ..Default::default()
            },
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(
        scene
            .engine
            .nodes
            .get(duration)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(f64::from(value.duration_ms)),
    );
}

fn transition(duration_ms: u32, curve: TransitionCurve) -> TransitionDefinition {
    TransitionDefinition { duration_ms, curve }
}

fn expected(left: [f64; 4]) -> Image {
    let mut result = image([0.0; 4]);
    for (rgba, x) in [(left, 0.0), ([0.0, 0.0, 1.0, 1.0], 16.0)] {
        result.draw(
            &Image::solid(16, 24, rgba),
            &InstanceDefinition {
                transform: Transform2D {
                    position: [x, 0.0],
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
    }
    result
}

fn publish(fixture: &mut Fixture, scene: &Scene) {
    fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
}

fn layer_stop(scene: &mut Scene, caller: Caller, wanted: TransitionDefinition) -> MatrixPlaybackSnapshot {
    let layer = LayerId(scene.owner(Scope::Layer));
    scene.stop(Scope::Layer, caller);
    let stopped = scene.playback();
    let request = stopped.layers.iter().find(|request| request.layer == layer).unwrap();
    assert_eq!(request.transition, wanted);
    assert_eq!(request.column, None);
    stopped
}

fn set_layer_duration(scene: &mut Scene, duration_ms: u32) {
    let layer = LayerId(scene.owner(Scope::Layer));
    let settings = inspect(&mut scene.engine, scene.root.media)
        .items
        .into_iter()
        .find(|entry| entry.id == MatrixItemId::Layer { id: layer })
        .unwrap()
        .layer
        .unwrap();
    scene.matrix(MatrixEdit::ConfigureLayer {
        layer,
        settings: LayerSettings {
            duration_ms,
            ..settings
        },
    });
}

fn check_overrides(fixture: &mut Fixture, adapter: AdapterId, scene: &mut Scene) {
    use TransitionCurve::{Linear, Smooth};
    set_layer_duration(scene, TransitionDefinition::default().duration_ms);
    let column = transition(800, Smooth);
    let linear = transition(1000, Linear);
    let zero = transition(0, Linear);
    for caller in [Caller::Transport, Caller::Script] {
        for (label, column_enabled, (cell, cell_enabled), wanted) in [
            ("default", false, (linear, false), TransitionDefinition::default()),
            ("column-over-dormant-cell", true, (linear, false), column),
            ("cell-over-column", true, (linear, true), linear),
            ("zero-over-column", true, (zero, true), zero),
        ] {
            set_transition(scene, Scope::Column, column, column_enabled);
            set_transition(scene, Scope::Cell, cell, cell_enabled);
            scene.play();
            publish(fixture, scene);
            fixture.pixels(
                adapter,
                &expected([1.0, 0.0, 0.0, 1.0]),
                &format!("{caller:?}-{label}-before"),
            );
            let anchor = fixture.time_ms();
            let stopped = layer_stop(scene, caller, wanted);
            publish(fixture, scene);
            let duration = u64::from(wanted.duration_ms);
            if duration == 0 {
                fixture.pixels(
                    adapter,
                    &expected([1.0, 0.0, 0.0, 0.0]),
                    &format!("{caller:?}-{label}-cut"),
                );
                continue;
            }
            for percent in [25, 50, 75, 100] {
                let elapsed = duration * percent / 100;
                fixture.sample_at(anchor + elapsed);
                let t = elapsed as f64 / duration as f64;
                let alpha = 1.0
                    - match wanted.curve {
                        Linear => t,
                        Smooth => t * t * (3.0 - 2.0 * t),
                    };
                fixture.pixels(
                    adapter,
                    &expected([1.0, 0.0, 0.0, alpha]),
                    &format!("{caller:?}-{label}-{percent}"),
                );
                if percent == 50 {
                    assert_eq!(layer_stop(scene, caller, wanted), stopped);
                    publish(fixture, scene);
                }
            }
        }
    }
}

fn check_interruption(fixture: &mut Fixture, adapter: AdapterId, scene: &mut Scene) {
    scene.play();
    let layer = LayerId(scene.owner(Scope::Layer));
    let column = scene
        .playback()
        .layers
        .iter()
        .find(|request| request.layer != layer)
        .unwrap()
        .column
        .unwrap();
    let green = color(&mut scene.engine, [0.0, 1.0, 0.0, 1.0]);
    edit(
        &mut scene.engine,
        MediaEdit::ConfigureMediaResolution {
            media: green.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(16, 24).unwrap(),
            },
        },
    );
    scene.matrix(MatrixEdit::SetCells {
        assignments: vec![CellAssignment {
            layer,
            column,
            source: Some(green),
        }],
    });
    let fade = transition(800, TransitionCurve::Linear);
    scene.matrix(MatrixEdit::ConfigureColumn {
        column,
        definition: ColumnDefinition { transition: Some(fade) },
    });
    scene.play();
    publish(fixture, scene);
    scene.matrix(MatrixEdit::LaunchCell {
        layer,
        column,
        mode: LaunchMode::Configured,
    });
    let launch = fixture.time_ms();
    publish(fixture, scene);
    fixture.sample_at(launch + 200);
    fixture.pixels(adapter, &expected([0.75, 0.25, 0.0, 1.0]), "layer-stop-interrupted-mix");

    let anchor = fixture.time_ms();
    layer_stop(scene, Caller::Script, fade);
    publish(fixture, scene);
    fixture.sample_at(anchor + 400);
    fixture.pixels(
        adapter,
        &expected([0.75, 0.25, 0.0, 0.5]),
        "layer-stop-entire-outgoing-mix",
    );
    scene.matrix(MatrixEdit::LaunchCell {
        layer,
        column,
        mode: LaunchMode::Cut,
    });
    publish(fixture, scene);
    fixture.sample_at(anchor + 900);
    fixture.pixels(
        adapter,
        &expected([0.0, 1.0, 0.0, 1.0]),
        "layer-stop-reactivation-survives-old-deadline",
    );
}

fn demand(scene: &Scene, fixture: &mut Fixture) -> Vec<ApplicationDemand> {
    let time = fixture.renderer().read_time().unwrap();
    capture_channels_at(&scene.engine, &[scene.root], 16, time)
        .unwrap()
        .application_demand()
}

fn check_shared_process(adapter: AdapterId) {
    use super::scoped_stops::phase;

    let mut scene = Scene::with_owned_process();
    let wanted = transition(800, TransitionCurve::Smooth);
    set_layer_duration(&mut scene, wanted.duration_ms);
    scene.play();
    // The Matrix runtime projects its last observed renderer time with elapsed wall time
    // while committing application-backed Stops. Use its ordinary advancing clock here.
    let mut fixture = Fixture::new(adapter);
    let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
    let retirement = runtime.retirement();
    let initial = demand(&scene, &mut fixture);
    assert_eq!(initial.len(), 2);
    runtime.reconcile(&scene.engine, &initial).unwrap();
    let original = phase(&runtime, ApplicationPhase::Ready);
    assert_eq!(original.consumers.len(), 2);
    assert_eq!(runtime.snapshot().applications.len(), 1);
    fixture.renderer().bind_applications(runtime.plan_retainer());
    publish(&mut fixture, &scene);
    fixture.pixels(adapter, &expected([1.0, 0.0, 0.0, 1.0]), "owned-layer-stop-before");

    layer_stop(&mut scene, Caller::Transport, wanted);
    let stopped = ApplicationId(scene.owner(Scope::Cell));
    let outgoing = demand(&scene, &mut fixture);
    assert_eq!(outgoing.len(), 2);
    assert!(
        outgoing
            .iter()
            .any(|entry| entry.application == stopped && entry.reasons.contains(&DemandReason::Outgoing))
    );
    runtime.reconcile(&scene.engine, &outgoing).unwrap();
    publish(&mut fixture, &scene);
    assert_eq!(phase(&runtime, ApplicationPhase::Ready).process_id, original.process_id);

    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    let remaining = loop {
        let demand = demand(&scene, &mut fixture);
        if demand.iter().all(|entry| entry.application != stopped) {
            break demand;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "outgoing application demand did not expire"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(remaining.len(), 1);
    runtime.reconcile(&scene.engine, &remaining).unwrap();
    publish(&mut fixture, &scene);
    fixture.pixels(
        adapter,
        &expected([1.0, 0.0, 0.0, 0.0]),
        "owned-layer-stop-peer-remains",
    );
    assert_eq!(phase(&runtime, ApplicationPhase::Ready).process_id, original.process_id);

    scene.stop(Scope::All, Caller::Script);
    assert!(demand(&scene, &mut fixture).is_empty());
    runtime.reconcile(&scene.engine, &[]).unwrap();
    assert_eq!(phase(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
    publish(&mut fixture, &scene);
    fixture.pixels(adapter, &image([0.0; 4]), "owned-layer-stop-all-completed");
    phase(&runtime, ApplicationPhase::Idle);
    drop(fixture);
    runtime.close();
    drop(runtime);
    assert!(retirement.wait_for_idle(Duration::from_secs(4)));
    eprintln!(
        "Layer Stop native shared-process retirement passed for PID {:?}",
        original.process_id
    );
}

#[test]
#[ignore = "explicit native Layer Stop timing, override and interruption qualification"]
fn layer_stop_envelopes_match_timed_gpu_pixels_on_each_adapter() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Layer Stop native watchdog expired");
            std::process::exit(1);
        }
    });
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!adapters.is_empty());
    for adapter in adapters {
        let mut scene = Scene::new(None);
        let mut fixture = Fixture::with_stepped_clock(adapter.id);
        check_overrides(&mut fixture, adapter.id, &mut scene);
        check_interruption(&mut fixture, adapter.id, &mut scene);
        drop(fixture);
        check_shared_process(adapter.id);
        eprintln!(
            "Layer Stop timed pixels and renderer retirement passed on {}",
            adapter.description
        );
    }
    done.send(()).unwrap();
}
