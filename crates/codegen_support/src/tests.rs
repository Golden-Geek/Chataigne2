use super::{declares_node_type, strip_for_scanning};

#[test]
fn strip_for_scanning_preserves_node_attrs_after_lifetimes() {
    let source = r#"
struct Helper {
    value: &'static str,
}

#[node("midi_module")]
pub struct MidiModule {}
"#;

    let stripped = strip_for_scanning(source);

    assert!(declares_node_type(&stripped));
    assert!(stripped.contains("pub struct MidiModule"));
}

#[test]
fn strip_for_scanning_still_strips_char_literals() {
    let source = r#"
const NOTE: char = 'A';
#[node("midi_module")]
pub struct MidiModule {}
"#;

    let stripped = strip_for_scanning(source);

    assert!(declares_node_type(&stripped));
    assert!(!stripped.contains("'A'"));
}
