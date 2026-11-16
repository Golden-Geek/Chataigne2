use super::*;
use crate::{
    app::AppNode,
    media::protocol::{MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind},
};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeMetaPatch},
    ui_sync::UiEditIntent,
};
use std::sync::Arc;

pub(super) fn display(id: &str, index: u32, primary: bool, edid: u8) -> DisplayInfo {
    DisplayInfo {
        internal: Some(primary),
        refresh_rate: None,
        id: id.into(),
        name: format!("Display {index}"),
        index,
        primary,
        edid: Some(format!("{edid:02x}").repeat(128)),
        bounds: PhysicalRectangle {
            left: ((index - 1) * 1920) as i32,
            top: 0,
            extent: golden_video::Extent2D::new(1920, 1080).unwrap(),
        },
    }
}
fn manager(engine: &AppEngine) -> golden_core::node::NodeId {
    engine
        .nodes
        .iter()
        .find_map(|(id, node)| matches!(node, AppNode::ScreenManager(_)).then_some(id))
        .unwrap()
}
pub(super) fn displays(engine: &mut AppEngine, displays: Vec<DisplayInfo>) {
    let id = manager(engine);
    let Some(AppNode::ScreenManager(node)) = engine.nodes.get_mut(id) else {
        panic!()
    };
    node.set_displays(Arc::from(displays));
}
fn definition(engine: &AppEngine, id: ScreenId) -> ScreenDefinition {
    let Some(AppNode::Screen(node)) = engine.nodes.get(engine.node_id_by_uuid(id.0).unwrap()) else {
        panic!()
    };
    node.definition()
}
fn assert_unselected_fullscreen(engine: &AppEngine, screen: ScreenId) {
    assert!(definition(engine, screen).target.is_none());
    let id = engine.node_id_by_uuid(screen.0).unwrap();
    let fields = super::parameters::parameters(engine, id);
    for (field, expected) in [("parameters/target_kind", "Fullscreen"), ("parameters/display", "")] {
        assert_eq!(
            engine
                .nodes
                .get(fields[field])
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            golden_core::parameter::ParamValue::Enum(expected.into())
        );
    }
}

#[test]
fn creation_uses_physical_external_identity_even_when_primary_and_ignores_unknown_connections() {
    let mut external = display("external", 9, true, 2);
    external.internal = Some(false);
    let mut internal = display("built-in", 1, false, 1);
    internal.internal = Some(true);
    let mut unknown = display("unknown", 2, false, 3);
    unknown.internal = None;
    for catalog in [vec![], vec![internal.clone()], vec![unknown.clone()]] {
        let mut engine = fixture();
        displays(&mut engine, catalog);
        let (screen, _) = create(&mut engine);
        assert_unselected_fullscreen(&engine, screen);
    }
    for catalog in [
        vec![internal.clone(), external.clone(), unknown.clone()],
        vec![unknown, external, internal],
    ] {
        let mut engine = fixture();
        displays(&mut engine, catalog);
        let (first, _) = create(&mut engine);
        assert_eq!(
            definition(&engine, first).target,
            Some(ScreenTarget::Fullscreen {
                display: "external".into()
            })
        );
        let (second, _) = create(&mut engine);
        assert_unselected_fullscreen(&engine, second);
    }
}

