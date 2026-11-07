use super::transitions::{launch, matrix_edit, prepared, time};
use super::*;
use crate::matrix::protocol::*;
use crate::media::{capture_channels_at, composition::protocol::*};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

#[test]
fn small_matrix_reference_chain_retains_authored_geometry_and_producer_after_reload() {
    let mut engine = engine();
    let extent = Extent2D::new(4, 4).unwrap();
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: source,
            source: ChannelSource::SolidColor { rgba: [1., 0., 0., 1.] },
        },
    );
    let MediaEditResult::ReferenceCreated { media } = edit(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let composition = create(&mut engine, MediaKind::Composition);
    for media in [source.media, composition.media] {
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media,
                size: SizePolicy::Fixed { extent },
            },
        );
    }
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::DropSource {
                source: media.0,
                instance: None,
                position: Point { x: 2., y: 2. },
                copy: false,
            },
        },
    );
    let matrix = create(&mut engine, MediaKind::Matrix);
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: matrix.media,
            size: SizePolicy::Fixed { extent },
        },
    );
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddColumns { parent: None, count: 1 },
    ) else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::DropSource {
            layer,
            column: columns[0],
            source: composition.media.0,
            copy: false,
        },
    );
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let before = crate::matrix::inspect(&engine, matrix.media).unwrap();
    let composition_before = crate::media::composition::inspect(&engine, composition).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    assert_eq!(crate::matrix::inspect(&loaded, matrix.media).unwrap(), before);
    assert_eq!(
        crate::media::composition::inspect(&loaded, composition).unwrap(),
        composition_before
    );
    launch(&mut loaded, matrix.media, columns[0], LaunchMode::Cut);
    let owner = renderer();
    for (stage, document) in ["before", "after"].into_iter().zip([&engine, &loaded]) {
        let captured = capture_channels_at(document, &[matrix], 32, time()).unwrap();
        assert!(
            captured
                .channels
                .iter()
                .any(|channel| channel.reference == source && channel.definition.is_some()),
            "{stage} reload must retain the selected producer"
        );
        let layer = captured
            .channels
            .iter()
            .find(|channel| channel.reference == before.items[0].channel)
            .unwrap();
        assert_eq!(layer.matrix.as_ref().unwrap().canvas, extent);
        assert_eq!(layer.matrix.as_ref().unwrap().inputs.len(), 1);
        let plan = prepared(&captured, matrix, &owner);
        assert_eq!(
            plan.channels
                .iter()
                .find(|channel| channel.reference == source)
                .unwrap()
                .resolution
                .availability,
            crate::media::resolution::ChannelAvailability::Ready
        );
    }
    close(owner);
}
