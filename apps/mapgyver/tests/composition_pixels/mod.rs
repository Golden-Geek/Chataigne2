use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::{Extent2D, plan::SampleFilter};
use mapgyver::{
    app::{AppEngine, AppNode},
    media::{channel, composition::protocol::*, protocol::*},
};
use std::time::Duration;

use crate::support::{native_pixels as fixture, raster as oracle};

fn edit(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(operation).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn configure(engine: &mut AppEngine, reference: ChannelReference, source: ChannelSource, size: SizePolicy) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.source = source;
    definition.size = size;
    definition.filter = SampleFilter::Nearest;
    edit(engine, MediaEdit::ConfigureChannel { reference, definition });
}
fn transform(position: [f32; 2], scale: [f32; 2], rotation: f32, pivot: [f32; 2]) -> Transform2D {
    Transform2D {
        position,
        scale,
        rotation,
        pivot,
    }
}
fn instance(source: ChannelReference, transform: Transform2D) -> InstanceDefinition {
    InstanceDefinition {
        source: Some(source),
        transform,
        ..Default::default()
    }
}
fn add(engine: &mut AppEngine, owner: ChannelReference, definitions: &[InstanceDefinition]) -> Vec<InstanceId> {
    let MediaEditResult::InstancesCreated { instances } = edit(
        engine,
        MediaEdit::Composition {
            reference: owner,
            edit: CompositionEdit::AddInstances {
                sources: definitions.iter().map(|d| d.source.unwrap()).collect(),
            },
        },
    ) else {
        panic!()
    };
    let ids = instances.iter().map(|i| i.instance).collect::<Vec<_>>();
    for (id, definition) in ids.iter().zip(definitions) {
        set_instance(engine, owner, *id, definition.clone());
    }
    ids
}
fn set_instance(
    engine: &mut AppEngine,
    reference: ChannelReference,
    instance: InstanceId,
    definition: InstanceDefinition,
) {
    edit(
        engine,
        MediaEdit::Composition {
            reference,
            edit: CompositionEdit::ConfigureInstance { instance, definition },
        },
    );
}

struct Scene {
    red: ChannelReference,
    inner: Vec<InstanceDefinition>,
    outer: Vec<InstanceDefinition>,
    outer_ids: Vec<InstanceId>,
    root: ChannelReference,
    width: usize,
    height: usize,
}

impl Scene {
    fn new(engine: &mut AppEngine) -> Self {
        let red = create(engine, MediaKind::SolidColor);
        let green = create(engine, MediaKind::SolidColor);
        let blue = create(engine, MediaKind::SolidColor);
        for (source, rgba) in [
            (red, [1.0, 0.0, 0.0, 0.5]),
            (green, [0.0, 1.0, 0.0, 0.75]),
            (blue, [0.0, 0.0, 1.0, 0.25]),
        ] {
            configure(
                engine,
                source,
                ChannelSource::SolidColor { rgba },
                SizePolicy::Fixed {
                    extent: Extent2D::new(1, 1).unwrap(),
                },
            );
        }
        let inner_channel = create(engine, MediaKind::Composition);
        configure(
            engine,
            inner_channel,
            ChannelSource::Composition {
                canvas: Extent2D::new(8, 4).unwrap(),
            },
            SizePolicy::Automatic,
        );
        let inner = vec![
            instance(red, transform([0.0, 0.0], [4.0, 4.0], 0.0, [0.0, 0.0])),
            instance(green, transform([4.0, 0.0], [4.0, 4.0], 0.0, [0.0, 0.0])),
        ];
        add(engine, inner_channel, &inner);
        let root = create(engine, MediaKind::Composition);
        configure(
            engine,
            root,
            ChannelSource::Composition {
                canvas: Extent2D::new(32, 24).unwrap(),
            },
            SizePolicy::Automatic,
        );
        let mut outer = vec![
            instance(blue, transform([0.0, 0.0], [32.0, 24.0], 0.0, [0.0, 0.0])),
            instance(inner_channel, transform([4.0, 4.0], [1.0, 1.0], 0.0, [0.0, 0.0])),
            instance(
                inner_channel,
                transform([24.0, 16.0], [-1.0, 1.0], std::f32::consts::FRAC_PI_2, [0.25, 0.75]),
            ),
            instance(inner_channel, transform([8.0, 4.0], [1.0, 1.0], 0.0, [0.0, 0.0])),
        ];
        outer[3].opacity = 0.5;
        let outer_ids = add(engine, root, &outer);
        Self {
            red,
            inner,
            outer,
            outer_ids,
            root,
            width: 32,
            height: 24,
        }
    }

