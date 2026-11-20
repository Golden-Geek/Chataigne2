use super::*;
use crate::media::{channel, validate_project};
use golden_assets::AbsoluteAssetPath;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

#[test]
fn file_drop_creates_and_replaces_the_primary_source_with_independent_history() {
    let mut engine = fixture();
    let directory = tempfile::tempdir().unwrap();
    let path =
        |name: &str| AbsoluteAssetPath::try_from(directory.path().join(name).to_string_lossy().into_owned()).unwrap();
    let drop_file = |path, target| MediaEdit::DropMediaFile { path, target };
    let history = engine.undo_len();
    let original = path("Projector still.WEBP");
    let MediaEditResult::Created { reference } = apply(&mut engine, drop_file(original.clone(), None)) else {
        panic!("file drop should create Picture media");
    };
    assert_eq!(engine.undo_len(), history + 1);
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!("expected Medias snapshot");
    };
    assert_eq!(snapshot.media[0].label, "Projector still");
    assert!(snapshot.assets.is_empty());
    let source = |engine: &AppEngine, reference| channel(engine, reference).unwrap().definition.source;
    let picture = |path| ChannelSource::Picture {
        file: Some(AssetLocator::Absolute { path }),
    };
    assert_eq!(source(&engine, reference), picture(original.clone()));
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, reference).is_err());
    assert!(engine.redo().unwrap());

    let MediaEditResult::Created { reference: peer } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!("expected an independent Picture channel");
    };
    let target = Some(reference.media.0);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(!invoke(&mut engine, drop_file(path("unsupported.txt"), None)).success);
    assert!(!invoke(&mut engine, drop_file(path("incompatible.mov"), target)).success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);

    let replacement = path("replacement.png");
    assert!(matches!(
        apply(&mut engine, drop_file(replacement.clone(), target)),
        MediaEditResult::Changed
    ));
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(source(&engine, reference), picture(replacement.clone()));
    assert_eq!(source(&engine, peer), ChannelSource::Picture { file: None });
    assert!(engine.undo().unwrap());
    assert_eq!(source(&engine, reference), picture(original));
    assert!(engine.redo().unwrap());

    let content = to_sparse_project_json_pretty(&engine).unwrap();
    let mut reloaded = from_sparse_project_json::<crate::app::AppNode>(&content).unwrap();
    reloaded.apply_edits().unwrap();
    validate_project(&reloaded).unwrap();
    assert_eq!(source(&reloaded, reference), picture(replacement));
    assert_eq!(source(&reloaded, peer), ChannelSource::Picture { file: None });
}

#[test]
fn shader_file_drop_sets_the_format_and_keeps_plain_json_unclassified() {
    use golden_video::shader::FragmentFormat;

    let mut engine = fixture();
    let root = tempfile::tempdir().unwrap();
    let path = |name: &str| AbsoluteAssetPath::try_from(root.path().join(name).to_string_lossy().into_owned()).unwrap();
    for (filename, expected) in [
        ("Rays.GLSL", FragmentFormat::Glsl),
        ("Grid.fs", FragmentFormat::Isf),
        ("Orbit.ShaderToy.JSON", FragmentFormat::ShaderToy),
    ] {
        let path = path(filename);
        let MediaEditResult::Created { reference } = apply(
            &mut engine,
            MediaEdit::DropMediaFile {
                path: path.clone(),
                target: None,
            },
        ) else {
            panic!("expected Shader Media");
        };
        assert_eq!(
            channel(&engine, reference).unwrap().definition.source,
            ChannelSource::Shader {
                format: expected,
                input: ShaderInput::File {
                    file: Some(AssetLocator::Absolute { path })
                },
                textures: Default::default(),
            }
        );
    }
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::DropMediaFile {
                path: path("configuration.json"),
                target: None
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!("expected Medias")
    };
    assert!(snapshot.media.iter().any(|media| media.label == "Orbit"));
}
