use super::*;
use crate::engine::NodeExecutionRule;
use crate::node::{NodeData, NodeUuid};
use crate::process_ctx::ProcessCtx;

struct ScheduleProbe {
    data: NodeData,
    rule: Arc<Mutex<NodeExecutionRule>>,
    effects: Arc<Mutex<Vec<String>>>,
}

impl ScheduleProbe {
    fn new(label: &str, identity: u128, effects: &Arc<Mutex<Vec<String>>>) -> Self {
        let mut data = NodeData::new(label.into());
        data.meta.uuid = NodeUuid(uuid::Uuid::from_u128(identity));
        Self {
            data,
            rule: Arc::new(Mutex::new(NodeExecutionRule::passive())),
            effects: effects.clone(),
        }
    }
}

impl Node for ScheduleProbe {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "production_schedule_probe"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.lock().unwrap().clone()
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
    fn inbox_requires_tree_snapshot(&self, _events: &crate::events::EventFrame) -> bool {
        false
    }
    fn update(&mut self, _ctx: &mut ProcessCtx) {
        self.effects.lock().unwrap().push(self.data.meta.label.clone());
    }
}

fn periodic(dependencies: impl IntoIterator<Item = NodeId>) -> NodeExecutionRule {
    NodeExecutionRule::periodic(100)
        .with_dependencies(dependencies)
        .with_compiled_kernel("test.schedule-probe")
}

fn fixture(second_active: bool) -> (ProductionState<ScheduleProbe>, NodeId, NodeId, Arc<Mutex<Vec<String>>>) {
    let effects = Arc::new(Mutex::new(Vec::new()));
    let mut engine = Engine::new(ScheduleProbe::new("root", 1, &effects));
    engine.add_node(ScheduleProbe::new("a", 10, &effects), None);
    engine.add_node(ScheduleProbe::new("b", 20, &effects), None);
    engine.apply_edits().unwrap();
    let a = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let b = engine.nodes.get(a).unwrap().node_data().next_sibling.unwrap();
    *engine.nodes.get(a).unwrap().rule.lock().unwrap() = periodic([]);
    if second_active {
        *engine.nodes.get(b).unwrap().rule.lock().unwrap() = periodic([a]);
    }
    let (state, _) = ProductionState::new(engine, Arc::new(RuntimeMetrics::default()), false).unwrap();
    (state, a, b, effects)
}

fn reverse_dependency(state: &mut ProductionState<ScheduleProbe>, a: NodeId, b: NodeId, apply_now: bool) {
    *state.engine.nodes.get(a).unwrap().rule.lock().unwrap() = periodic([b]);
    *state.engine.nodes.get(b).unwrap().rule.lock().unwrap() = periodic([]);
    state.engine.request_graph_reevaluation();
    if apply_now {
        state.engine.apply_edits().unwrap();
    }
}

#[test]
fn dependency_only_reorder_uses_the_current_resolved_order_in_production() {
    for apply_now in [false, true] {
        let (mut state, a, b, effects) = fixture(true);
        reverse_dependency(&mut state, a, b, apply_now);
        state.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(*effects.lock().unwrap(), ["b", "a"]);
        assert_eq!(
            state.pending_compilations.len(),
            1,
            "dependency-only changes must compile"
        );
        let completion = state.compiler.complete().unwrap();
        state.apply_completion(completion);
        assert_eq!(state.compiled_schedule_revision, state.engine.schedule_revision());
        effects.lock().unwrap().clear();
        state.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(*effects.lock().unwrap(), ["b", "a"]);
        assert!(state.pending_compilations.is_empty());
    }
}

#[test]
fn newly_scheduled_prerequisite_is_never_appended_after_its_dependent() {
    for apply_now in [false, true] {
        let (mut state, a, b, effects) = fixture(false);
        reverse_dependency(&mut state, a, b, apply_now);
        state.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(*effects.lock().unwrap(), ["b", "a"]);
    }
}

#[test]
fn completions_for_invalidated_or_replaced_schedule_revisions_are_rejected() {
    for resolve_now in [false, true] {
        let (mut state, a, b, effects) = fixture(true);
        state.request_compilation("test.old-schedule");
        let completion = state.compiler.complete().unwrap();
        let generation = state.semantic.current_generation().id;
        reverse_dependency(&mut state, a, b, true);
        if resolve_now {
            state.engine.resolve().unwrap();
        }
        state.apply_completion(completion);
        assert_eq!(state.semantic.current_generation().id, generation);
        state.run_tick(Duration::from_millis(10)).unwrap();
        assert_eq!(*effects.lock().unwrap(), ["b", "a"]);
        let current = state.compiler.complete().unwrap();
        state.apply_completion(current);
        assert_eq!(state.compiled_schedule_revision, state.engine.schedule_revision());
    }
}

