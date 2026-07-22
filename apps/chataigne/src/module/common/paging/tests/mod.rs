use super::*;

#[test]
fn sanitize_page_id_is_address_safe_and_reserves_default() {
    assert_eq!(sanitize_page_id("Main Mix"), "main_mix");
    assert_eq!(sanitize_page_id("  A//B  "), "a_b");
    assert_eq!(sanitize_page_id("EQ-Focus!"), "eq_focus");
    assert_eq!(sanitize_page_id("default"), "page");
    assert_eq!(sanitize_page_id(""), "page");
}

#[test]
fn unique_id_appends_suffix_on_collision() {
    let claimed = vec!["default".to_string(), "page".to_string()];
    assert_eq!(unique_id("lighting", &claimed), "lighting");
    assert_eq!(unique_id("page", &claimed), "page_2");
}
