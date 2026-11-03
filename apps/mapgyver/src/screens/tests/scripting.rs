use super::*;
use crate::screens::authoring::manager;
use golden_core::{
    app::to_sparse_project_json_pretty,
    application::service_script_app_operations,
    edit::Edit,
    events::EventKind,
    node::{NodeId, NodeMetaPatch},
    parameter::ParamValue,
    script::{ScriptMethodCall, ScriptNode, ScriptNodeConfig, ScriptSource},
};

fn call(engine: &mut AppEngine, node: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
}

fn script(engine: &mut AppEngine, text: String) {
    let parent = manager(engine).unwrap();
    let mut node = ScriptNode::new(
        "Screen Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    node.budgets.max_wall_time_us_per_load = 1_000_000;
    node.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(node.into(), Some(parent));
    engine.apply_edits().unwrap();
}

fn replies(engine: &AppEngine) -> Vec<serde_json::Value> {
    engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "screen-test-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn hosted_manager_methods_support_create_inspect_and_domain_rejection() {
    let mut engine = fixture();
    let create = serde_json::to_string(&ScreenEdit::CreateScreen { label: None }).unwrap();
    script(
        &mut engine,
        format!(
            r#"
        function init() {{ local.listen({{level:1}}); local.editScreens('create', {create:?}); }}
        function onAppOperationResult(reply) {{
            gc.emit('screen-test-reply', reply);
            if (reply.request_id === 'create') local.inspectScreens('catalog');
            if (reply.request_id === 'catalog') local.editScreens('reject', JSON.stringify({{
                kind: 'RenameScreen', screen: reply.value.snapshot.screens[0].id, label: ''
            }}));
        }}
    "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine).len(), 1);
    service_script_app_operations(&mut engine).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies.len(), 3);
    assert_eq!(replies[0]["status"], "applied");
    assert_eq!(replies[1]["value"]["kind"], "Manager");
    assert_eq!(replies[2]["status"], "rejected");
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn hosted_geometry_intent_uses_the_same_crop_validation_result_and_history() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let original = definition(&engine, reference);
    let request = serde_json::to_string(&ScreenEdit::EditSurfaceGeometry {
        reference,
        edit: SurfaceGeometryEdit::CropCorner {
            corner: 0,
            point: Point { x: 0.25, y: 0.125 },
        },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            r#"
            function init() {{
                local.listen({{level:1}});
                local.editScreens('crop', {request:?});
            }}
            function onAppOperationResult(reply) {{
                gc.emit('screen-test-reply', reply);
            }}
            "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "applied");
    assert_eq!(results[0]["value"]["kind"], "SurfaceChanged");
    let changed = definition(&engine, reference);
    assert_eq!(
        changed.crop,
        UnitRect {
            x: 0.25,
            y: 0.125,
            width: 0.75,
            height: 0.875
        }
    );
    assert_eq!(changed.quad, original.quad);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
}

#[test]
fn hosted_pixel_geometry_uses_backend_extent_and_one_history_step() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "Script pixels".into(),
                    extent: golden_video::Extent2D::new(640, 360).unwrap(),
                }),
                enabled: false,
            },
        },
    );
    let original = definition(&engine, reference);
    let request = serde_json::to_string(&ScreenEdit::EditSurfaceGeometry {
        reference,
        edit: SurfaceGeometryEdit::QuadCornerPixels {
            corner: 0,
            origin: original.quad.corners[0],
            pixels: PixelOffset { x: 1, y: 10 },
        },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            "function init() {{ local.listen({{level:1}}); local.editScreens('pixels', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('screen-test-reply', reply); }}"
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies[0]["status"], "applied");
    let changed = definition(&engine, reference);
    assert!((changed.quad.corners[0].x - original.quad.corners[0].x - 1.0 / 640.0).abs() < 1e-6);
    assert!((changed.quad.corners[0].y - original.quad.corners[0].y - 10.0 / 360.0).abs() < 1e-6);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
}

