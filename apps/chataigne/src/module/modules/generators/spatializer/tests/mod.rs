use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour, RangeConstraint},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::{
    SpatialPoint, SpatializerConfig, SpatializerDimension, SpatializerEndpointConfig,
    SpatializerMode, SpatializerModule, SpatializerSourceItem, SpatializerTargetItem,
    SpatializerValueLayout,
};

#[test]
fn spatializer_module_is_declared_under_generators_menu() {
    let item = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
        .into_iter()
        .find(|item| item.node_type == SpatializerModule::NODE_TYPE)
        .expect("Spatializer module should be exposed in the module catalog");

    assert_eq!(item.label, "Spatializer");
    assert_eq!(item.menu_path, vec!["Generators".to_string()]);
}

#[test]
fn spatializer_creates_default_source_target_and_matrix_value() {
    let (engine, module_id) = create_spatializer_module();

    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");
    let targets = find_path(&engine, module_id, "parameters/targets").expect("targets list");
    assert!(
        !engine
            .nodes
            .get(sources)
            .expect("sources list node")
            .node_data()
            .meta
            .can_be_disabled,
        "Source manager should not be disableable"
    );
    assert!(
        !engine
            .nodes
            .get(sources)
            .expect("sources list node")
            .node_data()
            .meta
            .user_permissions
            .can_edit_name,
        "Source manager should not be renameable"
    );
    assert!(
        !engine
            .nodes
            .get(targets)
            .expect("targets list node")
            .node_data()
            .meta
            .can_be_disabled,
        "Target manager should not be disableable"
    );
    assert!(
        !engine
            .nodes
            .get(targets)
            .expect("targets list node")
            .node_data()
            .meta
            .user_permissions
            .can_edit_name,
        "Target manager should not be renameable"
    );

    let source = nth_child(&engine, sources, 0).expect("default source");
    let target = nth_child(&engine, targets, 0).expect("default target");
    assert!(
        engine
            .nodes
            .get(source)
            .expect("source node")
            .node_data()
            .meta
            .can_be_disabled,
        "Source items should remain disableable"
    );
    assert!(
        engine
            .nodes
            .get(source)
            .expect("source node")
            .node_data()
            .meta
            .user_permissions
            .can_edit_name,
        "Source items should remain renameable"
    );
    assert!(find_child_by_key(&engine, source, "position_2d").is_some());
    assert!(find_child_by_key(&engine, target, "position_2d").is_some());
    assert!(find_child_by_key(&engine, source, "freeze_radius").is_none());
    assert!(find_child_by_key(&engine, target, "freeze_radius").is_some());
    assert_eq!(count_direct_children_by_decl(&engine, source, "position_2d"), 1);
    assert_eq!(count_direct_children_by_decl(&engine, target, "position_2d"), 1);
    assert_eq!(count_direct_children_by_decl(&engine, target, "freeze_radius"), 1);
    assert_eq!(count_direct_children_by_decl(&engine, source, "position_3d"), 0);
    assert_eq!(count_direct_children_by_decl(&engine, target, "position_3d"), 0);

    let value_layout =
        find_path(&engine, module_id, "parameters/value_layout").expect("value layout");
    let layout_snapshot = engine
        .nodes
        .get(value_layout)
        .and_then(|node| node.engine_param_snapshot())
        .expect("value layout should be a parameter");
    assert_eq!(
        layout_snapshot.value,
        ParamValue::Enum("sourceCentric".to_string())
    );

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let source_values = find_child_by_key(&engine, values_root, "Source").expect("source row");
    let target_value = find_child_by_key(&engine, source_values, "Target").expect("target cell");
    let snapshot = engine
        .nodes
        .get(target_value)
        .and_then(|node| node.engine_param_snapshot())
        .expect("target value should be a parameter");
    assert_eq!(snapshot.value, ParamValue::Float(1.0));
    assert_eq!(
        snapshot.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(1.0))
    );
}

