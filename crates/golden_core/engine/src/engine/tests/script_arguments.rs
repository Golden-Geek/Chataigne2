use super::*;
use crate::{edit::Edit, parameter::ParamValue, process_ctx::ProcessCtx};
use std::sync::Arc;

#[crate::node("script_argument_probe")]
struct ScriptArgumentProbe {
    captured: Option<Arc<Vec<ParamValue>>>,
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
        Ok(true)
    }
}

#[test]
fn script_argument_capture_preserves_the_owned_vector_without_cloning_values() {
    for method in ["accept", "reject"] {
        let mut engine = Engine::new(ScriptArgumentProbe::new(None));
        let mut args = Vec::with_capacity(128);
        args.push(ParamValue::Str("shared".repeat(1024)));
        let pointer = args.as_ptr();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: engine.root,
            method: method.into(),
            args,
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
