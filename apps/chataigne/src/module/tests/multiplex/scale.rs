use super::*;

use chataigne_alchemist::{AxisSet, ContextAxisId, ContextKey, ContextValuePath, EvaluationCtx, RuntimeRegistries};
use chataigne_state_machine::{
    Processor, ProcessorBindingAnalysis, ProcessorContextProvider, ProcessorDebugCapture, ProcessorId,
    ProcessorLifecycleEvent, ProcessorRuntime, statechart::StateId,
};
use golden_values::Value as RuntimeValue;
use sysinfo::{ProcessesToUpdate, System, get_current_pid};

use crate::app::systems_state_machine_manager::profiling::RuntimeScaleFixture;

mod workers;
mod idle;

struct ScaleContextProvider {
    keys: Vec<ContextKey>,
    axes: AxisSet,
}

impl ScaleContextProvider {
    fn new(lanes: usize) -> Self {
        let axis = ContextAxisId::new("scale_lane");
        let mut axes = AxisSet::new();
        axes.insert(axis.clone());
        Self {
            keys: (0..lanes)
                .map(|index| ContextKey::single(axis.clone(), format!("lane-{index}")))
                .collect(),
            axes,
        }
    }
}

impl ProcessorContextProvider for ScaleContextProvider {
    fn available_axes(&self, _processor_id: ProcessorId) -> AxisSet {
        self.axes.clone()
    }

    fn iter_context_keys<'a>(
        &'a self,
        _processor_id: ProcessorId,
        _axes: &'a AxisSet,
    ) -> Box<dyn Iterator<Item = ContextKey> + 'a> {
        Box::new(self.keys.iter().cloned())
    }

    fn resolve_context_value(
        &self,
        _key: &ContextKey,
        _axis: &ContextAxisId,
        _path: &ContextValuePath,
    ) -> Option<RuntimeValue> {
        None
    }
}

fn loaded_multiplex_scale_engine() -> crate::app::AppEngine {
    let path = targeted_performance_sample_path("test_multiplex.noisette");
    let mut engine = load_sparse_project_file::<AppNode, _>(&path).expect("multiplex sample should load");
    configure_loaded_engine(&mut engine).expect("multiplex sample should configure");
    prepare_engine_for_runtime(&mut engine).expect("multiplex sample should prepare");
    for _ in 0..10 {
        engine.run_tick(Duration::from_millis(8)).expect("warmup tick should run");
    }
    let processor_count = engine
        .nodes
        .iter()
        .filter(|(_, node)| node.get_type() == "state_processor")
        .count();
    let source = multiplex_condition_source(&engine, processor_count);
    let manager_id = state_machine_manager_id(&engine);
    let Some(AppNode::StateMachineManager(manager)) = engine.nodes.get_mut(manager_id) else {
        panic!("app should contain a state-machine manager");
    };
    manager.enable_runtime_scale_input_capture();
    let capture_ticks = measure_multiplex_source_ticks(&mut engine, source, 8, None);
    assert_eq!(capture_ticks.ticks_with_callbacks, 8);
    engine
}

fn resident_bytes(system: &mut System) -> u64 {
    let pid = get_current_pid().expect("current PID should exist");
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system.process(pid).expect("current process should exist").memory()
}

fn process_cpu_millis(system: &mut System) -> u64 {
    let pid = get_current_pid().expect("current PID should exist");
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
    system
        .process(pid)
        .expect("current process should exist")
        .accumulated_cpu_time()
}

fn sample_fixture() -> RuntimeScaleFixture {
    let engine = loaded_multiplex_scale_engine();
    let fixtures = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::StateMachineManager(manager) => Some(manager.runtime_scale_fixtures()),
            _ => None,
        })
        .expect("app should contain a state-machine manager");
    assert_eq!(fixtures.len(), 8, "all eight sample processors must capture runtime inputs");
    let fixture = fixtures
        .into_iter()
        .find(|fixture| fixture.compiled.analysis.has_stateful_nodes && !fixture.managed)
        .expect("the sample should compile a non-managed stateful product formula");
    assert_eq!(fixture.compiled.graph.exec_nodes.len(), 3);
    fixture
}