#[test]
fn dimension_and_mode_changes_materialize_endpoint_parameters() {
    let (mut engine, module_id) = create_spatializer_module();
    let source = nth_child(
        &engine,
        find_path(&engine, module_id, "parameters/sources").expect("sources list"),
        0,
    )
    .expect("default source");
    let target = nth_child(
        &engine,
        find_path(&engine, module_id, "parameters/targets").expect("targets list"),
        0,
    )
    .expect("default target");

    let dimensions = find_path(&engine, module_id, "parameters/dimensions").expect("dimensions");
    set_param(&mut engine, dimensions, ParamValue::Enum("3d".to_string()));
    stabilize(&mut engine);

    assert!(find_child_by_key(&engine, source, "position_2d").is_none());
    assert!(find_child_by_key(&engine, target, "position_2d").is_none());
    assert!(find_child_by_key(&engine, source, "position_3d").is_some());
    assert!(find_child_by_key(&engine, target, "position_3d").is_some());
    assert!(find_child_by_key(&engine, target, "freeze_radius").is_some());
    assert_eq!(count_direct_children_by_decl(&engine, source, "position_3d"), 1);
    assert_eq!(count_direct_children_by_decl(&engine, target, "position_3d"), 1);

    let mode = find_path(&engine, module_id, "parameters/mode").expect("mode");
    set_param(
        &mut engine,
        mode,
        ParamValue::Enum("sourceRadius".to_string()),
    );
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, source, "radius").is_some());
    assert!(find_child_by_key(&engine, target, "radius").is_none());
    assert!(find_child_by_key(&engine, target, "freeze_radius").is_none());

    set_param(&mut engine, mode, ParamValue::Enum("overlap".to_string()));
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, source, "radius").is_some());
    assert!(find_child_by_key(&engine, target, "radius").is_some());
    assert!(find_child_by_key(&engine, target, "freeze_radius").is_none());
}

#[test]
fn adding_sources_expands_source_centric_value_rows() {
    let (mut engine, module_id) = create_spatializer_module();
    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");

    engine.add_user_item(SpatializerSourceItem::new().into(), Some(sources));
    engine
        .apply_edits()
        .expect("second source should attach under sources");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    assert_eq!(child_count(&engine, values_root), 2);
    for source_index in 0..2 {
        let source_values = nth_child(&engine, values_root, source_index).expect("source row");
        assert_eq!(child_count(&engine, source_values), 1);
    }
}

#[test]
fn value_layout_switches_between_source_and_target_centric_trees() {
    let (mut engine, module_id) = create_spatializer_module();
    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");
    let targets = find_path(&engine, module_id, "parameters/targets").expect("targets list");

    engine.add_user_item(SpatializerSourceItem::new().into(), Some(sources));
    engine
        .apply_edits()
        .expect("second source should attach under sources");
    engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    engine
        .apply_edits()
        .expect("second target should attach under targets");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let source_values = find_child_by_key(&engine, values_root, "Source")
        .expect("source-centric default should create a source row");
    assert!(
        find_child_by_key(&engine, source_values, "Target").is_some(),
        "source-centric rows should contain target values"
    );
    assert!(
        find_child_by_key(&engine, values_root, "Target").is_none(),
        "target folders should not remain at values root in source-centric layout"
    );
    assert_eq!(child_count(&engine, values_root), 2);

    let value_layout =
        find_path(&engine, module_id, "parameters/value_layout").expect("value layout");
    set_param(
        &mut engine,
        value_layout,
        ParamValue::Enum("targetCentric".to_string()),
    );
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let target_values = find_child_by_key(&engine, values_root, "Target")
        .expect("target-centric layout should create a target row");
    assert!(
        find_child_by_key(&engine, target_values, "Source").is_some(),
        "target-centric rows should contain source values"
    );
    assert!(
        find_child_by_key(&engine, values_root, "Source").is_none(),
        "source folders should be removed from values root in target-centric layout"
    );
    assert_eq!(child_count(&engine, values_root), 2);
    assert_eq!(child_count(&engine, target_values), 2);

    set_param(
        &mut engine,
        value_layout,
        ParamValue::Enum("sourceCentric".to_string()),
    );
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let source_values = find_child_by_key(&engine, values_root, "Source")
        .expect("source-centric layout should be restored");
    assert!(
        find_child_by_key(&engine, source_values, "Target").is_some(),
        "restored source-centric rows should contain target values"
    );
    assert!(
        find_child_by_key(&engine, values_root, "Target").is_none(),
        "target folders should be removed again after switching back"
    );
    assert_eq!(child_count(&engine, values_root), 2);
    assert_eq!(child_count(&engine, source_values), 2);
}

