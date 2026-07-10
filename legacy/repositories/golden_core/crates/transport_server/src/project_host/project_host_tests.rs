use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::{normalize_project_save_path, replace_live_engine, sanitize_browser_upload_file_name};
use golden_engine as golden_core;
use golden_engine::app::{ProjectFileSpec, ProjectLifecycle, prepare_engine_for_runtime};
use golden_engine::define_node_enum;
use golden_engine::edit::Edit;
use golden_engine::engine::Engine;
use golden_engine::node::{Folder, Node, NodeCreationContext, NodeId};
use golden_engine::parameter::{ParamValue, ParameterEventBehaviour};
use golden_engine::process_ctx::ProcessCtx;

static PREVIOUS_ENGINE_DROPPED: AtomicBool = AtomicBool::new(false);
static PREVIOUS_ENGINE_DESTROYED: AtomicBool = AtomicBool::new(false);
static READY_CALLBACK_SAW_DROP: AtomicBool = AtomicBool::new(false);
static READY_CALLBACK_SAW_DESTROY: AtomicBool = AtomicBool::new(false);

#[golden_engine::node("drop_probe_node")]
struct DropProbeNode {}

impl Drop for DropProbeNode {
    fn drop(&mut self) {
        PREVIOUS_ENGINE_DROPPED.store(true, Ordering::SeqCst);
    }
}

#[golden_engine::node("drop_probe_node", from_struct)]
impl Node for DropProbeNode {}

#[golden_engine::node("destroy_probe_node")]
struct DestroyProbeNode {}

#[golden_engine::node("destroy_probe_node", from_struct)]
impl Node for DestroyProbeNode {
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        PREVIOUS_ENGINE_DESTROYED.store(true, Ordering::SeqCst);
    }
}

#[golden_engine::node("ready_probe_node")]
struct ReadyProbeNode {}

#[golden_engine::node("ready_probe_node", from_struct)]
impl Node for ReadyProbeNode {
    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        READY_CALLBACK_SAW_DROP.store(PREVIOUS_ENGINE_DROPPED.load(Ordering::SeqCst), Ordering::SeqCst);
        READY_CALLBACK_SAW_DESTROY.store(PREVIOUS_ENGINE_DESTROYED.load(Ordering::SeqCst), Ordering::SeqCst);
    }
}

#[golden_engine::node("bad_ready_node")]
struct BadReadyNode {}

#[golden_engine::node("bad_ready_node", from_struct)]
impl Node for BadReadyNode {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        ctx.edits.push(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
}

define_node_enum!(
    enum ReplaceOrderTestNode {
        BadReadyNode,
        DropProbeNode,
        DestroyProbeNode,
        ReadyProbeNode,
    }
);

impl ProjectLifecycle for ReplaceOrderTestNode {}

#[test]
fn normalize_project_save_path_replaces_non_matching_extension() {
    let spec = ProjectFileSpec::new("Noisette files", "noisette");
    let normalized = normalize_project_save_path("D:/tmp/demo.json", &spec).expect("path should normalize");
    assert!(
        normalized.ends_with("demo.noisette"),
        "expected save path to end with .noisette, got {normalized}"
    );
}

#[test]
fn normalize_project_save_path_appends_missing_extension() {
    let spec = ProjectFileSpec::new("Noisette files", ".noisette");
    let normalized = normalize_project_save_path("D:/tmp/demo", &spec).expect("path should normalize");
    assert!(
        normalized.ends_with("demo.noisette"),
        "expected save path to append .noisette, got {normalized}"
    );
}

#[test]
fn sanitize_browser_upload_file_name_uses_app_extension() {
    let spec = ProjectFileSpec::new("Noisette files", "noisette");
    assert_eq!(sanitize_browser_upload_file_name("show.json", &spec), "show.noisette");
    assert_eq!(sanitize_browser_upload_file_name("", &spec), "project.noisette");
}

#[test]
fn replace_live_engine_drops_previous_engine_before_node_ready_callbacks() {
    PREVIOUS_ENGINE_DROPPED.store(false, Ordering::SeqCst);
    PREVIOUS_ENGINE_DESTROYED.store(false, Ordering::SeqCst);
    READY_CALLBACK_SAW_DROP.store(false, Ordering::SeqCst);
    READY_CALLBACK_SAW_DESTROY.store(false, Ordering::SeqCst);

    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut live_engine = Engine::new(root);
    live_engine.add_node(DropProbeNode::new().into(), None);
    prepare_engine_for_runtime(&mut live_engine).expect("live engine should prepare");

    let shared_engine = Arc::new(Mutex::new(live_engine));

    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut next_engine = Engine::new(root);
    next_engine.add_node(ReadyProbeNode::new().into(), None);

    replace_live_engine(&shared_engine, next_engine, "test_replace", false).expect("engine replacement should succeed");

    assert!(
        PREVIOUS_ENGINE_DROPPED.load(Ordering::SeqCst),
        "previous engine should be dropped during replacement"
    );
    assert!(
        READY_CALLBACK_SAW_DROP.load(Ordering::SeqCst),
        "node-ready callbacks should only run after the previous engine is dropped"
    );
}

#[test]
fn replace_live_engine_runs_destroy_callbacks_before_node_ready_callbacks() {
    PREVIOUS_ENGINE_DROPPED.store(false, Ordering::SeqCst);
    PREVIOUS_ENGINE_DESTROYED.store(false, Ordering::SeqCst);
    READY_CALLBACK_SAW_DROP.store(false, Ordering::SeqCst);
    READY_CALLBACK_SAW_DESTROY.store(false, Ordering::SeqCst);

    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut live_engine = Engine::new(root);
    live_engine.add_node(DestroyProbeNode::new().into(), None);
    prepare_engine_for_runtime(&mut live_engine).expect("live engine should prepare");

    let shared_engine = Arc::new(Mutex::new(live_engine));

    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut next_engine = Engine::new(root);
    next_engine.add_node(ReadyProbeNode::new().into(), None);

    replace_live_engine(&shared_engine, next_engine, "test_replace", false).expect("engine replacement should succeed");

    assert!(
        PREVIOUS_ENGINE_DESTROYED.load(Ordering::SeqCst),
        "previous engine should run destroy callbacks during replacement"
    );
    assert!(
        READY_CALLBACK_SAW_DESTROY.load(Ordering::SeqCst),
        "node-ready callbacks should only run after the previous engine is destroyed"
    );
}

#[test]
fn replace_live_engine_recovery_reports_runtime_startup_errors() {
    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut live_engine = Engine::new(root);
    prepare_engine_for_runtime(&mut live_engine).expect("live engine should prepare");

    let shared_engine = Arc::new(Mutex::new(live_engine));

    let root: ReplaceOrderTestNode = Folder::new("Root").into();
    let mut next_engine = Engine::new(root);
    next_engine.add_node(BadReadyNode::new().into(), None);

    let recovery = replace_live_engine(&shared_engine, next_engine, "test_replace", true)
        .expect("recovery replacement should keep the usable graph");

    assert_eq!(recovery.problems.len(), 1);
    let problem = &recovery.problems[0];
    assert_eq!(problem.stage.as_str(), "runtime_startup");
    assert!(
        problem.message.contains("SetParam") && problem.message.contains("missing node"),
        "expected readable missing-node startup failure, got {}",
        problem.message
    );
}
