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
fn control_actor_rejects_overload_and_recovers_after_capacity_drains() {
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let actor = ControlActor::spawn_with_config(
        "bounded-test-control",
        0_u32,
        ControlActorConfig {
            pending_capacity: std::num::NonZeroUsize::MIN,
        },
    )
    .unwrap();

    let task_entered = entered.clone();
    let task_release = release.clone();
    let first = actor
        .handle()
        .submit(move |value| {
            task_entered.wait();
            task_release.wait();
            *value += 1;
            *value
        })
        .unwrap();
    entered.wait();
    let second = actor.handle().submit(|value| {
        *value += 1;
        *value
    });
    let error = actor.handle().submit(|value| {
        *value += 100;
        *value
    });
    let Err(error) = error else {
        panic!("the third operation must not block or exceed capacity");
    };
    assert_eq!(error.kind(), ControlErrorKind::Overloaded);
    assert_eq!(actor.metrics().snapshot().control_queue_depth, 1);

    release.wait();
    assert_eq!(first.wait().unwrap().output, 1);
    assert_eq!(second.unwrap().wait().unwrap().output, 2);

    let recovered = actor.call(|value| {
        *value += 1;
        *value
    });
    assert_eq!(recovered.unwrap().output, 3);
    let metrics = actor.metrics().snapshot();
    assert_eq!(metrics.control_rejected, 1);
    assert_eq!(metrics.control_queue_depth, 0);
}

#[test]
fn control_actor_shutdown_is_not_stranded_behind_a_full_queue() {
    let entered = Arc::new(std::sync::Barrier::new(2));
    let release = Arc::new(std::sync::Barrier::new(2));
    let actor = ControlActor::spawn_with_config(
        "bounded-shutdown-control",
        (),
        ControlActorConfig {
            pending_capacity: std::num::NonZeroUsize::MIN,
        },
    )
    .unwrap();
    let task_entered = entered.clone();
    let task_release = release.clone();
    let first = actor
        .handle()
        .submit(move |_| {
            task_entered.wait();
            task_release.wait();
        })
        .unwrap();
    entered.wait();
    let queued = actor.handle().submit(|_| 7_u8).unwrap();
    let probe = actor.handle();

    let (dropped_tx, dropped_rx) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        drop(actor);
        let _ = dropped_tx.send(());
    });
    loop {
        if let Err(error) = probe.submit(|_| ())
            && error.kind() == ControlErrorKind::Disconnected
        {
            break;
        }
        thread::yield_now();
    }
    release.wait();

    first.wait().unwrap();
    assert!(queued.wait().is_err(), "queued work is rejected during shutdown");
    dropped_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("actor shutdown completes after the active task cooperates");
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
        _context: &CompilationContext,
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
    let admission = service
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
    assert_eq!(completion.ticket, admission.ticket);
    assert_eq!(completion.result.unwrap().id, RuntimeGenerationId(7));
    assert_eq!(metrics.snapshot().compilation_applied, 1);
}

struct BlockingCompiler {
    first_started: Arc<std::sync::Barrier>,
    release_first: Arc<std::sync::Barrier>,
    compiled_revisions: Arc<std::sync::Mutex<Vec<u64>>>,
}

impl GenerationCompiler<u64> for BlockingCompiler {
    type Error = String;

    fn compile(
        &self,
        generation_id: RuntimeGenerationId,
        request: CompileRequest<u64>,
        context: &CompilationContext,
    ) -> Result<RuntimeGeneration, Self::Error> {
        self.compiled_revisions.lock().unwrap().push(request.revision.0);
        if request.revision == ProjectRevision(1) {
            self.first_started.wait();
            self.release_first.wait();
        }
        if context.is_stale() {
            return Err("stale".to_string());
        }
        Ok(generation(
            generation_id.0,
            request.revision.0,
            vec![("state", 0, Value::Unit)],
        ))
    }
}

fn compile_request(revision: u64) -> CompileRequest<u64> {
    CompileRequest {
        project: Arc::new(revision),
        revision: ProjectRevision(revision),
        changes: RuntimeChangeSet::new(),
        previous: None,
    }
}

#[test]
fn compiler_keeps_only_in_flight_and_latest_pending_generation() {
    let first_started = Arc::new(std::sync::Barrier::new(2));
    let release_first = Arc::new(std::sync::Barrier::new(2));
    let compiled_revisions = Arc::new(std::sync::Mutex::new(Vec::new()));
    let metrics = Arc::new(RuntimeMetrics::default());
    let service = CompilationService::spawn(
        BlockingCompiler {
            first_started: first_started.clone(),
            release_first: release_first.clone(),
            compiled_revisions: compiled_revisions.clone(),
        },
        1,
        metrics.clone(),
    )
    .unwrap();
    let handle = service.handle();

    let first = handle.request(compile_request(1)).unwrap();
    first_started.wait();
    let second = handle.request(compile_request(2)).unwrap();
    let third = handle.request(compile_request(3)).unwrap();
    assert_eq!(third.superseded_ticket, Some(second.ticket));
    assert_eq!(metrics.snapshot().compilation_pending_depth, 1);
    release_first.wait();

    let stale = service.complete().unwrap();
    assert_eq!(stale.ticket, first.ticket);
    assert!(matches!(
        stale.result,
        Err(CompilationError::Superseded { by_ticket }) if by_ticket == third.ticket
    ));
    let latest = service.complete().unwrap();
    assert_eq!(latest.ticket, third.ticket);
    assert_eq!(latest.result.unwrap().project_revision, ProjectRevision(3));
    assert_eq!(*compiled_revisions.lock().unwrap(), vec![1, 3]);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.compilation_requested, 3);
    assert_eq!(snapshot.compilation_applied, 1);
    assert_eq!(snapshot.compilation_rejected, 0);
    assert_eq!(snapshot.compilation_superseded, 2);
    assert_eq!(snapshot.compilation_pending_peak, 1);
    assert_eq!(snapshot.compilation_pending_depth, 0);
}
