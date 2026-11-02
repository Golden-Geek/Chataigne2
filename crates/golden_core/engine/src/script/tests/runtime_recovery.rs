use std::time::Duration;

use crate::{
    edit::Edit,
    engine::Engine,
    node::{Node, NodeMetaPatch},
    script::{ScriptBudgets, ScriptNode, ScriptNodeConfig, ScriptSource},
};

#[test]
fn ui_reload_reschedules_a_passive_inline_script_without_an_unrelated_edit() {
    let source = |value| ScriptNodeConfig {
        source: ScriptSource::Inline {
            text: format!("function init(){{gc.emit('passive-loaded',{value});}}"),
        },
    };
    let mut engine = Engine::new(ScriptNode::new("Passive", source(1)));
    engine.resolve().unwrap();
    engine.run_tick(Duration::from_millis(20)).unwrap();
    assert_eq!(
        engine.nodes.get(engine.root).unwrap().execution_rule(),
        crate::engine::NodeExecutionRule::passive()
    );
    // Settle the passive schedule before changing saved source without forcing a reload.
    engine.run_tick(Duration::from_millis(20)).unwrap();
    engine
        .ui_set_script_config(engine.root, (&source(2)).into(), false)
        .unwrap();
    engine.run_tick(Duration::from_millis(20)).unwrap();
    let history = engine.undo_len();
    engine.ui_reload_script(engine.root).unwrap();
    engine.run_tick(Duration::from_millis(20)).unwrap();
    assert!(engine.ui_event_log().iter().any(|event| matches!(
        &event.kind,
        crate::events::EventKind::Custom(event)
            if event.topic == "passive-loaded" && *event.payload == serde_json::json!(2)
    )));
    assert_eq!(
        engine.nodes.get(engine.root).unwrap().execution_rule(),
        crate::engine::NodeExecutionRule::passive()
    );
    assert_eq!(
        engine.undo_len(),
        history,
        "runtime reload must not author another history step"
    );
}

#[test]
fn failed_script_is_quarantined_while_edits_saves_and_reload_continue() {
    let mut script = ScriptNode::new(
        "Script",
        ScriptNodeConfig {
            source: ScriptSource::Inline {
                text: "function update() { emit('must-not-escape', 1); throw new Error('failed'); }".to_string(),
            },
        },
    );
    script.budgets = ScriptBudgets {
        max_wall_time_us_per_callback: 20_000,
        ..ScriptBudgets::default()
    };
    let mut engine = Engine::new(script);
    engine.resolve().expect("script schedule should resolve");

    engine
        .run_tick(Duration::from_millis(20))
        .expect("a failed script callback must not fail the engine tick");
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch {
            label: Some("Recovered Script".to_string()),
            ..NodeMetaPatch::default()
        },
    });
    engine
        .apply_edits()
        .expect("unrelated edits must progress after script failure");
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("script root should remain present")
            .node_data()
            .meta
            .label,
        "Recovered Script"
    );
    let saved = engine
        .to_project_json_with(Node::project_encode_data)
        .expect("project capture must progress after script failure");
    assert!(saved.contains("Recovered Script"));

    engine
        .nodes
        .get_mut(engine.root)
        .expect("script root should remain mutable")
        .set_config(
            ScriptNodeConfig {
                source: ScriptSource::Inline {
                    text: "function update() {}".to_string(),
                },
            },
            true,
        );
    engine
        .run_tick(Duration::from_millis(20))
        .expect("the next tick should reinitialize a clean script context");
    assert!(
        engine
            .nodes
            .get(engine.root)
            .expect("reloaded script should remain present")
            .manifest()
            .is_some(),
        "successful reload should publish a manifest"
    );
}
