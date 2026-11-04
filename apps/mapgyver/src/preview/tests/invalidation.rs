use crate::{
    app::AppEngine,
    media::protocol::{ChannelReference, MEDIA_NAMESPACE, MEDIA_OPERATION, MediaEdit, MediaEditResult, MediaKind},
    preview::{capture::capture_presentation, protocol::PreviewSource},
    screens::protocol::{
        Point, SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit, ScreenEditResult, SurfaceDefinition, SurfaceGeometryEdit,
    },
};
use golden_core::{
    app::create_new_project_engine,
    application::apply_project_intent,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};
use golden_io::WorkerNotifier;

fn invoke<T: serde::de::DeserializeOwned>(
    engine: &mut AppEngine,
    namespace: &str,
    operation: &str,
    edit: impl serde::Serialize,
) -> T {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: namespace.into(),
            operation: operation.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn color_parameter(engine: &AppEngine, reference: ChannelReference) -> NodeId {
    let root = engine.node_id_by_uuid(reference.channel.0).unwrap();
    let folder = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    let mut child = engine.nodes.get(folder).unwrap().node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id).unwrap();
        if node.node_data().meta.decl_id.0 == "parameters/rgba" {
            return id;
        }
        child = node.node_data().next_sibling;
    }
    panic!("missing color parameter")
}

#[test]
fn repeated_views_share_one_capture_source_without_relaxing_distinct_source_admission() {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let create = |engine: &mut AppEngine| {
        let MediaEditResult::Created { reference } = invoke(
            engine,
            MEDIA_NAMESPACE,
            MEDIA_OPERATION,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            },
        ) else {
            panic!()
        };
        PreviewSource::Channel { reference }
    };
    let first = create(&mut engine);
    let capture = capture_presentation(&engine, &vec![first; 64], 1, 128, &[]).unwrap();
    assert!(capture.channels.is_current());
    let second = create(&mut engine);
    assert!(capture_presentation(&engine, &[first, second], 1, 128, &[]).is_err());
}

#[test]
fn authored_color_and_warp_edits_wake_presentation_without_polling_or_source_frames() {
    let mut engine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference } = invoke(
        &mut engine,
        MEDIA_NAMESPACE,
        MEDIA_OPERATION,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ) else {
        panic!()
    };
    let ScreenEditResult::ScreenCreated { screen, surface } = invoke(
        &mut engine,
        SCREEN_NAMESPACE,
        SCREEN_OPERATION,
        ScreenEdit::CreateScreen { label: None },
    ) else {
        panic!()
    };
    let _: ScreenEditResult = invoke(
        &mut engine,
        SCREEN_NAMESPACE,
        SCREEN_OPERATION,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                ..Default::default()
            },
        },
    );
    let sources = [PreviewSource::Screen { screen }];
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let notifier = WorkerNotifier::new(move || {
        let _ = send.try_send(());
    });
    let capture = capture_presentation(&engine, &sources, 8, 128, &[]).unwrap();
    let subscriptions = capture.subscribe(notifier.clone());
    assert!(receive.try_recv().is_err(), "current captures do not wake needlessly");
    let color = color_parameter(&engine, reference);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: color,
            value: ParamValue::Color(0.2, 0.4, 0.6, 1.0),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(!capture.is_current());
    assert_eq!(
        receive.try_recv(),
        Ok(()),
        "color edit wakes the preparation worker immediately"
    );
    drop(subscriptions);
    let capture = capture_presentation(&engine, &sources, 8, 128, &[]).unwrap();
    let subscriptions = capture.subscribe(notifier.clone());
    let _: ScreenEditResult = invoke(
        &mut engine,
        SCREEN_NAMESPACE,
        SCREEN_OPERATION,
        ScreenEdit::EditSurfaceGeometry {
            reference: surface,
            edit: SurfaceGeometryEdit::QuadCorner {
                corner: 0,
                point: Point { x: 0.1, y: 0.1 },
            },
        },
    );
    assert!(!capture.is_current());
    assert_eq!(
        receive.try_recv(),
        Ok(()),
        "warp edit wakes the preparation worker immediately"
    );
    drop(subscriptions);
    let _late_subscriptions = capture.subscribe(notifier);
    assert_eq!(receive.try_recv(), Ok(()), "an edit racing subscription cannot be lost");
}