#[test]
fn hosted_quad_translation_uses_the_same_atomic_geometry_and_history() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let original = definition(&engine, reference);
    let request = serde_json::to_string(&ScreenEdit::EditSurfaceGeometry {
        reference,
        edit: SurfaceGeometryEdit::QuadTranslation {
            quad: original.quad,
            origin: Point { x: 0.5, y: 0.5 },
            point: Point { x: 0.25, y: 0.625 },
        },
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            "function init() {{ local.listen({{level:1}}); local.editScreens('translate', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('screen-test-reply', reply); }}"
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine)[0]["status"], "applied");
    let mut expected = original.clone();
    expected.quad.corners = original.quad.corners.map(|corner| Point {
        x: corner.x - 0.25,
        y: corner.y + 0.125,
    });
    assert_eq!(definition(&engine, reference), expected);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
}

#[test]
fn hosted_fill_and_fit_use_the_same_backend_geometry_and_one_step_history() {
    for mode in [SurfaceFitMode::Fit, SurfaceFitMode::Fill] {
        let mut engine = fixture();
        let (screen, reference) = create(&mut engine);
        let source = super::drop_sources::source(&mut engine);
        apply(
            &mut engine,
            ScreenEdit::ConfigureScreen {
                screen,
                definition: ScreenDefinition {
                    target: Some(ScreenTarget::Spout {
                        sender: "Script fit".into(),
                        extent: golden_video::Extent2D::new(800, 600).unwrap(),
                    }),
                    enabled: false,
                },
            },
        );
        let mut original = definition(&engine, reference);
        original.source = Some(source);
        original.quad.corners.swap(1, 2);
        apply(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference,
                definition: original.clone(),
            },
        );
        let request = serde_json::to_string(&ScreenEdit::FitSurface { reference, mode }).unwrap();
        script(
            &mut engine,
            format!(
                "function init() {{ local.listen({{level:1}}); local.editScreens('fit', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('screen-test-reply', reply); }}"
            ),
        );
        let history = engine.undo_len();
        service_script_app_operations(&mut engine).unwrap();
        assert_eq!(replies(&engine)[0]["status"], "applied");
        assert_eq!(engine.undo_len(), history + 1);
        let ScreenEditResult::Surface { snapshot } = apply(&mut engine, ScreenEdit::InspectSurface { reference })
        else {
            panic!()
        };
        match mode {
            SurfaceFitMode::Fit => assert!((snapshot.definition.quad.corners[0].y - 0.125).abs() < 1e-6),
            SurfaceFitMode::Fill => assert_eq!(snapshot.definition.quad, Quad::default()),
        };
        assert!(engine.undo().unwrap());
        assert_eq!(definition(&engine, reference), original);
    }
}

#[test]
fn source_drops_use_hosted_manager_methods_with_atomic_history() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let source = super::drop_sources::source(&mut engine);
    let request = serde_json::to_string(&ScreenEdit::DropSource {
        screen,
        surface: None,
        source: source.media.0,
    })
    .unwrap();
    script(
        &mut engine,
        format!(
            "function init() {{ local.listen({{level:1}}); local.editScreens('drop', {request:?}); }} function onAppOperationResult(reply) {{ gc.emit('screen-test-reply', reply); }}"
        ),
    );
    let original = crate::screens::inspect(&engine).unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["status"], "applied");
    assert_eq!(results[0]["value"]["kind"], "SurfaceCreated");
    let dropped = crate::screens::inspect(&engine).unwrap();
    assert_eq!(dropped.screens[0].surfaces.len(), 2);
    assert_eq!(dropped.screens[0].surfaces[1].definition.source, Some(source));
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), dropped);
}

#[test]
fn manager_disablement_cancels_pending_requests_and_rejects_new_calls() {
    let mut engine = fixture();
    let manager = manager(&engine).unwrap();
    let request = serde_json::to_string(&ScreenEdit::CreateScreen { label: None }).unwrap();
    call(
        &mut engine,
        manager,
        "editScreens",
        vec![ParamValue::Str("create".into()), ParamValue::Str(request)],
    );
    engine.edits.push(Edit::PatchMeta {
        node: manager,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: manager,
        call: ScriptMethodCall::prepare("inspectScreens", vec![ParamValue::Str("disabled".into())]).unwrap(),
    });
    assert!(engine.apply_edits().is_err());
    assert!(engine.undo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert!(crate::screens::inspect(&engine).unwrap().screens.is_empty());
}
