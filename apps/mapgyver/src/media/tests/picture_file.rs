use super::*;
use crate::{
    app::AppNode,
    media::{asset, channel},
};
use golden_core::{
    app::{ProjectLifecycle, from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeUuid},
    parameter::ParamValue,
};

fn binding(engine: &mut AppEngine, reference: ChannelReference) -> PictureFileBinding {
    let MediaEditResult::PictureFile { binding: Some(binding) } =
        apply(engine, MediaEdit::InspectPictureFile { reference })
    else {
        panic!()
    };
    binding
}

#[test]
fn channel_file_materializes_once_and_forwards_one_real_parameter_across_shared_consumers() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    let empty = binding(&mut engine, reference);
    assert!(empty.asset.is_none() && empty.parameter.is_none());
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference,
            file: String::new(),
        },
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference,
            file: "images/source.png".into(),
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let configured = binding(&mut engine, reference);
    let id = configured.asset.unwrap();
    let address = configured.parameter.as_ref().unwrap();
    let parameter = engine.nodes.get(address.node).unwrap();
    assert_eq!(parameter.node_data().meta.uuid, address.uuid);
    assert_eq!(
        parameter.engine_param_snapshot().unwrap().value,
        ParamValue::File("images/source.png".into())
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    let peer = create(&mut engine, MediaKind::Picture);
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: peer,
            source: ChannelSource::Picture { asset: Some(id) },
        },
    );
    assert_eq!(binding(&mut engine, peer).parameter, configured.parameter);
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: peer,
            file: "images/shared.png".into(),
        },
    );
    assert_eq!(binding(&mut engine, reference).asset, Some(id));
    assert_eq!(
        asset(&engine, id).unwrap().locator,
        AssetLocator::ProjectRelative {
            path: "images/shared.png".to_owned().try_into().unwrap()
        }
    );
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference,
            file: String::new(),
        },
    );
    assert!(asset(&engine, id).is_err());
    assert_eq!(binding(&mut engine, peer).asset, Some(id));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(binding(&mut loaded, reference).parameter.unwrap().uuid, address.uuid);
    assert_eq!(
        channel(&loaded, peer).unwrap().definition.source,
        ChannelSource::Picture { asset: Some(id) }
    );
}

#[test]
fn channel_file_rejections_are_atomic_and_deleted_asset_replacement_is_undoable() {
    let mut engine = fixture();
    let picture = create(&mut engine, MediaKind::Picture);
    let solid = create(&mut engine, MediaKind::SolidColor);
    for (reference, file) in [(picture, "../escape.png"), (solid, "image.png")] {
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::ConfigurePictureFile {
                    reference,
                    file: file.into()
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: picture,
            file: "first.png".into(),
        },
    );
    let first = binding(&mut engine, picture).asset.unwrap();
    apply(&mut engine, MediaEdit::RemoveAsset { asset: first });
    let missing = binding(&mut engine, picture);
    assert_eq!(missing.asset, Some(first));
    assert!(missing.parameter.is_none());
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: picture,
            file: "replacement.png".into(),
        },
    );
    let replacement = binding(&mut engine, picture).asset.unwrap();
    assert_ne!(replacement, first);
    assert!(engine.undo().unwrap());
    assert_eq!(binding(&mut engine, picture), missing);
    assert!(engine.node_id_by_uuid(NodeUuid(replacement.get())).is_none());
    assert!(engine.undo().unwrap());
    assert!(asset(&engine, first).is_ok());
}

#[test]
fn file_binding_query_is_immutable_nullable_after_deletion_and_shares_the_public_result() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    for removed in [false, true] {
        if removed {
            apply(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
        }
        let edit = MediaEdit::InspectPictureFile { reference };
        let payload = serde_json::to_value(&edit).unwrap();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.ui_history_state();
        let query = AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload)
            .unwrap()
            .unwrap();
        let reply = invoke(&mut engine, edit);
        assert!(reply.success);
        assert_eq!(query, reply.result.unwrap().value);
        assert_eq!(query["binding"].is_null(), removed);
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