#[test]
fn value_layout_parser_accepts_label_tokens() {
    assert_eq!(
        super::parse_spatializer_value_layout("Target Centric"),
        SpatializerValueLayout::TargetCentric
    );
    assert_eq!(
        super::parse_spatializer_value_layout("target_centric"),
        SpatializerValueLayout::TargetCentric
    );
    assert_eq!(
        super::parse_spatializer_value_layout("source centric"),
        SpatializerValueLayout::SourceCentric
    );
}

#[test]
fn endpoint_renames_update_generated_value_names_without_reload() {
    let (mut engine, module_id) = create_spatializer_module();
    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");
    let targets = find_path(&engine, module_id, "parameters/targets").expect("targets list");
    let source = nth_child(&engine, sources, 0).expect("default source");
    let target = nth_child(&engine, targets, 0).expect("default target");

    rename_node(&mut engine, source, "Projector");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let source_values = find_child_by_key(&engine, values_root, "Projector")
        .expect("source-centric row should follow source rename");
    assert!(
        find_child_by_key(&engine, values_root, "Source").is_none(),
        "old source value row label should be replaced"
    );
    assert!(
        find_child_by_key(&engine, source_values, "Target").is_some(),
        "source-centric row should still contain target values"
    );

    rename_node(&mut engine, target, "Screen");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let source_values = find_child_by_key(&engine, values_root, "Projector")
        .expect("renamed source row should remain");
    assert!(
        find_child_by_key(&engine, source_values, "Screen").is_some(),
        "source-centric value should follow target rename"
    );
    assert!(
        find_child_by_key(&engine, source_values, "Target").is_none(),
        "old target value label should be replaced"
    );

    let value_layout =
        find_path(&engine, module_id, "parameters/value_layout").expect("value layout");
    set_param(
        &mut engine,
        value_layout,
        ParamValue::Enum("targetCentric".to_string()),
    );
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let target_values = find_child_by_key(&engine, values_root, "Screen")
        .expect("target-centric row should use target label");
    assert!(
        find_child_by_key(&engine, target_values, "Projector").is_some(),
        "target-centric row should contain source values"
    );

    rename_node(&mut engine, source, "Camera");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let target_values = find_child_by_key(&engine, values_root, "Screen")
        .expect("target row should remain after source rename");
    assert!(
        find_child_by_key(&engine, target_values, "Camera").is_some(),
        "target-centric source value should follow source rename"
    );
    assert!(
        find_child_by_key(&engine, target_values, "Projector").is_none(),
        "old source value label should be replaced"
    );

    rename_node(&mut engine, target, "Wall");
    stabilize(&mut engine);

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let target_values = find_child_by_key(&engine, values_root, "Wall")
        .expect("target-centric row should follow target rename");
    assert!(
        find_child_by_key(&engine, values_root, "Screen").is_none(),
        "old target row label should be replaced"
    );
    assert!(
        find_child_by_key(&engine, target_values, "Camera").is_some(),
        "renamed target row should still contain source values"
    );
}

#[test]
fn voronoi_values_update_when_source_position_changes() {
    let (mut engine, module_id) = create_spatializer_module();
    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");
    let targets = find_path(&engine, module_id, "parameters/targets").expect("targets list");
    let source = nth_child(&engine, sources, 0).expect("source");

    engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    engine
        .apply_edits()
        .expect("second target should attach under targets");
    stabilize(&mut engine);

    let target_a = nth_child(&engine, targets, 0).expect("first target");
    let target_b = nth_child(&engine, targets, 1).expect("second target");
    set_position_2d(&mut engine, target_a, 0.0, 0.0);
    set_position_2d(&mut engine, target_b, 10.0, 0.0);
    set_position_2d(&mut engine, source, 0.0, 0.0);
    stabilize(&mut engine);

    assert_eq!(source_target_values(&engine, module_id, 0), vec![1.0, 0.0]);

    set_position_2d(&mut engine, source, 10.0, 0.0);
    stabilize(&mut engine);

    assert_eq!(source_target_values(&engine, module_id, 0), vec![0.0, 1.0]);

    set_position_2d(&mut engine, source, 5.0, 0.0);
    stabilize(&mut engine);

    assert_eq!(source_target_values(&engine, module_id, 0), vec![0.5, 0.5]);
}

