use super::{
    MultiplexContextLinkTarget, MultiplexTemplateToken, MultiplexTokenSelector, UserContextMultiplexList,
    UserContextRegistry, UserContextValueType, multiplex_index_context_link_symbol, multiplex_list_context_link_symbol,
    parse_multiplex_context_link_symbol, parse_multiplex_template_token,
};
use crate::node::NodeId;

#[test]
fn multiplex_context_link_symbols_round_trip() {
    let one_based = multiplex_index_context_link_symbol("axis-a", false);
    assert_eq!(
        parse_multiplex_context_link_symbol(&one_based),
        Some(MultiplexContextLinkTarget::Index {
            axis_id: "axis-a".to_owned(),
            zero_based: false,
        })
    );

    let zero_based = multiplex_index_context_link_symbol("axis-a", true);
    assert_eq!(
        parse_multiplex_context_link_symbol(&zero_based),
        Some(MultiplexContextLinkTarget::Index {
            axis_id: "axis-a".to_owned(),
            zero_based: true,
        })
    );

    let list = multiplex_list_context_link_symbol("axis-a", "Names:localized");
    assert_eq!(
        parse_multiplex_context_link_symbol(&list),
        Some(MultiplexContextLinkTarget::List {
            axis_id: "axis-a".to_owned(),
            symbol: "Names:localized".to_owned(),
        })
    );
}

#[test]
fn multiplex_template_tokens_support_default_named_and_ordinal_axes() {
    assert_eq!(
        parse_multiplex_template_token("index"),
        Some(MultiplexTemplateToken::Index {
            zero_based: false,
            multiplex: MultiplexTokenSelector::First,
        })
    );
    assert_eq!(
        parse_multiplex_template_token("index0:2"),
        Some(MultiplexTemplateToken::Index {
            zero_based: true,
            multiplex: MultiplexTokenSelector::Ordinal(2),
        })
    );
    assert_eq!(
        parse_multiplex_template_token("index:Fixtures"),
        Some(MultiplexTemplateToken::Index {
            zero_based: false,
            multiplex: MultiplexTokenSelector::Name("Fixtures".to_owned()),
        })
    );
    assert_eq!(
        parse_multiplex_template_token("list:Names"),
        Some(MultiplexTemplateToken::List {
            multiplex: MultiplexTokenSelector::First,
            list: "Names".to_owned(),
        })
    );
    assert_eq!(
        parse_multiplex_template_token("list:Fixtures:Names"),
        Some(MultiplexTemplateToken::List {
            multiplex: MultiplexTokenSelector::Name("Fixtures".to_owned()),
            list: "Names".to_owned(),
        })
    );
}

#[test]
fn context_candidates_keep_same_named_lists_from_each_multiplex() {
    let owner = NodeId(1);
    let consumer = NodeId(2);
    let mut registry = UserContextRegistry::default();
    registry.ensure_scope(owner);

    for (axis, list) in [("axis-a", NodeId(10)), ("axis-b", NodeId(20))] {
        let multiplex = UserContextMultiplexList {
            multiplex: NodeId(list.0 - 1),
            list,
            axis_id: axis.to_owned(),
            index_link_symbol: multiplex_index_context_link_symbol(axis, false),
            index0_link_symbol: multiplex_index_context_link_symbol(axis, true),
            list_link_symbol: multiplex_list_context_link_symbol(axis, "Names"),
            value_type: UserContextValueType::Str,
            entries: Vec::new(),
        };
        if axis == "axis-a" {
            registry
                .upsert_multiplex_list_entry(owner, "Names", multiplex)
                .expect("first list should register");
        } else {
            registry
                .upsert_additional_multiplex_list_entry(owner, "Names", multiplex)
                .expect("same-named list should remain separately enumerable");
        }
    }

    let candidates = registry.collect_candidates(consumer, Some(UserContextValueType::Str), |node| {
        (node == consumer).then_some(owner)
    });
    assert_eq!(candidates.len(), 2);
    assert_ne!(
        candidates[0]
            .multiplex
            .as_ref()
            .expect("candidate should be multiplexed")
            .axis_id,
        candidates[1]
            .multiplex
            .as_ref()
            .expect("candidate should be multiplexed")
            .axis_id
    );
}
