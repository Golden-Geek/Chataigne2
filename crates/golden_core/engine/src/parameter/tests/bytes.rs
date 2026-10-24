use super::*;

#[test]
fn binary_parameter_factory_and_authored_restore_preserve_values_and_defaults() {
    let mut parameter = Parameter::project_create("bytes").unwrap();
    assert_eq!(parameter.get_type(), "bytes");
    parameter.value = ParamValue::Bytes(vec![0, 127, 255]);
    parameter.default_value = ParamValue::Bytes(vec![42]);
    let state = parameter.engine_capture_authored_data().unwrap();
    let encoded = parameter.project_encode_data().unwrap();
    parameter.value = ParamValue::Bytes(vec![1, 2]);
    parameter.engine_restore_authored_data(state).unwrap();
    assert_eq!(parameter.value, ParamValue::Bytes(vec![0, 127, 255]));
    let mut reopened = Parameter::project_create("bytes").unwrap();
    reopened.project_decode_data(&encoded).unwrap();
    assert_eq!(reopened.value, parameter.value);
    assert_eq!(reopened.default_value, parameter.default_value);
    assert!(
        reopened
            .engine_prepare_param_value(ParamValue::Str("[1,2]".into()))
            .is_err()
    );
}
