use chataigne_alchemist::{
    ANodeTypeId, ChannelDescriptor, ChannelLayout, CompileCtx, ManagedFilterValueMode, StableRef, ValueTypeId,
    configured_managed_variant,
};
use chataigne_state_machine::{
    ValueLaneKey,
    alchemist::{shared_node_registry, shared_value_type_registry},
    validate_mapping_filter_application,
};
use golden_core::parameter::{ParamValue, Parameter};

use super::super::palette::{
    filter_palette_items_for_layout, filter_palette_items_for_mode, trigger_filter_palette_items,
};
use crate::app::systems_alchemist_formula::create_anode_user_item_tree;

fn layout(value_types: &[&str]) -> ChannelLayout {
    ChannelLayout::new(
        value_types
            .iter()
            .enumerate()
            .map(|(index, value_type)| {
                let id = format!("channel:{index}");
                ChannelDescriptor::input(
                    ValueLaneKey::new(&id).unwrap(),
                    &id,
                    StableRef::new(ValueTypeId::new("source"), id.clone()),
                    Some(ValueTypeId::new(*value_type)),
                )
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn backend_filter_palette_materializes_executable_variants_for_the_current_layout() {
    let ctx = CompileCtx {
        value_types: shared_value_type_registry(),
        nodes: shared_node_registry(),
        properties: None,
    };
    let contains = |items: &[golden_core::node::UserCreatableItem], suffix: &str| {
        items.iter().any(|item| {
            item.node_type
                .starts_with(&format!("alchemist_anode:{suffix}@managed/"))
        })
    };
    let float = filter_palette_items_for_layout(&layout(&["float"]), &ctx);
    let timed = ctx.nodes.get(&ANodeTypeId::new("timed_delay")).unwrap();
    let timed_instance = configured_managed_variant(timed.as_ref(), 0, None).unwrap();
    let timed_result = validate_mapping_filter_application(&timed_instance, &layout(&["float"]), &ctx);
    assert!(
        timed_result.is_ok(),
        "Timed Delay cannot compile for float: {timed_result:?}"
    );
    assert!(contains(&float, "remap"));
    for kind in [
        "curve_remap",
        "timed_delay",
        "delay_one_tick",
        "threshold",
        "gradient_sampler",
        "convert_to_int",
        "convert_to_string",
    ] {
        assert!(contains(&float, kind), "float palette is missing {kind}");
    }
    let math_each = float
        .iter()
        .find(|item| item.label == "Math (Each)")
        .expect("single float should offer elementwise Math");
    assert_eq!(math_each.node_type, "alchemist_anode:math@managed/1");
    assert!(!float.iter().any(|item| item.label == "Math (Combine)"));
    assert!(!contains(&float, "sum"));
    assert!(!contains(&float, "average"));
    let math_tree = create_anode_user_item_tree(&math_each.node_type).unwrap();
    let config = math_tree
        .children
        .iter()
        .find(|child| child.node.node_data().meta.decl_id.0 == "config")
        .unwrap();
    let application = config
        .children
        .iter()
        .find(|child| child.node.node_data().meta.decl_id.0 == "config/application")
        .unwrap();
    let value = &application.node.as_any().downcast_ref::<Parameter>().unwrap().value;
    assert_eq!(value, &ParamValue::Enum("each".into()));
    assert!(!contains(&float, "pack_vec3"));
    assert!(!contains(&float, "extract_color"));

    let bool_layout = filter_palette_items_for_layout(&layout(&["bool"]), &ctx);
    assert!(!contains(&bool_layout, "remap"));
    assert!(!contains(&bool_layout, "pack_vec3"));

    let two_floats = filter_palette_items_for_layout(&layout(&["float", "float"]), &ctx);
    assert!(contains(&two_floats, "sum"));
    assert!(contains(&two_floats, "average"));
    for kind in [
        "product",
        "minimum",
        "maximum",
        "difference",
        "distance",
        "pack_vec2",
        "compare",
    ] {
        assert!(contains(&two_floats, kind), "two-float palette is missing {kind}");
    }

    let three_floats = filter_palette_items_for_layout(&layout(&["float", "float", "float"]), &ctx);
    assert!(contains(&three_floats, "pack_vec3"));
    assert!(!contains(&three_floats, "distance"));
    for (node_type, variant) in [("math", 0), ("sum", 0), ("average", 0)] {
        let expected_type = format!("alchemist_anode:{node_type}@managed/{variant}/inputs/3");
        let item = three_floats
            .iter()
            .find(|item| item.node_type == expected_type)
            .unwrap();
        if node_type == "math" {
            assert_eq!(item.label, "Math (Combine)");
        }
        let tree = create_anode_user_item_tree(&item.node_type).unwrap();
        let config = tree
            .children
            .iter()
            .find(|child| child.node.node_data().meta.decl_id.0 == "config")
            .unwrap();
        let count = config
            .children
            .iter()
            .find(|child| child.node.node_data().meta.decl_id.0 == "config/num_inputs")
            .unwrap();
        assert_eq!(
            count.node.as_any().downcast_ref::<Parameter>().unwrap().value,
            ParamValue::Int(3)
        );
        let inputs = tree
            .children
            .iter()
            .find(|child| child.node.node_data().meta.decl_id.0 == "inputs")
            .unwrap();
        assert_eq!(inputs.children.len(), 3);
    }

    let mixed = filter_palette_items_for_layout(&layout(&["float", "bool", "string"]), &ctx);
    assert!(!contains(&mixed, "remap"));
    assert!(!contains(&mixed, "sum"));
    assert!(!contains(&mixed, "average"));
    assert!(!contains(&mixed, "math"));
    assert!(contains(&mixed, "convert_tuple"));
    let routed = filter_palette_items_for_mode(
        &layout(&["float", "bool", "string"]),
        &ctx,
        ManagedFilterValueMode::Routed,
    );
    assert!(contains(&routed, "remap"));

    let color = filter_palette_items_for_layout(&layout(&["color"]), &ctx);
    assert!(contains(&color, "extract_color"));
    assert!(contains(&color, "convert_compound"));
    let vec2 = filter_palette_items_for_layout(&layout(&["vec2"]), &ctx);
    assert!(contains(&vec2, "extract_vec2"));
    assert!(contains(&vec2, "convert_compound"));

    let trigger = trigger_filter_palette_items(&ctx);
    assert!(contains(&trigger, "condition_gate"));
    assert!(!contains(&trigger, "remap"));

    assert!(create_anode_user_item_tree("alchemist_anode:sum@managed/0/inputs/65").is_none());
    assert!(create_anode_user_item_tree("alchemist_anode:remap@managed/0/inputs/3").is_none());
}