#[test]
fn atomic_screen_creation_reserves_prepared_siblings_and_preserves_exact_undo_redo_reload() {
    use golden_core::ui_sync::UiDuplicateCreateUserItemSpec;
    let mut engine = fixture();
    displays(
        &mut engine,
        vec![display("external-b", 2, false, 2), display("external-a", 7, false, 3)],
    );
    let parent = manager(&engine);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let sources = [engine.root, parent];
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNodes {
            nodes: vec![],
            created_items: sources
                .into_iter()
                .map(|source| UiDuplicateCreateUserItemSpec {
                    source,
                    parent,
                    node_type: "mapgyver.screen".into(),
                    label: None,
                    initial_params: vec![],
                })
                .collect(),
            dependent_items: (0..2)
                .map(|_| golden_core::ui_sync::UiDuplicateDependentUserItem {
                    parent,
                    node_type: "mapgyver.screen".into(),
                    label: None,
                    initial_params: vec![],
                })
                .collect(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let snapshot = super::super::inspect(&engine).unwrap();
    assert_eq!(snapshot.screens.len(), 4);
    let unselected = snapshot
        .screens
        .iter()
        .filter(|screen| screen.definition.target.is_none())
        .collect::<Vec<_>>();
    assert_eq!(unselected.len(), 2);
    for screen in unselected {
        assert_unselected_fullscreen(&engine, screen.id);
    }
    let mut targets = snapshot
        .screens
        .iter()
        .filter_map(|screen| screen.definition.target.clone())
        .map(|target| match target {
            ScreenTarget::Fullscreen { display } => display,
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    targets.sort();
    assert_eq!(targets, ["external-a", "external-b"]);
    let committed = to_sparse_project_json_pretty(&engine).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), committed);
    let mut loaded = from_sparse_project_json::<AppNode>(&committed).unwrap();
    loaded.apply_edits().unwrap();
    displays(&mut loaded, vec![display("different", 1, false, 4)]);
    assert_eq!(super::super::inspect(&loaded).unwrap(), snapshot);
}
fn media(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

#[test]
fn live_creation_reserves_disabled_external_assignments_and_leaves_fullscreen_unselected_when_full() {
    let mut engine = fixture();
    displays(
        &mut engine,
        vec![
            display("primary", 1, true, 1),
            display("external-a", 2, false, 2),
            display("external-b", 3, false, 3),
        ],
    );
    let (first, _) = create(&mut engine);
    assert!(
        matches!(definition(&engine, first).target, Some(ScreenTarget::Fullscreen { display }) if display == "external-a")
    );
    let first_node = engine.node_id_by_uuid(first.0).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node: first_node,
            patch: NodeMetaPatch {
                enabled: Some(false),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let root = manager(&engine);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent: root,
            node_type: "mapgyver.screen".into(),
            label: None,
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let snapshot = super::super::inspect(&engine).unwrap();
    assert_eq!(snapshot.screens.len(), 2);
    assert!(
        matches!(&snapshot.screens[1].definition.target, Some(ScreenTarget::Fullscreen { display }) if display == "external-b")
    );
    assert!(snapshot.screens[1].definition.enabled);
    let (third, _) = create(&mut engine);
    let (fourth, _) = create(&mut engine);
    assert_unselected_fullscreen(&engine, third);
    assert_unselected_fullscreen(&engine, fourth);
    let Some(AppNode::Screen(node)) = engine.nodes.get(first_node) else {
        panic!()
    };
    let hint = node.display_hint();
    assert_eq!(hint.identity, "external-a");
    assert_eq!(hint.index, 2);
    assert!(hint.edid.is_some());
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    let Some(AppNode::Screen(node)) = loaded.nodes.get(loaded.node_id_by_uuid(first.0).unwrap()) else {
        panic!()
    };
    assert_eq!(node.display_hint(), hint);
    assert!(!node.definition().enabled);
    let Some(AppNode::ScreenManager(node)) = loaded.nodes.get(manager(&loaded)) else {
        panic!()
    };
    assert!(node.displays().is_empty(), "native catalog must not be authored data");
}

#[test]
fn last_good_display_hints_reserve_a_recovered_display_without_rewriting_the_saved_identity() {
    let mut engine = fixture();
    displays(
        &mut engine,
        vec![display("primary", 1, true, 1), display("old-path", 2, false, 2)],
    );
    let (first, _) = create(&mut engine);
    let id = engine.node_id_by_uuid(first.0).unwrap();
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let hint = node.display_hint();
    displays(
        &mut engine,
        vec![
            display("primary", 1, true, 1),
            display("recovered-path", 2, false, 2),
            display("available", 3, false, 3),
        ],
    );
    let (second, _) = create(&mut engine);
    assert!(
        matches!(definition(&engine, second).target, Some(ScreenTarget::Fullscreen { display }) if display == "available")
    );
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        panic!()
    };
    assert_eq!(node.display_hint(), hint, "fallback matching is runtime-only");
}

#[test]
fn initial_surface_uses_first_enabled_media_in_folder_order_and_its_first_ordered_channel() {
    let mut engine = fixture();
    let MediaEditResult::Created { reference: disabled } = media(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ) else {
        panic!()
    };
    let node = engine.node_id_by_uuid(disabled.media.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
                .into()
            }
        )
        .success
    );
    let MediaEditResult::FolderCreated { folder } = media(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference } = media(
        &mut engine,
        MediaEdit::CreateMediaInFolder {
            folder,
            media_kind: MediaKind::Picture,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference: expected } = media(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!()
    };
    media(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: expected,
            after: None,
        },
    );
    media(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    );
    let (_, surface) = create(&mut engine);
    assert_eq!(super::definition(&engine, surface).source, Some(expected));
}

#[test]
fn intrinsic_screen_factory_remains_stable_and_enablement_has_no_parameter_duplicate() {
    let mut engine = fixture();
    let root = manager(&engine);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    displays(&mut engine, vec![display("secondary", 2, false, 2)]);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let owner = engine.nodes.get(root).unwrap();
    let tree = owner.create_user_item_tree("mapgyver.screen").unwrap();
    let node = tree.node.as_any().downcast_ref::<super::super::ScreenNode>().unwrap();
    assert_eq!(node.definition(), ScreenDefinition::default());
    assert_eq!(node.authored_display_identity(), "");
    let (screen, _) = create(&mut engine);
    let root = engine.node_id_by_uuid(screen.0).unwrap();
    let mut child = engine.nodes.get(root).unwrap().node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        child = node.node_data().next_sibling;
        if node.node_data().meta.decl_id.0 != "parameters" {
            continue;
        }
        let mut leaf = node.node_data().first_child;
        while let Some(id) = leaf {
            let node = engine.nodes.get(id).unwrap();
            leaf = node.node_data().next_sibling;
            assert_ne!(node.node_data().meta.decl_id.0, "parameters/render_enabled");
            if node.node_data().meta.decl_id.0 == "parameters/display" {
                assert!(matches!(
                    node.engine_param_snapshot().unwrap().value,
                    golden_core::parameter::ParamValue::Enum(_)
                ));
            }
            if node.node_data().meta.decl_id.0.starts_with("parameters/display_") {
                let snapshot = node.engine_param_snapshot().unwrap();
                assert!(snapshot.read_only);
                assert!(!node.node_data().meta.presentation.show_in_nested_inspector);
            }
        }
    }
}
