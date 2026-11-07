use super::*;

fn settings(engine: &mut AppEngine, scene: &Scene, item: MatrixItemId) -> CompositeSettings {
    inspect(engine, scene.root.media)
        .items
        .into_iter()
        .find(|row| row.id == item)
        .unwrap()
        .settings
}
fn exposure(engine: &mut AppEngine, scene: &Scene, item: MatrixItemId, exposed: bool) {
    let mut value = settings(engine, scene, item);
    value.expose_channel = exposed;
    matrix(
        engine,
        scene.root.media,
        MatrixEdit::ConfigureItem { item, settings: value },
    );
}

#[test]
#[ignore = "explicit M11/8 native nested Group blending and public output exposure"]
fn native_hidden_nested_groups_keep_final_pixels_and_exact_export_recovery() {
    let hardware = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let scene = Scene::new(&mut engine);
        let inner = inspect(&mut engine, scene.root.media)
            .items
            .into_iter()
            .find(|row| row.channel == scene.group_channel)
            .unwrap()
            .id;
        let MediaEditResult::MatrixCreated { items, .. } = matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::AddGroup {
                parent: None,
                label: Some("Outer".into()),
            },
        ) else {
            panic!()
        };
        let outer = items[0];
        let MatrixItemId::Group { id: outer_id } = outer else {
            panic!()
        };
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::MoveItem {
                item: scene.outside,
                parent: Some(outer_id),
                after: None,
            },
        );
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::MoveItem {
                item: inner,
                parent: Some(outer_id),
                after: Some(scene.outside),
            },
        );
        let mut outer_settings = settings(&mut engine, &scene, outer);
        outer_settings.opacity = 0.4;
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::ConfigureItem {
                item: outer,
                settings: outer_settings,
            },
        );
        assert!(
            inspect(&mut engine, scene.root.media)
                .items
                .iter()
                .all(|row| !row.settings.expose_channel)
        );
        let MediaEditResult::ReferenceCreated { media: alias } = edit(
            &mut engine,
            MediaEdit::CreateReference {
                target: Some(scene.root.media),
                label: None,
            },
        ) else {
            panic!()
        };
        edit(
            &mut engine,
            MediaEdit::SelectReferenceChannel {
                media: alias,
                channel: Some(scene.group_channel.channel),
            },
        );
        let alias_group = mapgyver::media::resolve_source_node(&engine, alias.0).unwrap();
        let mut fixture = Fixture::new(adapter.id);
        let mut final_image = image([0.; 4]);
        for blend in [
            BlendMode::Normal,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
        ] {
            let mut value = settings(&mut engine, &scene, inner);
            value.blend = blend;
            matrix(
                &mut engine,
                scene.root.media,
                MatrixEdit::ConfigureItem {
                    item: inner,
                    settings: value,
                },
            );
            launch(&mut engine, scene.root, scene.columns[0], LaunchMode::Cut);
            let mut combined = image([0., 0., 1., 0.25]);
            draw(&mut combined, &scene.expected(false)[1], 1., blend);
            final_image = image([0.; 4]);
            draw(&mut final_image, &combined, 0.4, BlendMode::Normal);
            fixture.publish_channels(&engine, &[scene.root, scene.root], scene.root, None);
            fixture.pixels_channels(
                adapter.id,
                &[&final_image, &final_image],
                &final_image,
                &format!("hidden-nested-{blend:?}"),
            );
        }
        exposure(&mut engine, &scene, inner, true);
        fixture.publish_channels(&engine, &[alias_group, alias_group], alias_group, None);
        let [_, group_image, _] = scene.expected(false);
        fixture.pixels_channels(
            adapter.id,
            &[&group_image, &group_image],
            &group_image,
            "exposed-reference-group",
        );
        exposure(&mut engine, &scene, inner, false);
        fixture.publish_channels(&engine, &[scene.root, scene.root], scene.root, None);
        fixture.pixels_channels(
            adapter.id,
            &[&final_image, &final_image],
            &final_image,
            "hidden-again-final-preserved",
        );
        exposure(&mut engine, &scene, inner, true);
        fixture.publish_channels(&engine, &[alias_group, alias_group], alias_group, None);
        fixture.pixels_channels(
            adapter.id,
            &[&group_image, &group_image],
            &group_image,
            "same-id-reference-recovered",
        );
        drop(fixture);
        let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        launch(&mut loaded, scene.root, scene.columns[0], LaunchMode::Cut);
        let mut fixture = Fixture::new(adapter.id);
        fixture.publish_channels(&loaded, &[alias_group, alias_group], alias_group, None);
        fixture.pixels_channels(
            adapter.id,
            &[&group_image, &group_image],
            &group_image,
            "reloaded-exposed-reference",
        );
        drop(fixture);
        eprintln!(
            "M11/8 hidden nested Group/exposure adapter={:?} description={} completed",
            adapter.id, adapter.description
        );
    }
}