fn build_processors(
    fixture: &RuntimeScaleFixture,
    provider: &ScaleContextProvider,
    processor_count: usize,
) -> Vec<(Processor, ProcessorRuntime)> {
    (0..processor_count)
        .map(|_| {
            let mut processor = fixture.processor.clone();
            processor.id = ProcessorId::new();
            processor.condition = None;
            let mut runtime = ProcessorRuntime::new(processor.id);
            assert!(runtime.compile_from_shared_formula(
                &processor,
                &fixture.formula,
                fixture.compiled.clone(),
            ));
            runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::ProjectStart);
            runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::StateEnter(StateId::new()));
            runtime.apply_lifecycle(&processor, ProcessorLifecycleEvent::ProcessorEnable);
            runtime.rebuild_execution_plan(
                provider,
                &ProcessorBindingAnalysis {
                    input_axes: provider.available_axes(processor.id),
                    ..ProcessorBindingAnalysis::default()
                },
            );
            (processor, runtime)
        })
        .collect::<Vec<_>>()
}

fn profile_partition(processor_count: usize, lanes_per_processor: usize) {
    let _performance_guard = lock_performance_test();
    assert_eq!(processor_count * lanes_per_processor, 100_000);
    let fixture = sample_fixture();
    let provider = ScaleContextProvider::new(lanes_per_processor);
    let mut system = System::new();
    let rss_before_build = resident_bytes(&mut system);
    let build_started = Instant::now();
    let mut processors = build_processors(&fixture, &provider, processor_count);
    let build_ms = build_started.elapsed().as_millis();
    let rss_after_build = resident_bytes(&mut system);

    let registries = RuntimeRegistries {
        value_types: chataigne_state_machine::alchemist::shared_value_type_registry(),
    };
    let mut tick_us = Vec::new();
    let mut kernel_ns = 0;
    let mut kernel_evaluations = 0;
    let mut total_intents = 0;
    let mut total_diagnostics = 0;
    for tick in 1..=4 {
        let ctx = EvaluationCtx {
            logical_tick: tick,
            delta_time: Duration::from_millis(8),
            events: &[],
            inputs: &fixture.inputs,
            registries: &registries,
        };
        let kernel_before = chataigne_state_machine::processor_kernel_profile_snapshot();
        let started = Instant::now();
        let mut lane_count = 0;
        for (processor, runtime) in &mut processors {
            let lanes = runtime.evaluate_processor_with_context_provider_and_capture(
                processor,
                &ctx,
                &provider,
                &ProcessorDebugCapture::Off,
            );
            lane_count += lanes.len();
            total_intents += lanes.iter().map(|lane| lane.output.intents.len()).sum::<usize>();
            total_diagnostics += lanes.iter().map(|lane| lane.output.diagnostics.len()).sum::<usize>();
        }
        let elapsed_us = started.elapsed().as_micros();
        let kernel_after = chataigne_state_machine::processor_kernel_profile_snapshot();
        assert_eq!(lane_count, 100_000, "all scale lanes must be evaluated");
        if tick > 1 {
            tick_us.push(elapsed_us);
            kernel_ns += kernel_after.elapsed_ns - kernel_before.elapsed_ns;
            kernel_evaluations += kernel_after.evaluations - kernel_before.evaluations;
        }
    }
    assert_eq!(kernel_evaluations, 300_000);
    let memory_count = processors
        .iter()
        .map(|(_, runtime)| runtime.lanes.memory_count())
        .sum::<usize>();
    assert_eq!(memory_count, 100_000, "stateful lanes must retain lane-private memory");
    tick_us.sort_unstable();
    let rss_after_evaluation = resident_bytes(&mut system);
    eprintln!(
        "formula scale: processors={processor_count} lanes_per_processor={lanes_per_processor} \
         formula={} exec_nodes={} build_ms={build_ms} tick_us={tick_us:?} kernel_ms={} \
         kernel_evaluations={kernel_evaluations} intents={total_intents} diagnostics={total_diagnostics} \
         rss_before_mb={} rss_built_mb={} rss_evaluated_mb={}",
        fixture.formula.label,
        fixture.compiled.graph.exec_nodes.len(),
        kernel_ns / 1_000_000,
        rss_before_build / 1_000_000,
        rss_after_build / 1_000_000,
        rss_after_evaluation / 1_000_000,
    );
}

#[test]
#[ignore = "manual T18 product-formula scale qualification"]
fn multiplex_formula_scale_1000_by_100() {
    profile_partition(1_000, 100);
}

#[test]
#[ignore = "manual T18 product-formula scale qualification"]
fn multiplex_formula_scale_10000_by_10() {
    profile_partition(10_000, 10);
}
