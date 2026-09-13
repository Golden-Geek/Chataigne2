use chataigne_alchemist::{ChannelDescriptor, ChannelLayout, CompileCtx, StableRef, ValueTypeId};
use chataigne_state_machine::{
    alchemist::{shared_node_registry, shared_value_type_registry},
    ValueLaneKey,
};
use golden_core::parameter::{ParamValue, Parameter};

use super::super::palette::{filter_palette_items_for_layout, trigger_filter_palette_items};
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
        items
            .iter()
            .any(|item| item.node_type.starts_with(&format!("alchemist_anode:{suffix}@managed/")))
    };
    let float = filter_palette_items_for_layout(&layout(&["float"]), &ctx);
    assert!(contains(&float, "remap"));
    let math_each = float
        .iter()
        .find(|item| item.label == "Math (Each)")
        .expect("single float should offer elementwise Math");
    assert_eq!(math_each.node_type, "alchemist_anode:math@managed/1");
    assert!(!float.iter().any(|item| item.label == "Math (Combine)"));
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
    assert!(two_floats.iter().any(|item| item.label == "Math (Combine)"));

    let three_floats = filter_palette_items_for_layout(&layout(&["float", "float", "float"]), &ctx);
    assert!(contains(&three_floats, "pack_vec3"));
    assert!(!three_floats.iter().any(|item| item.label == "Math (Combine)"));

    let color = filter_palette_items_for_layout(&layout(&["color"]), &ctx);
    assert!(contains(&color, "extract_color"));

    let trigger = trigger_filter_palette_items(&ctx);
    assert!(contains(&trigger, "condition_gate"));
    assert!(!contains(&trigger, "remap"));
}
