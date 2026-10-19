use super::*;
use crate::script::ScriptArguments;
use crate::{edit::Edit, parameter::ParamValue, process_ctx::ProcessCtx};

#[crate::node("script_argument_probe")]
struct ScriptArgumentProbe {
    captured: Option<ScriptArguments>,
}
#[crate::node("script_argument_probe", from_struct)]
impl Node for ScriptArgumentProbe {
    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        assert!(ctx.capture_script_args(&[ParamValue::Int(99)]).is_err());
        let captured = ctx.capture_script_args(args).unwrap();
        assert_eq!(captured.as_ptr(), args.as_ptr());
        self.captured = Some(captured);
        if method == "reject" {
            return Err("injected script rejection".into());
        }
        Ok(method != "unsupported")
    }
}

#[test]
fn script_argument_capture_preserves_the_owned_vector_without_cloning_values() {
    for method in ["accept", "reject", "unsupported"] {
        let mut engine = Engine::new(ScriptArgumentProbe::new(None));
        let mut args = Vec::with_capacity(128);
        args.push(ParamValue::Str("shared".repeat(1024)));
        let pointer = args.as_ptr();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: engine.root,
            call: crate::script::ScriptMethodCall::prepare(method, args).unwrap(),
        });
        let result = engine.apply_edits();
        assert_eq!(result.is_ok(), method == "accept");
        let captured = engine.nodes.get(engine.root).unwrap().captured.as_ref().unwrap();
        assert_eq!(captured.as_ptr(), pointer);
        assert_eq!(captured.capacity(), 128);
        let mut ctx = ProcessCtx::new(crate::process_ctx::ExecutionPhase::EngineTick, engine.time);
        assert!(ctx.capture_script_args(captured).is_err());
        ctx.call_node_mutation_without_snapshot(engine.root, |_, ctx| {
            assert!(ctx.capture_script_args(&[]).is_err());
            Ok(())
        });
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
    }
}

#[test]
fn missing_target_rejects_before_dispatch_and_discarded_suffix_cannot_run_later() {
    let mut engine = Engine::new(ScriptArgumentProbe::new(None));
    let call =
        crate::script::ScriptMethodCall::prepare("accept", vec![ParamValue::Str("retained".repeat(1024))]).unwrap();
    let args = call.args().clone();
    let pointer = args.as_ptr();
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: NodeId(u64::MAX),
        call: call.clone(),
    });
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: engine.root,
        call,
    });
    assert!(engine.apply_edits().is_err());
    assert!(engine.nodes.get(engine.root).unwrap().captured.is_none());
    engine.apply_edits().unwrap();
    assert!(engine.nodes.get(engine.root).unwrap().captured.is_none());
    assert_eq!(args.as_ptr(), pointer);
    assert_eq!(args[0], ParamValue::Str("retained".repeat(1024)));
}
