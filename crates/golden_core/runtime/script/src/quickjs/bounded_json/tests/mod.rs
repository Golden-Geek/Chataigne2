use super::*;

#[test]
fn parser_bounds_structure_before_expanding_wide_nested_or_duplicate_key_inputs() {
    let at_limit = format!("[{}]", vec!["0"; 4096].join(","));
    assert_eq!(parse(&at_limit).unwrap().as_array().unwrap().len(), 4096);
    let too_wide = format!("[{}]", vec!["0"; 4097].join(","));
    assert!(parse(&too_wide).unwrap_err().to_string().contains("item/depth"));
    let nested = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    assert!(parse(&nested).unwrap_err().to_string().contains("item/depth"));
    let duplicate = format!("{{{}}}", vec![r#""same":0"#; 4097].join(","));
    assert!(parse(&duplicate).unwrap_err().to_string().contains("item/depth"));
}

#[test]
fn parser_preserves_json_numbers_strings_and_explicitly_rejects_malformed_arguments() {
    let source = r#"[null,true,-123,18446744073709551615,1.25,"héllo",{"key":[2,3]}]"#;
    assert_eq!(parse(source).unwrap(), serde_json::from_str::<Value>(source).unwrap());
    for source in ["[", "[] garbage", "[NaN]", "[1,]"] {
        assert!(parse(source).is_err(), "{source}");
    }
}
