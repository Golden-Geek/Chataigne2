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

fn display(id: &str, index: u32, primary: bool, edid: u8) -> DisplayInfo {
    DisplayInfo {
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
fn displays(engine: &mut AppEngine, displays: Vec<DisplayInfo>) {
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
fn live_creation_uses_next_secondary_reserves_disabled_outputs_and_falls_back_to_unique_shared_senders() {
    let mut engine = fixture();
    displays(
        &mut engine,
        vec![
            display("primary", 1, true, 1),
            display("secondary-two", 2, false, 2),
            display("secondary-three", 3, false, 3),
        ],
    );
    let (first, _) = create(&mut engine);
    assert!(
        matches!(definition(&engine, first).target, Some(ScreenTarget::Fullscreen { display }) if display == "secondary-two")
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
        matches!(&snapshot.screens[1].definition.target, Some(ScreenTarget::Fullscreen { display }) if display == "secondary-three")
    );
    assert!(snapshot.screens[1].definition.enabled);
    let (third, _) = create(&mut engine);
    let (fourth, _) = create(&mut engine);
    assert!(
        matches!(definition(&engine, third).target, Some(ScreenTarget::Spout { sender, .. }) if sender == "MapGyver Screen")
    );
    assert!(
        matches!(definition(&engine, fourth).target, Some(ScreenTarget::Spout { sender, .. }) if sender == "MapGyver Screen 2")
    );
    let Some(AppNode::Screen(node)) = engine.nodes.get(first_node) else {
        panic!()
    };
    let hint = node.display_hint();
    assert_eq!(hint.identity, "secondary-two");
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
