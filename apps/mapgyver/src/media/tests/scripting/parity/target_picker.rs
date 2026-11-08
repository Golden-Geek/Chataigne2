use super::*;
use golden_core::node::Node;

#[test]
fn bounded_picker_reads_and_validated_choices_match_all_public_callers() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let other = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(first.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let AppNode::ReferenceMedia(node) = engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()).unwrap() else {
        panic!()
    };
    let media_param = engine.nodes.get(node.target_parameter()).unwrap().node_data().meta.uuid;
    let channel_param = engine
        .nodes
        .get(node.channel_parameter())
        .unwrap()
        .node_data()
        .meta
        .uuid;
    let baseline = to_sparse_project_json_pretty(&engine).unwrap();
    for operation in [
        MediaEdit::InspectTargetPicker {
            query: TargetPickerQuery {
                parameter: media_param,
                folder: None,
                cursor: None,
                backwards: false,
                limit: 64,
            },
        },
        MediaEdit::InspectTargetPicker {
            query: TargetPickerQuery {
                parameter: channel_param,
                folder: None,
                cursor: None,
                backwards: false,
                limit: 0,
            },
        },
        MediaEdit::SelectTargetPickerChoice {
            parameter: channel_param,
            target: Some(second.channel.0),
        },
        MediaEdit::SelectTargetPickerChoice {
            parameter: channel_param,
            target: Some(other.channel.0),
        },
        MediaEdit::SelectTargetPickerChoice {
            parameter: media_param,
            target: Some(other.media.0),
        },
        MediaEdit::SelectTargetPickerChoice {
            parameter: media_param,
            target: Some(media.0),
        },
        MediaEdit::SelectTargetPickerChoice {
            parameter: media_param,
            target: None,
        },
    ] {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&baseline);
            let result = run(&mut engine, caller, operation.clone());
            let actual = (result, authored(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {operation:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}