#[test]
fn voronoi_weights_do_not_jump_when_source_crosses_target_cell_boundary() {
    let left_target = test_endpoint(NodeId(20), "Left", SpatialPoint::new(0.0, 0.0, 0.0), 1.0);
    let right_target =
        test_endpoint(NodeId(21), "Right", SpatialPoint::new(10.0, 0.0, 0.0), 1.0);
    let top_target = test_endpoint(NodeId(22), "Top", SpatialPoint::new(5.0, 8.0, 0.0), 1.0);
    let before_source =
        test_endpoint(NodeId(10), "Source", SpatialPoint::new(4.999, 1.0, 0.0), 1.0);
    let after_source =
        test_endpoint(NodeId(11), "Source", SpatialPoint::new(5.001, 1.0, 0.0), 1.0);

    let before = test_config(
        SpatializerMode::Voronoi,
        vec![before_source.clone()],
        vec![
            left_target.clone(),
            right_target.clone(),
            top_target.clone(),
        ],
    );
    let after = test_config(
        SpatializerMode::Voronoi,
        vec![after_source.clone()],
        vec![
            left_target.clone(),
            right_target.clone(),
            top_target.clone(),
        ],
    );

    let before_values = [
        super::spatializer_value(&before, &left_target, &before_source),
        super::spatializer_value(&before, &right_target, &before_source),
        super::spatializer_value(&before, &top_target, &before_source),
    ];
    let after_values = [
        super::spatializer_value(&after, &left_target, &after_source),
        super::spatializer_value(&after, &right_target, &after_source),
        super::spatializer_value(&after, &top_target, &after_source),
    ];

    assert!((before_values.iter().sum::<f64>() - 1.0).abs() < 0.000_001);
    assert!((after_values.iter().sum::<f64>() - 1.0).abs() < 0.000_001);
    for (before_value, after_value) in before_values.iter().zip(after_values.iter()) {
        assert!(
            (before_value - after_value).abs() < 0.002,
            "Voronoi weight jumped from {before_value} to {after_value}"
        );
    }
}

#[test]
fn sparse_reload_preserves_single_declared_parameter_block() {
    let (mut engine, module_id) = create_spatializer_module();
    let parameters = find_path(&engine, module_id, "parameters").expect("parameters root");
    let sources = find_path(&engine, module_id, "parameters/sources").expect("sources list");
    let targets = find_path(&engine, module_id, "parameters/targets").expect("targets list");
    let source = nth_child(&engine, sources, 0).expect("default source");
    let source_position = find_child_by_key(&engine, source, "position_2d").expect("source position");
    set_param(&mut engine, source_position, ParamValue::Vec2(3.6, 1.8));

    engine.add_user_item(SpatializerTargetItem::new().into(), Some(targets));
    engine
        .apply_edits()
        .expect("second target should attach under targets");
    stabilize(&mut engine);
    let second_target = nth_child(&engine, targets, 1).expect("second target");
    let target_position =
        find_child_by_key(&engine, second_target, "position_2d").expect("target position");
    set_param(&mut engine, target_position, ParamValue::Vec2(2.6, 2.5));
    stabilize(&mut engine);

    assert_single_spatializer_parameter_block(&engine, parameters);

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("sparse project should encode");
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("sparse project should decode");
    stabilize(&mut loaded);
    let loaded_module = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Spatializer module should reload");
    let loaded_parameters =
        find_path(&loaded, loaded_module, "parameters").expect("parameters should reload");

    assert_single_spatializer_parameter_block(&loaded, loaded_parameters);
}

