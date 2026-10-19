use super::*;
use crate::{edit::Edit, parameter::ParamValue, process_ctx::ProcessCtx, script::ScriptPropertyWrite};

#[crate::node("script_property_probe")]
struct ScriptPropertyProbe {
    calls: usize,
    seen_pointer: usize,
}
#[crate::node("script_property_probe", from_struct)]
impl Node for ScriptPropertyProbe {
    fn engine_set_script_property(
        &mut self,
        _: &mut ProcessCtx,
        property: &str,
        value: &ParamValue,
    ) -> Result<bool, String> {
        let ParamValue::Str(text) = value else {
            return Err("expected string".into());
        };
        self.calls += 1;
        self.seen_pointer = text.as_ptr() as usize;
        if property == "reject" {
            return Err("injected rejection".into());
        }
        Ok(property == "accept")
    }
}

#[test]
fn property_dispatch_borrows_original_storage_for_acceptance_rejection_and_unsupported_keys() {
    for property in ["accept", "reject", "unsupported"] {
        let mut engine = Engine::new(ScriptPropertyProbe::new(0, 0));
        let text = "retained".repeat(4096);
        let pointer = text.as_ptr() as usize;
        let write = ScriptPropertyWrite::prepare(property, ParamValue::Str(text)).unwrap();
        let retained = write.clone();
        engine.edits.push(Edit::SetNodeScriptProperty {
            node: engine.root,
            write,
        });
        assert_eq!(engine.apply_edits().is_ok(), property == "accept");
        let node = engine.nodes.get(engine.root).unwrap();
        assert_eq!(node.calls, 1);
        assert_eq!(node.seen_pointer, pointer);
        let ParamValue::Str(text) = retained.value() else {
            panic!("original property")
        };
        assert_eq!(text.as_ptr() as usize, pointer);
    }
}

#[test]
fn missing_property_target_discards_unvisited_suffix_and_later_edits_still_progress() {
    let mut engine = Engine::new(ScriptPropertyProbe::new(0, 0));
    let write = ScriptPropertyWrite::prepare("accept", ParamValue::Str("source".repeat(4096))).unwrap();
    engine.edits.push(Edit::SetNodeScriptProperty {
        node: NodeId(u64::MAX),
        write: write.clone(),
    });
    engine.edits.push(Edit::SetNodeScriptProperty {
        node: engine.root,
        write: write.clone(),
    });
    assert!(engine.apply_edits().is_err());
    engine.apply_edits().unwrap();
    assert_eq!(engine.nodes.get(engine.root).unwrap().calls, 0);
    engine.edits.push(Edit::SetNodeScriptProperty {
        node: engine.root,
        write,
    });
    engine.apply_edits().unwrap();
    assert_eq!(engine.nodes.get(engine.root).unwrap().calls, 1);
}
