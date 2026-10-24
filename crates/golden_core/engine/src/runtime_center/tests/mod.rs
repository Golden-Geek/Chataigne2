use super::*;

mod scheduling;

#[test]
fn binary_runtime_projection_retains_raw_bytes_in_both_directions() {
    let bytes = ParamValue::Bytes(vec![0, 127, 255]);
    let runtime = param_to_runtime_value(&bytes, 7, 99).unwrap();
    let RuntimeValue::Extension(ref extension) = runtime else {
        panic!("binary extension")
    };
    assert_eq!(&*extension.payload, &[0, 127, 255]);
    assert_eq!(runtime_to_param_value(&runtime, Some(&bytes)).unwrap(), bytes);
}
