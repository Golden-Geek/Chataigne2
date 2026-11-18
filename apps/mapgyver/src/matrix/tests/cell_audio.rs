use super::*;
use crate::matrix::audio::Mix;
use golden_core::{
    node::NodeId,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::time::{Duration, Instant};

fn audio_parameter(engine: &AppEngine, cell: CellId, name: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(cell.0).unwrap();
    let tree = engine.process_tree_snapshot();
    let parameters = tree.find_child_by_decl_id(owner, "parameters").unwrap();
    let audio = tree.find_child_by_decl_id(parameters, "parameters/audio").unwrap();
    tree.find_child_by_decl_id(audio, &format!("parameters/audio/{name}"))
        .unwrap()
}

fn author(engine: &mut AppEngine, cell: CellId, file: &str) {
    for (name, value) in [
        ("audio_mode", ParamValue::Enum("File".into())),
        ("audio_file", ParamValue::File(file.into())),
    ] {
        let node = audio_parameter(engine, cell, name);
        let ack = apply_project_intent(
            engine,
            UiEditIntent::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
    }
}

#[test]
fn matrix_cell_audio_inherits_overrides_crossfades_and_reloads_without_visual_media() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let layer = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    let video = create(&mut engine, MediaKind::Video);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer,
                column: columns[0],
                source: Some(video),
            }],
        },
    );
    let cells = columns
        .iter()
        .map(|&column| {
            let MediaEditResult::MatrixCellPrepared { cell, .. } =
                edit(&mut engine, matrix.media, MatrixEdit::EnsureCell { layer, column })
            else {
                panic!("Cell")
            };
            cell
        })
        .collect::<Vec<_>>();
    author(&mut engine, cells[1], "audio-only.wav");
    let root = super::super::playback::root(&engine, matrix.media).unwrap();
    let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root) else {
        panic!("Matrix root")
    };
    let control = root.matrix_playback().unwrap().clone();
    let launch = |engine: &mut AppEngine, column, mode| {
        edit(engine, matrix.media, MatrixEdit::LaunchCell { layer, column, mode })
    };
    let capture = || {
        let mut mix = Mix::default();
        let now = Instant::now();
        control.capture_audio(now, &mut mix).unwrap();
        (mix, now)
    };

    launch(&mut engine, columns[0], LaunchMode::Cut);
    let (mix, now) = capture();
    assert_eq!(mix.media[&video.channel][0].at(now), 1.0);
    assert!(mix.files.is_empty());
    author(&mut engine, cells[0], "override.wav");
    let (mix, now) = capture();
    assert!(mix.media[&video.channel].is_empty());
    assert_eq!(mix.files.len(), 1);
    assert_eq!(mix.files[0].weight.at(now), 1.0);

    assert!(
        super::super::configuration::index(&engine, matrix.media, &control)
            .unwrap()
            .playable
            .contains(&(layer, columns[1]))
    );
    launch(&mut engine, columns[1], LaunchMode::Configured);
    let (mix, now) = capture();
    assert_eq!(mix.files.len(), 2);
    let mid = now + Duration::from_millis(125);
    assert!(mix.files.iter().all(|source| (source.weight.at(mid) - 0.5).abs() < 0.1));
    edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    let (mix, _) = capture();
    assert!(mix.files.is_empty());
    assert!(mix.media.is_empty());

    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut reloaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    reloaded.apply_edits().unwrap();
    reloaded.settle_edit_notifications(16).unwrap();
    let Some(AppNode::MatrixCell(reloaded)) = reloaded
        .node_id_by_uuid(cells[1].0)
        .and_then(|id| reloaded.nodes.get(id))
    else {
        panic!("reloaded Audio Cell")
    };
    assert_eq!(reloaded.audio_label().as_deref(), Some("audio-only.wav"));
}
