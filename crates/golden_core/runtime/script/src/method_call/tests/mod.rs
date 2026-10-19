use super::*;
use std::time::Duration;

#[test]
fn original_storage_and_spare_capacity_survive_clones_and_final_owner_handoff() {
    let mut args = Vec::with_capacity(128);
    let mut text = String::with_capacity(1024);
    text.push('x');
    args.push(ParamValue::Str(text));
    let pointer = args.as_ptr();
    let call = ScriptMethodCall::prepare("sendText", args).unwrap();
    assert_eq!(call.args().as_ptr(), pointer);
    assert_eq!(call.args().capacity(), 128);
    assert_eq!(
        call.args().retained_bytes(),
        size_of::<Vec<ParamValue>>() + 64 + 128 * size_of::<ParamValue>() + 1024
    );
    let clone = call.clone();
    drop(call);
    assert_eq!(clone.args().as_ptr(), pointer);
    assert_eq!(clone.method(), "sendText");
}

#[test]
fn rejected_count_and_spare_allocations_return_original_data_to_the_producer() {
    let reference = golden_parameters::NodeReference {
        relative_path_from_root: Vec::with_capacity(MAX_ARGUMENT_BYTES / size_of::<String>() + 1),
        ..Default::default()
    };
    for args in [
        vec![ParamValue::Int(0); MAX_ARGUMENTS + 1],
        Vec::with_capacity(MAX_ARGUMENT_BYTES / size_of::<ParamValue>() + 1),
        vec![ParamValue::Str(String::with_capacity(MAX_ARGUMENT_BYTES + 1))],
        vec![ParamValue::Reference(reference)],
    ] {
        let pointer = args.as_ptr();
        let capacity = args.capacity();
        let error = ScriptMethodCall::prepare("oversize", args).unwrap_err();
        let (method, returned) = error.into_parts();
        assert_eq!(method, "oversize");
        assert_eq!(returned.as_ptr(), pointer);
        assert_eq!(returned.capacity(), capacity);
    }
    let method = String::with_capacity(MAX_METHOD_BYTES + 1);
    let pointer = method.as_ptr();
    let error = ScriptMethodCall::prepare(method, Vec::new()).unwrap_err();
    assert_eq!(error.into_parts().0.as_ptr(), pointer);
}

#[test]
fn cancelled_and_sealed_conversions_preserve_admission_until_the_last_lease() {
    let pool = PayloadPool::new("script-permit-test", 1, MAX_ARGUMENT_BYTES).unwrap();
    let reserve = || ScriptCallReservation {
        method: "sendText".into(),
        reservation: pool.try_reserve(MAX_ARGUMENT_BYTES).unwrap(),
    };
    let reservation = reserve();
    assert!(pool.try_reserve(1).is_err());
    drop(reservation);
    assert!(pool.wait_for_idle(Duration::ZERO));
    let call = reserve()
        .finish(vec![ParamValue::Str("partial conversion".into())])
        .unwrap();
    let args = call.args().clone();
    drop(call);
    assert!(!pool.wait_for_idle(Duration::ZERO));
    drop(args);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_reserve(MAX_ARGUMENT_BYTES).is_ok());
}