    fn expected(&self, red_missing: bool) -> oracle::Image {
        let red = oracle::Image::solid(1, 1, [1.0, 0.0, 0.0, 0.5]);
        let green = oracle::Image::solid(1, 1, [0.0, 1.0, 0.0, 0.75]);
        let blue = oracle::Image::solid(1, 1, [0.0, 0.0, 1.0, 0.25]);
        let mut inner = oracle::Image::solid(8, 4, [0.0; 4]);
        if !red_missing {
            inner.draw(&red, &self.inner[0]);
        }
        inner.draw(&green, &self.inner[1]);
        let mut outer = oracle::Image::solid(self.width, self.height, [0.0; 4]);
        outer.draw(&blue, &self.outer[0]);
        for definition in &self.outer[1..] {
            outer.draw(&inner, definition);
        }
        outer
    }
}

#[test]
#[ignore = "explicit M06 native output/preview-compositor pixel acceptance on all hardware adapters"]
fn native_nested_composition_pixels_cover_transforms_blends_resize_missing_undo_and_reload() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(120)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Composition pixel/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let mut scene = Scene::new(&mut engine);
        let mut fixture = fixture::Fixture::new(adapter.id);
        for (blend, label) in [
            (BlendMode::Normal, "normal"),
            (BlendMode::Add, "add"),
            (BlendMode::Multiply, "multiply"),
            (BlendMode::Screen, "screen"),
        ] {
            scene.outer[3].blend = blend;
            set_instance(&mut engine, scene.root, scene.outer_ids[3], scene.outer[3].clone());
            fixture.publish(&engine, scene.root);
            fixture.pixels(adapter.id, &scene.expected(false), label);
        }
        scene.outer[2].transform = transform([14.0, 18.0], [2.0, 1.0], -std::f32::consts::FRAC_PI_2, [1.0, 0.0]);
        set_instance(&mut engine, scene.root, scene.outer_ids[2], scene.outer[2].clone());
        fixture.publish(&engine, scene.root);
        fixture.pixels(adapter.id, &scene.expected(false), "independent-transform");
        scene.width = 64;
        scene.height = 48;
        configure(
            &mut engine,
            scene.root,
            ChannelSource::Composition {
                canvas: Extent2D::new(64, 48).unwrap(),
            },
            SizePolicy::Automatic,
        );
        fixture.publish(&engine, scene.root);
        fixture.pixels(adapter.id, &scene.expected(false), "canvas-resize");
        edit(&mut engine, MediaEdit::RemoveMedia { media: scene.red.media });
        fixture.publish(&engine, scene.root);
        fixture.pixels(adapter.id, &scene.expected(true), "missing-shared-source");
        assert!(engine.undo().unwrap());
        fixture.publish(&engine, scene.root);
        fixture.pixels(adapter.id, &scene.expected(false), "undo-source-removal");
        drop(fixture);
        let encoded = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut restored = golden_core::app::from_sparse_project_json::<AppNode>(&encoded).unwrap();
        restored.apply_edits().unwrap();
        let mut fixture = fixture::Fixture::new(adapter.id);
        fixture.publish(&restored, scene.root);
        fixture.pixels(adapter.id, &scene.expected(false), "reload-recreate");
        drop(fixture);
        eprintln!(
            "M06 native Composition adapter={:?} description={} completed",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
