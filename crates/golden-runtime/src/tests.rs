use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use golden_model::EntityId;
use golden_values::Value;

use super::*;

#[derive(Default)]
struct Collector {
    effects: Vec<CommittedEffect>,
}

impl EffectCommitter for Collector {
    fn commit(&mut self, effect: &CommittedEffect) {
        self.effects.push(effect.clone());
    }
}

fn workload(processors: usize, lanes: usize, id: GenerationId) -> GenerationSpec {
    let instances = processors * lanes;
    let mut slots = Vec::with_capacity(instances * 3);
    let mut inputs = Vec::with_capacity(instances);
    let mut state = Vec::with_capacity(instances);
    for processor in 0..processors {
        for lane in 0..lanes {
            let base = slots.len() as u32;
            slots.push(SlotSpec {
                key: format!("p{processor}.l{lane}.input").into(),
                initial: ScalarValue::Float(0.0),
            });
            slots.push(SlotSpec {
                key: format!("p{processor}.l{lane}.scale").into(),
                initial: ScalarValue::Float(2.0),
            });
            slots.push(SlotSpec {
                key: format!("p{processor}.l{lane}.output").into(),
                initial: ScalarValue::Float(0.0),
            });
            inputs.push(InputBindingSpec {
                id: InputId(format!("p{processor}.l{lane}").into()),
                slot: base,
            });
            state.push(StateSpec {
                key: StableStateKey(format!("p{processor}.l{lane}.state").into()),
                initial: 0.0,
            });
        }
    }
    let mut operations = Vec::with_capacity(instances * 2);
    for instance in 0..instances {
        let base = (instance * 3) as u32;
        operations.push(OperationSpec::MultiplyFloat {
            left: base,
            right: base + 1,
            output: base + 2,
            batch: 0,
        });
    }
    for instance in (0..instances).rev() {
        let processor = instance / lanes;
        let lane = instance % lanes;
        operations.push(OperationSpec::Emit {
            input: (instance * 3 + 2) as u32,
            sink: EffectSinkId(format!("sink-{processor}-{lane}").into()),
            order: EffectOrder {
                processor: processor as u32,
                lane: lane as u32,
                operation: 0,
            },
            batch: 1,
        });
    }
    GenerationSpec {
        id,
        slots,
        inputs,
        operations,
        state,
        sparse_threshold_percent: 30,
    }
}

fn compiled(processors: usize, lanes: usize) -> Arc<RuntimeGeneration> {
    Arc::new(
        GenerationCompiler
            .compile(workload(processors, lanes, GenerationId(EntityId::new())))
            .unwrap(),
    )
}

#[test]
fn direct_routes_choose_sparse_or_dense_execution_and_commit_effects_deterministically() {
    let generation = compiled(5, 127);
    let first_input = generation.direct_input(&InputId("p0.l0".into())).unwrap();
    let all_inputs = generation.inputs.values().copied().collect::<Vec<_>>();
    let mut runtime = SemanticRuntime::new(Arc::clone(&generation));
    let mut collector = Collector::default();

    let warm = runtime.tick(&[], &mut collector).unwrap();
    assert_eq!(warm.execution_mode, ExecutionMode::Dense);
    collector.effects.clear();

    let sparse = runtime
        .tick(
            &[InputUpdate {
                slot: first_input,
                value: ScalarValue::Float(3.0),
            }],
            &mut collector,
        )
        .unwrap();
    assert_eq!(sparse.execution_mode, ExecutionMode::Sparse);
    assert_eq!(sparse.executed_operations, 2);
    assert_eq!(collector.effects[0].value, ScalarValue::Float(6.0));

    collector.effects.clear();
    let updates = all_inputs
        .into_iter()
        .enumerate()
        .map(|(index, slot)| InputUpdate {
            slot,
            value: ScalarValue::Float(index as f64 + 10.0),
        })
        .collect::<Vec<_>>();
    let dense = runtime.tick(&updates, &mut collector).unwrap();
    assert_eq!(dense.execution_mode, ExecutionMode::Dense);
    assert!(collector.effects.windows(2).all(|pair| pair[0].order < pair[1].order));
    assert_eq!(dense.project_snapshots, 0);
    assert_eq!(dense.topology_traversals, 0);
    assert_eq!(dense.binding_rebuilds, 0);
    assert_eq!(dense.semantic_allocations, 0);
}

