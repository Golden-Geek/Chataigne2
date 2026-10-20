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
    assert_eq!(validate("{\"a\":[1,true,null]}", 100, 5, 2).unwrap(), 5);
    assert_eq!(validate("{\"a\":1,\"a\":2}", 100, 3, 2).unwrap(), 3);
    assert!(
        parse("{\"a\":1,\"a\":2}", 100, 2, 2).is_err(),
        "duplicate keys consume budget too"
    );
}

#[test]
fn retained_json_admission_counts_sparse_map_nodes_and_spare_storage() {
    let value = serde_json::json!({"entry": "value"});
    assert!(retained_weight(&value, 512, 100, 64).is_err());
    assert!(retained_weight(&value, 8192, 100, 64).is_ok());
    let mut text = String::with_capacity(8192);
    text.push('x');
    assert!(retained_weight(&serde_json::Value::String(text), 1024, 100, 64).is_err());
    let value = serde_json::Value::Array(Vec::with_capacity(1024));
    assert!(retained_weight(&value, 4096, 100, 64).is_err());
}
