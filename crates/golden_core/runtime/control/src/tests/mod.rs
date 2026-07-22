use std::convert::Infallible;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use golden_values::Value;

use super::*;

fn generation(id: u64, revision: u64, bindings: Vec<(&str, usize, Value)>) -> RuntimeGeneration {
    let arenas = ArenaLayout {
        inputs: 2,
        states: bindings.len(),
        values: 4,
        effects: 2,
    };
    RuntimeGenerationBuilder {
        id: RuntimeGenerationId(id),
        project_revision: ProjectRevision(revision),
        statecharts: Vec::new(),
        processor_kernels: vec![CompiledProcessorKernel {
            id: KernelId(0),
            stable_key: "test-kernel".into(),
            inputs_per_lane: 1,
            outputs_per_lane: 1,
            state_per_lane: 1,
        }],
        processor_instances: vec![ProcessorInstanceLayout {
            id: ProcessorInstanceId(0),
            kernel: KernelId(0),
            first_lane: LaneIndex(0),
            lane_count: 1,
            input_base: ValueSlot(0),
            state_base: StateSlot(0),
            output_base: ValueSlot(2),
            effect_base: EffectSlot(0),
        }],
        contexts: CompiledContextCatalog {
            lane_count: 1,
            lane_keys: Arc::from([Arc::from("main")]),
        },
        input_routes: InputRoutingTable::new(
            arenas.inputs,
            vec![InputRoute {
                input: InputSlot(0),
                target: ValueSlot(0),
                dependent: WorkUnitId(0),
            }],
        )
        .unwrap(),
        schedule: RuntimeSchedule::new(
            vec![ScheduledWork {
                id: WorkUnitId(0),
                kernel: KernelId(0),
                first_lane: 0,
                lane_count: 1,
            }],
            0.5,
        )
        .unwrap(),
        effects: EffectRoutingTable::new(
            vec![
                EffectRoute {
                    slot: EffectSlot(1),
                    state_order: 0,
                    processor_order: 0,
                    lane_order: 0,
                    effect_order: 0,
                },
                EffectRoute {
                    slot: EffectSlot(0),
                    state_order: 1,
                    processor_order: 0,
                    lane_order: 0,
                    effect_order: 0,
                },
            ],
            arenas.effects,
        )
        .unwrap(),
        observation: ObservationCatalog::default(),
        arenas,
        state_bindings: bindings
            .into_iter()
            .map(|(key, slot, default)| StableStateBinding {
                key: StableStateKey::new(key),
                slot: StateSlot(slot as u32),
                default,
            })
            .collect(),
    }
    .build()
    .unwrap()
}

#[test]
fn control_actor_owns_state_and_reports_acknowledgement_lifecycle() {
    let caller_thread = thread::current().id();
    let actor = ControlActor::spawn("test-control", 3_u32).unwrap();
    let pending = actor
        .handle()
        .submit(move |value| {
            assert_ne!(thread::current().id(), caller_thread);
            *value += 4;
            *value
        })
        .unwrap();

    assert_eq!(pending.status(), ControlStatus::Accepted);
    let receipt = pending.wait().unwrap();
    assert_eq!(receipt.status, ControlStatus::Applied);
    assert_eq!(receipt.output, 7);
    assert_eq!(actor.metrics().snapshot().control_applied, 1);
}

#[test]
fn generation_swap_migrates_only_compatible_stable_state() {
    let first = Arc::new(generation(
        1,
        10,
        vec![("processor/a/lane/0", 0, Value::Int(1)), ("removed", 1, Value::Int(2))],
    ));
    let mut runtime = SemanticRuntime::new(first);
    *runtime.arenas_mut().state_mut(StateSlot(0)).unwrap() = Value::Int(42);
    let next = Arc::new(generation(
        2,
        11,
        vec![("added", 0, Value::Int(7)), ("processor/a/lane/0", 1, Value::Int(0))],
    ));

    let report = runtime.swap_generation(next);

    assert_eq!(report.migrated_states, 1);
    assert_eq!(report.initialized_states, 1);
    assert_eq!(runtime.arenas().state(StateSlot(0)), Some(&Value::Int(7)));
    assert_eq!(runtime.arenas().state(StateSlot(1)), Some(&Value::Int(42)));
    assert_eq!(runtime.current_generation().id, RuntimeGenerationId(2));
}

#[test]
fn scheduler_switches_sparse_and_dense_without_completion_order_sorting() {
    let units = (0..8)
        .map(|index| ScheduledWork {
            id: WorkUnitId(index),
            kernel: KernelId(0),
            first_lane: index,
            lane_count: 1,
        })
        .collect();
    let schedule = RuntimeSchedule::new(units, 0.5).unwrap();
    let metrics = Arc::new(RuntimeMetrics::default());
    let scheduler = PersistentBatchScheduler::new(
        3,
        |work: ScheduledWork| {
            thread::sleep(Duration::from_millis((8 - work.id.0) as u64));
            work.id.0 * 10
        },
        metrics.clone(),
    )
    .unwrap();
    let mut dirty = DirtySet::new(8);
    dirty.mark(WorkUnitId(1)).unwrap();
    dirty.mark(WorkUnitId(6)).unwrap();
    let mut outputs = Vec::with_capacity(8);
    let output_capacity = outputs.capacity();

    let sparse = scheduler.execute_into(&schedule, &dirty, &mut outputs).unwrap();
    assert_eq!(sparse, ExecutionMode::Sparse);
    assert_eq!(outputs, vec![(WorkUnitId(1), 10), (WorkUnitId(6), 60)]);
    assert_eq!(outputs.capacity(), output_capacity);

    dirty.mark_all();
    let dense = scheduler.execute_into(&schedule, &dirty, &mut outputs).unwrap();
    assert_eq!(dense, ExecutionMode::Dense);
    assert_eq!(outputs.first(), Some(&(WorkUnitId(0), 0)));
    assert_eq!(outputs.last(), Some(&(WorkUnitId(7), 70)));
    assert_eq!(outputs.capacity(), output_capacity);
    assert_eq!(metrics.snapshot().work_units, 10);
}