#[test]
fn generation_swap_migrates_stable_state_and_rejects_stale_direct_slots() {
    let first = compiled(1, 1);
    let old_input = first.direct_input(&InputId("p0.l0".into())).unwrap();
    let state = StableStateKey("p0.l0.state".into());
    let mut runtime = SemanticRuntime::new(first);
    runtime.set_state(&state, 42.0).unwrap();
    let next = compiled(1, 1);

    let report = runtime.swap_generation(next);

    assert_eq!(report.migrated_state, 1);
    assert_eq!(runtime.state(&state), Some(42.0));
    let error = runtime
        .tick(
            &[InputUpdate {
                slot: old_input,
                value: ScalarValue::Float(1.0),
            }],
            &mut Collector::default(),
        )
        .unwrap_err();
    assert_eq!(error, SemanticRuntimeError::StaleInputSlot);
}

#[test]
fn asynchronous_compilation_keeps_the_previous_valid_generation_running() {
    let initial = compiled(1, 1);
    let initial_id = initial.id;
    let input = initial.direct_input(&InputId("p0.l0".into())).unwrap();
    let mut control = RuntimeControlPlane::new(initial);
    let request = control.request_compilation(GenerationSpec {
        sparse_threshold_percent: 0,
        ..workload(1, 1, GenerationId(EntityId::new()))
    });
    let mut collector = Collector::default();
    let tick = control
        .semantic_mut()
        .tick(
            &[InputUpdate {
                slot: input,
                value: ScalarValue::Float(5.0),
            }],
            &mut collector,
        )
        .unwrap();
    assert_eq!(tick.generation, initial_id);

    let event = wait_for_compilation(&mut control);
    assert_eq!(
        event,
        ControlPlaneEvent::CompilationRejected {
            request,
            error: RuntimeCompileError::InvalidSparseThreshold(0),
        }
    );
    assert_eq!(control.semantic().generation().id, initial_id);
}

#[test]
fn a_valid_background_generation_activates_only_at_the_control_boundary() {
    let initial = compiled(1, 1);
    let initial_id = initial.id;
    let mut control = RuntimeControlPlane::new(initial);
    let next_id = GenerationId(EntityId::new());
    let request = control.request_compilation(workload(2, 1, next_id));

    assert_eq!(control.semantic().generation().id, initial_id);
    let event = wait_for_compilation(&mut control);
    assert!(matches!(
        event,
        ControlPlaneEvent::GenerationActivated {
            request: activated,
            swap: GenerationSwapReport { current, .. },
        } if activated == request && current == next_id
    ));
    assert_eq!(control.semantic().generation().id, next_id);
}

fn wait_for_compilation(control: &mut RuntimeControlPlane) -> ControlPlaneEvent {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(event) = control.poll_compilation() {
            return event;
        }
        assert!(Instant::now() < deadline, "compilation worker timed out");
        thread::yield_now();
    }
}

#[test]
fn canonical_p50_l1_and_p5_l127_value_ticks_pass_debug_and_release_gates() {
    for (processors, lanes) in [(50, 1), (5, 127)] {
        let generation = compiled(processors, lanes);
        let updates = generation
            .inputs
            .values()
            .copied()
            .enumerate()
            .map(|(index, slot)| InputUpdate {
                slot,
                value: ScalarValue::Float(index as f64 + 1.0),
            })
            .collect::<Vec<_>>();
        let mut runtime = SemanticRuntime::new(generation);
        let mut collector = Collector::default();
        runtime.tick(&[], &mut collector).unwrap();
        collector.effects.clear();

        let started = Instant::now();
        let metrics = runtime.tick(&updates, &mut collector).unwrap();
        let elapsed = started.elapsed();
        let gate = if cfg!(debug_assertions) {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(100)
        };
        assert!(elapsed < gate, "P{processors}-L{lanes} took {elapsed:?}");
        assert_eq!(metrics.project_snapshots, 0);
        assert_eq!(metrics.topology_traversals, 0);
        assert_eq!(metrics.binding_rebuilds, 0);
        assert_eq!(metrics.semantic_allocations, 0);
    }
}