#[test]
fn spatializer_math_covers_all_modes() {
    let source = test_endpoint(NodeId(10), "Source", SpatialPoint::new(0.0, 0.0, 0.0), 10.0);
    let target = test_endpoint(NodeId(20), "Target", SpatialPoint::new(3.0, 4.0, 0.0), 10.0);

    let moving_source =
        test_endpoint(NodeId(11), "Source", SpatialPoint::new(3.0, 4.0, 0.0), 1.0);
    let left_target = test_endpoint(NodeId(23), "Left", SpatialPoint::new(0.0, 0.0, 0.0), 1.0);
    let right_target =
        test_endpoint(NodeId(24), "Right", SpatialPoint::new(20.0, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![moving_source.clone()],
        vec![left_target.clone(), right_target.clone()],
    );
    let left_value = super::spatializer_value(&config, &left_target, &moving_source);
    let right_value = super::spatializer_value(&config, &right_target, &moving_source);
    assert!(left_value > right_value);
    assert!(right_value > 0.0);
    assert!((left_value + right_value - 1.0).abs() < 0.000_001);

    let exact_source =
        test_endpoint(NodeId(12), "Source", SpatialPoint::new(0.0, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![exact_source.clone()],
        vec![left_target.clone(), right_target.clone()],
    );
    assert_eq!(super::spatializer_value(&config, &left_target, &exact_source), 1.0);
    assert_eq!(
        super::spatializer_value(&config, &right_target, &exact_source),
        0.0
    );

    let middle_source =
        test_endpoint(NodeId(13), "Source", SpatialPoint::new(10.0, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![middle_source.clone()],
        vec![left_target.clone(), right_target.clone()],
    );
    assert_eq!(
        super::spatializer_value(&config, &left_target, &middle_source),
        0.5
    );
    assert_eq!(
        super::spatializer_value(&config, &right_target, &middle_source),
        0.5
    );

    let center_target = test_endpoint(
        NodeId(25),
        "Center",
        SpatialPoint::new(10.0, 0.0, 0.0),
        1.0,
    );
    let near_left_source =
        test_endpoint(NodeId(14), "Source", SpatialPoint::new(2.5, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![near_left_source.clone()],
        vec![
            left_target.clone(),
            center_target.clone(),
            right_target.clone(),
        ],
    );
    let left_value = super::spatializer_value(&config, &left_target, &near_left_source);
    let center_value = super::spatializer_value(&config, &center_target, &near_left_source);
    let right_value = super::spatializer_value(&config, &right_target, &near_left_source);
    assert!(left_value > 0.0);
    assert!(center_value > 0.0);
    assert_eq!(right_value, 0.0);
    assert!((left_value + center_value + right_value - 1.0).abs() < 0.000_001);

    let frozen_target = SpatializerEndpointConfig {
        freeze_radius: 3.0,
        ..left_target.clone()
    };
    let frozen_source =
        test_endpoint(NodeId(15), "Source", SpatialPoint::new(2.0, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![frozen_source.clone()],
        vec![frozen_target.clone(), center_target.clone()],
    );
    assert_eq!(
        super::spatializer_value(&config, &frozen_target, &frozen_source),
        1.0
    );
    assert_eq!(
        super::spatializer_value(&config, &center_target, &frozen_source),
        0.0
    );

    let thawed_source =
        test_endpoint(NodeId(16), "Source", SpatialPoint::new(4.0, 0.0, 0.0), 1.0);
    let config = test_config(
        SpatializerMode::Voronoi,
        vec![thawed_source.clone()],
        vec![frozen_target.clone(), center_target.clone()],
    );
    let frozen_value = super::spatializer_value(&config, &frozen_target, &thawed_source);
    let center_value = super::spatializer_value(&config, &center_target, &thawed_source);
    assert!(frozen_value > 0.0);
    assert!(center_value > 0.0);
    assert!((frozen_value + center_value - 1.0).abs() < 0.000_001);

    let config = test_config(
        SpatializerMode::TargetRadius,
        vec![source.clone()],
        vec![target.clone()],
    );
    assert!((super::spatializer_value(&config, &target, &source) - 0.5).abs() < 0.000_001);

    let config = test_config(
        SpatializerMode::SourceRadius,
        vec![source.clone()],
        vec![target.clone()],
    );
    assert!((super::spatializer_value(&config, &target, &source) - 0.5).abs() < 0.000_001);

    let overlap_source =
        test_endpoint(NodeId(12), "Source", SpatialPoint::new(0.0, 0.0, 0.0), 5.0);
    let overlap_target =
        test_endpoint(NodeId(21), "Target", SpatialPoint::new(6.0, 0.0, 0.0), 3.0);
    let config = test_config(
        SpatializerMode::Overlap,
        vec![overlap_source.clone()],
        vec![overlap_target.clone()],
    );
    assert!(
        (super::spatializer_value(&config, &overlap_target, &overlap_source) - (1.0 / 3.0))
            .abs()
            < 0.000_001
    );

    let contained_target =
        test_endpoint(NodeId(22), "Target", SpatialPoint::new(3.0, 0.0, 0.0), 2.0);
    let config = test_config(
        SpatializerMode::Overlap,
        vec![source.clone()],
        vec![contained_target.clone()],
    );
    assert_eq!(super::spatializer_value(&config, &contained_target, &source), 1.0);
}

#[test]
fn spatializer_supported_scale_uses_sparse_delaunay_topology() {
    const TARGET_SIDE: usize = 32;
    const TARGET_COUNT: usize = TARGET_SIDE * TARGET_SIDE;
    const SOURCE_COUNT: usize = 512;

    let targets: Vec<_> = (0..TARGET_COUNT)
        .map(|index| {
            let x = (index % TARGET_SIDE) as f64;
            let y = (index / TARGET_SIDE) as f64 + x * 0.000_1;
            test_endpoint(
                NodeId(100_000 + index as u64),
                "Target",
                SpatialPoint::new(x, y, 0.0),
                1.0,
            )
        })
        .collect();
    let sources: Vec<_> = (0..SOURCE_COUNT)
        .map(|index| {
            let x = ((index * 17) % (TARGET_SIDE * 10)) as f64 / 10.0;
            let y = ((index * 29) % (TARGET_SIDE * 10)) as f64 / 10.0;
            test_endpoint(
                NodeId(200_000 + index as u64),
                "Source",
                SpatialPoint::new(x, y, 0.0),
                1.0,
            )
        })
        .collect();
    let config = test_config(SpatializerMode::Voronoi, sources.clone(), targets);

    let topology = super::VoronoiTopology::compile(&config);
    let directed_edge_count: usize = topology
        .target_indices
        .iter()
        .map(|target_index| topology.neighbours(*target_index).len())
        .sum();
    assert_eq!(topology.target_indices.len(), TARGET_COUNT);
    assert!(
        directed_edge_count <= TARGET_COUNT * 6,
        "planar topology should stay linear at the supported target scale; got {directed_edge_count} directed edges"
    );
    assert!(
        topology
            .target_indices
            .iter()
            .all(|target_index| !topology.neighbours(*target_index).is_empty())
    );

    let values = super::spatializer_values_by_target(&config);
    for source in sources {
        let total: f64 = values
            .values()
            .filter_map(|target_values| target_values.get(&source.item_id))
            .sum();
        assert!(
            (total - 1.0).abs() < 0.000_001,
            "source {} should retain normalized Voronoi influence, got {total}",
            source.item_id.0
        );
    }
}

#[test]
fn spatializer_script_template_documents_value_matrix_surface() {
    let config = crate::app::module::script_api::module_script_config(SpatializerModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("Spatializer module script template should resolve to inline source");
    };

    assert!(source.contains("Spatializer module values"));
    assert!(source.contains("local.values"));
}

fn test_config(
    mode: SpatializerMode,
    sources: Vec<SpatializerEndpointConfig>,
    targets: Vec<SpatializerEndpointConfig>,
) -> SpatializerConfig {
    SpatializerConfig {
        dimension: SpatializerDimension::Two,
        mode,
        value_layout: SpatializerValueLayout::SourceCentric,
        sources,
        targets,
    }
}

fn test_endpoint(
    item_id: NodeId,
    label: &str,
    position: SpatialPoint,
    radius: f64,
) -> SpatializerEndpointConfig {
    SpatializerEndpointConfig {
        item_id,
        item_uuid: golden_core::node::NodeUuid(uuid::Uuid::from_u128(item_id.0 as u128 + 1)),
        value_decl_id: format!("test_{}", item_id.0),
        label: label.to_string(),
        enabled: true,
        position,
        radius,
        freeze_radius: 0.0,
    }
}

fn create_spatializer_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SpatializerModule::create().into(), None);
    stabilize(&mut engine);
    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Spatializer module should be attached under root");
    (engine, module_id)
}

fn stabilize(engine: &mut crate::app::AppEngine) {
    for _ in 0..10 {
        engine.apply_edits().expect("edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("stabilization inbox should dispatch");
    }
    engine.resolve().expect("schedule should resolve");
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("parameter edit should apply");
}

fn set_position_2d(engine: &mut crate::app::AppEngine, item: NodeId, x: f64, y: f64) {
    let position = find_child_by_key(engine, item, "position_2d").expect("2D position");
    set_param(engine, position, ParamValue::Vec2(x, y));
}

fn source_target_values(
    engine: &crate::app::AppEngine,
    module_id: NodeId,
    source_index: usize,
) -> Vec<f64> {
    let values_root = find_path(engine, module_id, "values").expect("values root");
    let source_folder_id =
        nth_child(engine, values_root, source_index).expect("source-centric source row");
    let source_folder = engine.nodes.get(source_folder_id).expect("source row");
    let mut values = Vec::new();
    let mut current = source_folder.node_data().first_child;
    while let Some(target_value) = current {
        let target_value_node = engine.nodes.get(target_value).expect("target value child");
        let value = target_value_node
            .engine_param_snapshot()
            .and_then(|snapshot| snapshot.value.as_float())
            .expect("target value should be a float");
        values.push(value);
        current = target_value_node.node_data().next_sibling;
    }
    values
}

#[allow(dead_code)]
fn patch_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("enabled patch should apply");
}

fn rename_node(engine: &mut crate::app::AppEngine, node: NodeId, label: &str) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some(label.to_string()),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("rename patch should apply");
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    for segment in path.split('/') {
        if segment.trim().is_empty() {
            continue;
        }
        current = find_child_by_key(engine, current, segment)?;
    }
    Some(current)
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        let meta = &child.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn child_count(engine: &crate::app::AppEngine, parent: NodeId) -> usize {
    let mut current = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    let mut count = 0usize;
    while let Some(child_id) = current {
        count += 1;
        current = engine
            .nodes
            .get(child_id)
            .and_then(|node| node.node_data().next_sibling);
    }
    count
}

fn count_direct_children_by_decl(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> usize {
    let mut current = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    let mut count = 0usize;
    while let Some(child_id) = current {
        let Some(child) = engine.nodes.get(child_id) else {
            break;
        };
        if child.node_data().meta.decl_id.0 == decl_id {
            count += 1;
        }
        current = child.node_data().next_sibling;
    }
    count
}

fn assert_single_spatializer_parameter_block(engine: &crate::app::AppEngine, parameters: NodeId) {
    let decls = direct_child_decl_ids(engine, parameters);
    for decl_id in ["dimensions", "mode", "value_layout", "sources", "targets"] {
        assert_eq!(
            count_direct_children_by_decl_tail(engine, parameters, decl_id),
            1,
            "parameters should contain one '{decl_id}' child after persistence; children were {decls:?}"
        );
    }
}

fn count_direct_children_by_decl_tail(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    decl_tail: &str,
) -> usize {
    let mut current = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    let mut count = 0usize;
    while let Some(child_id) = current {
        let Some(child) = engine.nodes.get(child_id) else {
            break;
        };
        if child.node_data().meta.decl_id.0.rsplit('/').next() == Some(decl_tail) {
            count += 1;
        }
        current = child.node_data().next_sibling;
    }
    count
}

fn direct_child_decl_ids(engine: &crate::app::AppEngine, parent: NodeId) -> Vec<String> {
    let mut current = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    let mut decls = Vec::new();
    while let Some(child_id) = current {
        let Some(child) = engine.nodes.get(child_id) else {
            break;
        };
        decls.push(child.node_data().meta.decl_id.0.clone());
        current = child.node_data().next_sibling;
    }
    decls
}

fn nth_child(engine: &crate::app::AppEngine, parent: NodeId, index: usize) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    let mut current_index = 0usize;
    while let Some(child_id) = current {
        if current_index == index {
            return Some(child_id);
        }
        current_index += 1;
        current = engine.nodes.get(child_id)?.node_data().next_sibling;
    }
    None
}
