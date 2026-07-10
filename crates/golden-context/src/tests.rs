use super::*;

#[test]
fn inherited_contexts_can_accumulate_or_replace_axes_explicitly() {
    let axis_id = EntityId::new();
    let first = ContextItem {
        id: EntityId::new(),
        label: "first".into(),
        value: Value::Integer(1),
    };
    let second = ContextItem {
        id: EntityId::new(),
        label: "second".into(),
        value: Value::Integer(2),
    };
    let layers = vec![
        ContextLayer {
            mode: ContextLayerMode::Replace,
            axes: vec![ContextAxis {
                id: axis_id,
                label: "base".into(),
                items: vec![first],
            }],
        },
        ContextLayer {
            mode: ContextLayerMode::Accumulate,
            axes: vec![ContextAxis {
                id: axis_id,
                label: "inherited".into(),
                items: vec![second],
            }],
        },
    ];
    let axes = compose_context_layers(&layers);
    assert_eq!(axes.len(), 1);
    assert_eq!(axes[0].items.len(), 2);
    assert_eq!(
        LaneLayout::compile(axes, ContextLimits::default())
            .unwrap()
            .lane_count(),
        2
    );
}

fn axis(label: &str, count: usize) -> ContextAxis {
    ContextAxis {
        id: EntityId::new(),
        label: label.into(),
        items: (0..count)
            .map(|index| ContextItem {
                id: EntityId::new(),
                label: format!("{label}-{index}").into(),
                value: Value::Integer(index as i64),
            })
            .collect(),
    }
}

#[test]
fn last_axis_advances_fastest() {
    let first = axis("first", 2);
    let second = axis("second", 3);
    let expected = ContextKey(vec![first.items[1].id, second.items[0].id]);
    let layout = LaneLayout::compile(vec![first, second], ContextLimits::default()).unwrap();
    assert_eq!(layout.lane_count(), 6);
    assert_eq!(layout.key_at(3), Some(expected));
}

#[test]
fn budgets_reject_materialization_before_lane_enumeration() {
    let error = LaneLayout::compile(
        vec![axis("a", 8), axis("b", 8)],
        ContextLimits {
            maximum_items_per_axis: 8,
            maximum_lanes: 32,
        },
    )
    .unwrap_err();
    assert_eq!(error, ContextError::LaneBudgetExceeded { count: 64, maximum: 32 });
}

#[test]
fn migration_keeps_only_stable_context_key_intersections() {
    let first = axis("first", 2);
    let second = axis("second", 2);
    let original = LaneLayout::compile(vec![first.clone(), second.clone()], ContextLimits::default()).unwrap();

    let mut resized_second = second;
    resized_second.items.remove(0);
    let replacement = LaneLayout::compile(vec![first, resized_second], ContextLimits::default()).unwrap();
    assert_eq!(original.migration_to(&replacement), [(1, 0), (3, 1)]);
}