#[test]
fn one_hundred_thousand_values_pass_dense_sparse_and_idle_release_gates() {
    let generation = compiled(100_000, 1);
    let inputs = generation.inputs.values().copied().collect::<Vec<_>>();
    let mut runtime = SemanticRuntime::new(generation);
    let mut collector = Collector::default();

    runtime.tick(&[], &mut collector).unwrap();
    collector.effects.clear();

    let idle_started = Instant::now();
    let idle = runtime.tick(&[], &mut collector).unwrap();
    let idle_elapsed = idle_started.elapsed();
    assert_eq!(idle.execution_mode, ExecutionMode::Idle);
    assert_eq!(idle.executed_operations, 0);
    assert!(collector.effects.is_empty());

    let sparse_started = Instant::now();
    let sparse = runtime
        .tick(
            &[InputUpdate {
                slot: inputs[0],
                value: ScalarValue::Float(3.0),
            }],
            &mut collector,
        )
        .unwrap();
    let sparse_elapsed = sparse_started.elapsed();
    assert_eq!(sparse.execution_mode, ExecutionMode::Sparse);
    assert_eq!(sparse.executed_operations, 2);
    assert_eq!(collector.effects.len(), 1);
    collector.effects.clear();

    let updates = inputs
        .into_iter()
        .enumerate()
        .map(|(index, slot)| InputUpdate {
            slot,
            value: ScalarValue::Float(index as f64 + 10.0),
        })
        .collect::<Vec<_>>();
    let dense_started = Instant::now();
    let dense = runtime.tick(&updates, &mut collector).unwrap();
    let dense_elapsed = dense_started.elapsed();
    assert_eq!(dense.execution_mode, ExecutionMode::Dense);
    assert_eq!(dense.executed_operations, 200_000);
    assert_eq!(collector.effects.len(), 100_000);
    assert_eq!(dense.project_snapshots, 0);
    assert_eq!(dense.topology_traversals, 0);
    assert_eq!(dense.binding_rebuilds, 0);
    assert_eq!(dense.semantic_allocations, 0);

    let (idle_gate, sparse_gate, dense_gate) = if cfg!(debug_assertions) {
        (
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(10),
        )
    } else {
        (
            Duration::from_millis(50),
            Duration::from_millis(100),
            Duration::from_secs(2),
        )
    };
    assert!(idle_elapsed < idle_gate, "100k idle tick took {idle_elapsed:?}");
    assert!(sparse_elapsed < sparse_gate, "100k sparse tick took {sparse_elapsed:?}");
    assert!(dense_elapsed < dense_gate, "100k dense tick took {dense_elapsed:?}");
    eprintln!("100k qualification: idle={idle_elapsed:?} sparse={sparse_elapsed:?} dense={dense_elapsed:?}");
}

#[test]
fn canonical_values_cross_once_into_dense_scalar_representation() {
    assert_eq!(
        ScalarValue::try_from(&Value::Integer(7)).unwrap(),
        ScalarValue::Integer(7)
    );
    assert_eq!(
        ScalarValue::try_from(&Value::String("not dense".into())).unwrap_err(),
        ScalarConversionError::Unsupported("string")
    );
}

#[test]
fn compiler_rejects_operations_authored_before_their_dependencies() {
    let spec = GenerationSpec {
        id: GenerationId(EntityId::new()),
        slots: vec![
            SlotSpec {
                key: "input".into(),
                initial: ScalarValue::Float(0.0),
            },
            SlotSpec {
                key: "intermediate".into(),
                initial: ScalarValue::Float(0.0),
            },
            SlotSpec {
                key: "output".into(),
                initial: ScalarValue::Float(0.0),
            },
        ],
        inputs: Vec::new(),
        operations: vec![
            OperationSpec::Copy {
                input: 1,
                output: 2,
                batch: 0,
            },
            OperationSpec::Copy {
                input: 0,
                output: 1,
                batch: 0,
            },
        ],
        state: Vec::new(),
        sparse_threshold_percent: 30,
    };
    assert_eq!(
        GenerationCompiler.compile(spec).unwrap_err(),
        RuntimeCompileError::OperationOrder
    );
}
