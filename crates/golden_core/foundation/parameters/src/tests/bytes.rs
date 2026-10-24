use super::*;
use golden_values::Value;

#[test]
fn bytes_are_exact_in_scripts_persistence_and_runtime_without_text_coercion() {
    for bytes in [vec![], vec![0, 255], vec![1, 2, 3], vec![0; 512]] {
        let parameter = ByteArray::to_param_value(ByteArray(bytes.clone()));
        assert_eq!(ByteArray::from_param_value(&parameter), Some(ByteArray(bytes.clone())));
        let json = serde_json::to_value(&parameter).unwrap();
        assert_eq!(json, serde_json::json!({ "Bytes": bytes }));
        assert_eq!(
            ParamValue::from_script_json(&parameter.to_script_json()).unwrap(),
            parameter
        );
        assert_eq!(serde_json::from_value::<ParamValue>(json).unwrap(), parameter);
        let Value::Extension(runtime) = Value::try_from(&parameter).unwrap() else {
            panic!("binary extension")
        };
        assert_eq!(&*runtime.payload, bytes.as_slice());
        assert!(coerce_param_value_for_target(&ParamValue::Str("[0,255]".into()), &parameter, None).is_none());
    }
    assert!(matches!(
        ParamValue::from_script_json(&serde_json::json!([1, 2, 3])),
        Ok(ParamValue::Vec3(..))
    ));
    assert!(ParamValue::from_script_json(&serde_json::json!({"Bytes": [256]})).is_err());
}

#[test]
fn retained_bytes_charge_allocated_capacity() {
    let mut bytes = Vec::with_capacity(4096);
    bytes.push(1);
    let parameter = ParamValue::Bytes(bytes);
    assert!(parameter.retained_bytes() >= 4096);
    assert!(parameter.retained_heap_bytes() >= 4096);
}
