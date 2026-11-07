use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{ProjectNode, from_sparse_project_json, to_sparse_project_json_pretty, validate_engine_project_candidate},
    node::Node,
    process_ctx::ExecutionPhase,
};

#[test]
fn copying_a_screen_retains_independent_output_settings_through_history_and_projects() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let mut original = super::super::inspect(&engine).unwrap().screens[0].definition.clone();
    original.anti_aliasing = ScreenAntiAliasing::Smooth2x;
    original.frame_rate = ScreenFrameRate::Explicit { fps: 29.97 };
    original.enabled = false;
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: original.clone(),
        },
    );
    let root = engine.node_id_by_uuid(screen.0).unwrap();
    let parent = engine.nodes.get(root).unwrap().node_data().parent.unwrap();
    let before = super::super::inspect(&engine).unwrap();
    let history = engine.undo_len();
    let copied_root = engine
        .duplicate_subtree_with(
            root,
            parent,
            Some(root),
            None,
            Node::project_encode_data,
            AppNode::project_decode_node,
        )
        .unwrap();
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    let copied = ScreenId(engine.nodes.get(copied_root).unwrap().node_data().meta.uuid);
    assert_ne!(screen, copied);
    let snapshot = super::super::inspect(&engine).unwrap();
    let copy = snapshot.screens.iter().find(|item| item.id == copied).unwrap();
    assert_eq!(copy.definition, original);
    assert_ne!(
        copy.surfaces[0].reference.surface,
        before.screens[0].surfaces[0].reference.surface
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), snapshot);
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: copied,
            definition: ScreenDefinition {
                anti_aliasing: ScreenAntiAliasing::Raw,
                frame_rate: ScreenFrameRate::Explicit { fps: 50.0 },
                ..original.clone()
            },
        },
    );
    let edited = super::super::inspect(&engine).unwrap();
    assert_eq!(
        edited.screens.iter().find(|item| item.id == screen).unwrap().definition,
        original
    );
    assert_eq!(
        edited
            .screens
            .iter()
            .find(|item| item.id == copied)
            .unwrap()
            .definition
            .anti_aliasing,
        ScreenAntiAliasing::Raw
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert_eq!(
        edited
            .screens
            .iter()
            .find(|item| item.id == copied)
            .unwrap()
            .definition
            .frame_rate,
        ScreenFrameRate::Explicit { fps: 50.0 }
    );
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    validate_engine_project_candidate(&loaded).unwrap();
    assert_eq!(super::super::inspect(&loaded).unwrap(), edited);
    assert!(engine.undo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap(), snapshot);
}
