use super::*;
use golden_core::script::ScriptMethodCall;
mod capture;
mod worker;

fn script(method: &'static str, args: Vec<ParamValue>) -> HttpInput {
    let call = ScriptMethodCall::prepare(method, args).unwrap();
    HttpInput::Script {
        method,
        args: call.args().clone(),
    }
}

#[test]
fn script_materialization_is_deferred_and_preserves_the_original_argument_storage() {
    let text = "body".repeat(4096);
    let pointer = text.as_ptr();
    let call = ScriptMethodCall::prepare("post", vec![ParamValue::Str("/path".into()), ParamValue::Str(text)]).unwrap();
    let input = HttpInput::Script {
        method: "post",
        args: call.args().clone(),
    };
    drop(call);
    let HttpInput::Script { args, .. } = &input else {
        unreachable!()
    };
    let ParamValue::Str(text) = &args[1] else {
        unreachable!()
    };
    assert_eq!(text.as_ptr(), pointer);
    let request = input.materialize().unwrap();
    assert_eq!(request.payload.path, "/path");
}