#[test]
fn deterministic_effect_routes_suppress_every_shadow_effect() {
    let runtime_generation = generation(1, 1, vec![("state", 0, Value::Unit)]);
    let metrics = Arc::new(RuntimeMetrics::default());
    let mut effects = EffectBuffer::new(2, metrics.clone());
    effects.stage(EffectSlot(0), "later").unwrap();
    effects.stage(EffectSlot(1), "first").unwrap();
    let mut dispatched = Vec::new();
    let report = effects
        .commit(
            &runtime_generation.effects,
            EffectCommitMode::Authoritative,
            &mut |value| -> Result<(), Infallible> {
                dispatched.push(value);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(dispatched, vec!["first", "later"]);
    assert_eq!(report.committed, 2);

    effects.stage(EffectSlot(0), "hidden").unwrap();
    let report = effects
        .commit(
            &runtime_generation.effects,
            EffectCommitMode::ShadowSuppressed,
            &mut |_value| -> Result<(), Infallible> { panic!("shadow mode must not dispatch") },
        )
        .unwrap();
    assert_eq!(report.suppressed, 1);
    assert_eq!(metrics.snapshot().effects_suppressed, 1);
}

#[test]
fn module_input_mailbox_coalesces_values_and_preserves_lossless_updates() {
    let runtime_generation = generation(1, 1, vec![("state", 0, Value::Unit)]);
    let mut arenas = RuntimeArenas::for_generation(&runtime_generation);
    let mut dirty = DirtySet::new(runtime_generation.schedule.work_count());
    let (mailbox, handle) = RuntimeInputMailbox::new(InputIngressConfig {
        input_count: runtime_generation.arenas.inputs,
        lossless_capacity: 4,
    })
    .unwrap();
    handle
        .publish(RuntimeInputUpdate {
            slot: InputSlot(0),
            value: Value::Int(1),
            source_time_ns: 10,
            revision: 1,
            delivery: InputDelivery::LatestValue,
        })
        .unwrap();
    handle
        .publish(RuntimeInputUpdate {
            slot: InputSlot(0),
            value: Value::Int(2),
            source_time_ns: 20,
            revision: 2,
            delivery: InputDelivery::LatestValue,
        })
        .unwrap();
    handle
        .publish(RuntimeInputUpdate {
            slot: InputSlot(1),
            value: Value::Trigger(golden_values::TriggerValue::fired(3, 1)),
            source_time_ns: 15,
            revision: 3,
            delivery: InputDelivery::LosslessOrdered,
        })
        .unwrap();
    let mut scratch = Vec::with_capacity(4);

    let applied = mailbox
        .drain_into(&mut arenas, &runtime_generation.input_routes, &mut dirty, &mut scratch)
        .unwrap();

    assert_eq!(applied, 2);
    assert_eq!(arenas.input(InputSlot(0)), Some(&Value::Int(2)));
    assert!(dirty.contains(WorkUnitId(0)));
}

struct TestCompiler;

impl GenerationCompiler<u64> for TestCompiler {
    type Error = Infallible;

    fn compile(
        &self,
        generation_id: RuntimeGenerationId,
        request: CompileRequest<u64>,
    ) -> Result<RuntimeGeneration, Self::Error> {
        assert!(request.changes.affects("processors"));
        assert_eq!(*request.project, 99);
        Ok(generation(
            generation_id.0,
            request.revision.0,
            vec![("state", 0, Value::Unit)],
        ))
    }
}

#[test]
fn asynchronous_compilation_keeps_previous_generation_available() {
    let metrics = Arc::new(RuntimeMetrics::default());
    let service = CompilationService::spawn(TestCompiler, 7, metrics.clone()).unwrap();
    let previous = Arc::new(generation(6, 20, vec![("state", 0, Value::Int(8))]));
    let mut changes = RuntimeChangeSet::new();
    changes.mark("processors");
    let ticket = service
        .handle()
        .request(CompileRequest {
            project: Arc::new(99),
            revision: ProjectRevision(21),
            changes,
            previous: Some(previous.clone()),
        })
        .unwrap();

    assert_eq!(previous.id, RuntimeGenerationId(6));
    let completion = service.complete().unwrap();
    assert_eq!(completion.ticket, ticket);
    assert_eq!(completion.result.unwrap().id, RuntimeGenerationId(7));
    assert_eq!(metrics.snapshot().compilation_applied, 1);
}
