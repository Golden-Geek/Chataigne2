use super::super::{OutgoingBytes, SendInput};
use super::*;

#[test]
fn canonical_argument_storage_survives_original_owner_and_materializes_only_on_native_owner() {
    let args = Arc::new(vec![ParamValue::Str(r"hello\n".into())]);
    let pointer = args.as_ptr();
    let script = ScriptSend::retain(Method::Text, args.clone()).unwrap();
    assert_eq!(script.args.as_ptr(), pointer);
    assert!(matches!(script.frame_kind(), StreamingSendFrameKind::Text));
    let bytes = OutgoingBytes::new(SendInput::Script(&script), 1024).unwrap();
    assert!(bytes.is_empty(), "actor admission does not encode bytes");
    assert_eq!(bytes.encoded_len(), None, "unprepared output has no claimed byte count");
    assert_eq!(
        bytes.weight(),
        9,
        "source length plus line ending is reserved before conversion"
    );
    drop(script);
    drop(args);
    let result = std::thread::spawn(move || bytes.into_vec()).join().unwrap().unwrap();
    assert_eq!(result, b"hello\n");
}

#[test]
fn argument_admission_counts_spare_vector_capacity_and_large_sources() {
    let args = Arc::new(Vec::with_capacity(MAX_SOURCE_BYTES / size_of::<ParamValue>() + 1));
    assert!(ScriptSend::retain(Method::Bytes, args).is_err());
    let args = Arc::new(vec![ParamValue::Str("a".repeat(MAX_SOURCE_BYTES + 1))]);
    assert!(ScriptSend::retain(Method::Text, args).is_err());
    let args = Arc::new(vec![ParamValue::Int(0); 4097]);
    assert!(ScriptSend::retain(Method::Bytes, args).is_err());
    let mut reference = golden_core::node::NodeReference::default();
    let mut name = String::with_capacity(MAX_SOURCE_BYTES + 1);
    name.push('x');
    reference.set_cached_name(Some(name));
    assert!(ScriptSend::retain(Method::Text, Arc::new(vec![ParamValue::Reference(reference)])).is_err());
    let mut reference = golden_core::node::NodeReference::default();
    reference.set_relative_path_from_root(Vec::with_capacity(MAX_SOURCE_BYTES / size_of::<String>() + 1));
    assert!(ScriptSend::retain(Method::Text, Arc::new(vec![ParamValue::Reference(reference)])).is_err());
}

#[test]
fn encoding_failure_and_transport_output_limit_cannot_turn_into_an_empty_success() {
    for (method, text, limit) in [(Method::Hex, "zz", 100), (Method::Text, "too long", 3)] {
        let script = ScriptSend::retain(method, Arc::new(vec![ParamValue::Str(text.into())])).unwrap();
        let mut bytes = OutgoingBytes::new(SendInput::Script(&script), limit).unwrap();
        assert!(bytes.prepare().is_err());
        assert_eq!(bytes.encoded_len(), None);
        assert!(bytes.prepare().is_err(), "failure is terminal for this occurrence");
        assert!(bytes.into_vec().is_err());
    }
}

#[test]
fn native_byte_and_hex_methods_keep_their_frame_kinds_and_numeric_arguments() {
    let bytes = ScriptSend::retain(Method::Bytes, Arc::new(vec![ParamValue::Int(1), ParamValue::Int(255)])).unwrap();
    assert!(matches!(bytes.frame_kind(), StreamingSendFrameKind::Binary));
    assert_eq!(bytes.materialize().unwrap().bytes, [1, 255]);
    let hex = ScriptSend::retain(Method::Hex, Arc::new(vec![ParamValue::Str("48 69".into())])).unwrap();
    assert!(matches!(hex.frame_kind(), StreamingSendFrameKind::Text));
    assert_eq!(hex.materialize().unwrap().bytes, b"Hi");
}

#[test]
fn conservative_output_bounds_cover_scalar_extremes_without_encoding_at_admission() {
    for value in [
        ParamValue::Float(f64::MAX),
        ParamValue::Float(f64::MIN_POSITIVE),
        ParamValue::Float(f64::from_bits(1)),
        ParamValue::Vec3(f64::MAX, f64::MIN, f64::from_bits(1)),
        ParamValue::Color(f64::MAX, f64::MIN, f64::from_bits(1), 1.0),
    ] {
        let script = ScriptSend::retain(Method::Text, Arc::new(vec![value])).unwrap();
        let bound = script.output_bound();
        let bytes = OutgoingBytes::new(SendInput::Script(&script), bound)
            .unwrap()
            .into_vec()
            .unwrap();
        assert!(!bytes.is_empty() && bytes.capacity() <= bound);
    }
    let script = ScriptSend::retain(Method::Bytes, Arc::new(vec![ParamValue::Str("1".into())])).unwrap();
    let bytes = OutgoingBytes::new(SendInput::Script(&script), 1)
        .unwrap()
        .into_vec()
        .unwrap();
    assert_eq!(bytes, [1]);
    assert_eq!(
        bytes.capacity(),
        1,
        "parser scratch capacity is not retained by a send cursor"
    );
}
