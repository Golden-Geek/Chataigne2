use super::*;

#[test]
fn user_context_nearest_scope_wins_and_reports_type_mismatch() {
    let mut contexts = UserContextRegistry::new();
    let owner_outer = NodeId(1);
    let owner_inner = NodeId(2);
    let consumer = NodeId(3);

    contexts.ensure_scope(owner_outer);
    contexts.ensure_scope(owner_inner);
    contexts
        .upsert_entry(owner_outer, "tempo", NodeId(10), UserContextValueType::Float)
        .expect("outer entry should be inserted");
    contexts
        .upsert_entry(owner_inner, "tempo", NodeId(11), UserContextValueType::Int)
        .expect("inner entry should be inserted");

    let mut parents = HashMap::<NodeId, Option<NodeId>>::new();
    parents.insert(consumer, Some(owner_inner));
    parents.insert(owner_inner, Some(owner_outer));
    parents.insert(owner_outer, None);

    let resolved = contexts.resolve_symbol(consumer, "tempo", None, |node| parents.get(&node).copied().flatten());
    assert!(
        matches!(resolved, UserContextLookup::Resolved(UserContextResolution { entry_param, lexical_depth: 1, .. }) if entry_param == NodeId(11))
    );

    let mismatch = contexts.resolve_symbol(consumer, "tempo", Some(UserContextValueType::Float), |node| {
        parents.get(&node).copied().flatten()
    });
    assert!(matches!(
        mismatch,
        UserContextLookup::TypeMismatch(UserContextTypeMismatch {
            found: UserContextValueType::Int,
            expected: UserContextValueType::Float,
            lexical_depth: 1,
            ..
        })
    ));
}

#[test]
fn dynamic_context_lane_index_flattens_nested_dimensions() {
    let mut dynamic = DynamicContext::new();
    dynamic.push_dim(DynamicContextDimId::new("outer"), 3, 8, None);
    dynamic.push_dim(DynamicContextDimId::new("inner"), 4, 8, None);

    let dims = vec![DynamicContextDimId::new("outer"), DynamicContextDimId::new("inner")];
    let lane = compute_dynamic_context_lane_index(&dims, &[8, 8], &dynamic).expect("lane should compute");
    assert_eq!(lane, 3 + 4 * 8);
}
