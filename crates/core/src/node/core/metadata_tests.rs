use serde_json::json;

use super::PresentationHint;

#[test]
fn presentation_hint_defaults_to_nested_inspector_visibility() {
    assert!(PresentationHint::default().show_in_nested_inspector);
}

#[test]
fn presentation_hint_serializes_nested_inspector_flag_when_disabled() {
    let hint = PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "show_in_nested_inspector": false
        })
    );
}

#[test]
fn presentation_hint_serializes_inspector_content_flag_when_disabled() {
    let hint = PresentationHint {
        show_in_inspector_content: false,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "show_in_inspector_content": false
        })
    );
}

#[test]
fn presentation_hint_omits_default_nested_inspector_visibility() {
    let serialized = serde_json::to_value(PresentationHint::default()).expect("presentation hint should serialize");

    assert_eq!(serialized, json!({}));
}

#[test]
fn presentation_hint_serializes_collapsed_when_enabled() {
    let hint = PresentationHint {
        collapsed: true,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "collapsed": true
        })
    );
}
