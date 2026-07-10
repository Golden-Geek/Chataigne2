use crate::{EnterPolicy, HistoryPolicy, LifecycleEvent, Statechart};

#[test]
fn leaf_transition_emits_exit_and_enter() {
    let mut chart = Statechart::new();
    let first = chart.add_leaf(chart.root_region, "First").unwrap();
    let second = chart.add_leaf(chart.root_region, "Second").unwrap();
    chart.set_initial(chart.root_region, first).unwrap();
    let transition = chart.add_transition(first, second, 0).unwrap();
    chart.initialize().unwrap();

    let outcome = chart.step(|candidate| candidate.id == transition).unwrap().unwrap();

    assert_eq!(outcome.exited, vec![first]);
    assert_eq!(outcome.entered, vec![second]);
    assert_eq!(
        outcome.lifecycle,
        vec![LifecycleEvent::Exit(first), LifecycleEvent::Enter(second)]
    );
}

#[test]
fn composite_initial_child_is_entered() {
    let mut chart = Statechart::new();
    let (parent, child_region) = chart
        .add_composite(
            chart.root_region,
            "Parent",
            HistoryPolicy::None,
            EnterPolicy::InitialChild,
        )
        .unwrap();
    let child = chart.add_leaf(child_region, "Child").unwrap();
    chart.set_initial(chart.root_region, parent).unwrap();
    chart.set_initial(child_region, child).unwrap();

    let lifecycle = chart.initialize().unwrap();

    assert_eq!(
        lifecycle,
        vec![LifecycleEvent::Enter(parent), LifecycleEvent::Enter(child)]
    );
    assert!(chart.active.active_scopes.contains(&parent));
    assert!(chart.active.active_scopes.contains(&child));
}

#[test]
fn deepest_transition_wins_before_priority() {
    let mut chart = Statechart::new();
    let (parent, child_region) = chart
        .add_composite(
            chart.root_region,
            "Parent",
            HistoryPolicy::None,
            EnterPolicy::InitialChild,
        )
        .unwrap();
    let child = chart.add_leaf(child_region, "Child").unwrap();
    let outer = chart.add_leaf(chart.root_region, "Outer").unwrap();
    chart.set_initial(chart.root_region, parent).unwrap();
    chart.set_initial(child_region, child).unwrap();
    let parent_transition = chart.add_transition(parent, outer, 100).unwrap();
    let child_transition = chart.add_transition(child, outer, 0).unwrap();
    chart.initialize().unwrap();

    let outcome = chart.step(|_| true).unwrap().unwrap();

    assert_eq!(outcome.transition, child_transition);
    assert_ne!(outcome.transition, parent_transition);
}

#[test]
fn last_active_child_history_is_restored() {
    let mut chart = Statechart::new();
    let (parent, child_region) = chart
        .add_composite(
            chart.root_region,
            "Parent",
            HistoryPolicy::Shallow,
            EnterPolicy::LastActiveChild,
        )
        .unwrap();
    let first = chart.add_leaf(child_region, "First").unwrap();
    let second = chart.add_leaf(child_region, "Second").unwrap();
    let outside = chart.add_leaf(chart.root_region, "Outside").unwrap();
    chart.set_initial(chart.root_region, parent).unwrap();
    chart.set_initial(child_region, first).unwrap();
    let to_second = chart.add_transition(first, second, 0).unwrap();
    let leave = chart.add_transition(parent, outside, 0).unwrap();
    let return_to_parent = chart.add_transition(outside, parent, 0).unwrap();
    chart.initialize().unwrap();
    chart.step(|transition| transition.id == to_second).unwrap();
    chart.step(|transition| transition.id == leave).unwrap();
    chart.step(|transition| transition.id == return_to_parent).unwrap();

    assert!(chart.active.active_scopes.contains(&second));
}