#[test]
fn inserting_a_prerequisite_during_compilation_preserves_current_dependency_order() {
    let (mut state, a, _, effects) = fixture(false);
    state.request_compilation("test.before-insertion");
    let old_completion = state.compiler.complete().unwrap();
    let prerequisite = ScheduleProbe::new("new prerequisite", 30, &effects);
    *prerequisite.rule.lock().unwrap() = periodic([]);
    state.engine.add_node(prerequisite, None);
    state.engine.apply_edits().unwrap();
    let added = state
        .engine
        .nodes
        .get(state.engine.root)
        .unwrap()
        .node_data()
        .last_child
        .unwrap();
    *state.engine.nodes.get(a).unwrap().rule.lock().unwrap() = periodic([added]);
    state.engine.request_graph_reevaluation();
    state.engine.apply_edits().unwrap();
    state.apply_completion(old_completion);
    state.run_tick(Duration::from_millis(10)).unwrap();
    assert_eq!(*effects.lock().unwrap(), ["new prerequisite", "a"]);
    let completion = state.compiler.complete().unwrap();
    state.apply_completion(completion);
    effects.lock().unwrap().clear();
    state.run_tick(Duration::from_millis(10)).unwrap();
    assert_eq!(*effects.lock().unwrap(), ["new prerequisite", "a"]);
}

#[test]
fn project_epoch_fences_completions_even_when_request_and_schedule_revisions_coincide() {
    let (mut state, _, _, _) = fixture(true);
    state.request_compilation("test.old-project");
    let old_completion = state.compiler.complete().unwrap();
    let (mut candidate, _, _, _) = fixture(true);
    let compiled = state.compile_project_candidate(&mut candidate.engine).unwrap();
    state
        .commit_project(candidate.engine, compiled, ProjectGeneration::new(2), false, |_| ())
        .unwrap_or_else(|rejected| panic!("{}", rejected.error));
    state.request_compilation("test.new-project");
    assert_eq!(old_completion.revision, state.compilation_revision);
    let generation = state.semantic.current_generation().id;
    state.apply_completion(old_completion);
    assert_eq!(state.semantic.current_generation().id, generation);
    let completion = state.compiler.complete().unwrap();
    state.apply_completion(completion);
    assert_eq!(state.compiled_schedule_revision, state.engine.schedule_revision());
    assert_eq!(state.project_generation, ProjectGeneration::new(2));
}

#[test]
fn failed_resolution_keeps_the_previous_schedule_and_revision_usable() {
    let (mut state, a, _, _) = fixture(true);
    let revision = state.engine.schedule_revision();
    let topology = state.engine.schedule_topology().to_vec();
    *state.engine.nodes.get(a).unwrap().rule.lock().unwrap() = NodeExecutionRule::periodic(0);
    state.engine.request_graph_reevaluation();
    state.engine.apply_edits().unwrap();
    assert!(state.engine.resolve().is_err());
    assert_eq!(state.engine.schedule_revision(), revision);
    assert_eq!(state.engine.schedule_topology(), topology);
    *state.engine.nodes.get(a).unwrap().rule.lock().unwrap() = periodic([]);
    state.run_tick(Duration::from_millis(10)).unwrap();
    assert_ne!(state.engine.schedule_revision(), revision);
}

#[test]
fn candidate_changed_after_compilation_is_rejected_before_project_cutover() {
    for resolve_now in [false, true] {
        let (mut state, _, _, _) = fixture(true);
        let (mut candidate, a, b, _) = fixture(true);
        let compiled = state.compile_project_candidate(&mut candidate.engine).unwrap();
        reverse_dependency(&mut candidate, a, b, true);
        if resolve_now {
            candidate.engine.resolve().unwrap();
        }
        let generation = state.semantic.current_generation().id;
        assert!(
            state
                .commit_project(candidate.engine, compiled, ProjectGeneration::new(2), false, |_| ())
                .is_err()
        );
        assert_eq!(state.project_generation, ProjectGeneration::INITIAL);
        assert_eq!(state.semantic.current_generation().id, generation);
        state.run_tick(Duration::from_millis(10)).unwrap();
    }
}
