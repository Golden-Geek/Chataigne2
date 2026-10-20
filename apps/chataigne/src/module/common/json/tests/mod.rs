use super::*;

#[test]
fn parsing_rejects_wide_deep_malformed_and_oversized_sources_before_materialization() {
    for source in ["[1,2,3]", "[[[0]]]", "[", "true false"] {
        assert!(parse(source, 100, 3, 2).is_err(), "{source}");
    }
    assert!(parse("\"large\"", 4, 10, 2).is_err());
    assert_eq!(
        parse("{\"a\":[1,true,null]}", 100, 5, 2).unwrap(),
        serde_json::json!({"a":[1,true,null]})
    );
    assert!(
        parse("{\"a\":1,\"a\":2}", 100, 2, 2).is_err(),
        "duplicate keys consume budget too"
    );
}
