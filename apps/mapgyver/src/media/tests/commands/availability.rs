use super::*;
use crate::applications::protocol::*;

#[test]
fn command_and_hosted_quickjs_availability_use_the_same_committed_request_and_history() {
    let mut engine = fixture();
    let MediaEditResult::ApplicationCreated { application } = apply(
        &mut engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Create { label: None },
        },
    ) else {
        panic!()
    };
    script(&mut engine, "function init(){local.listen({level:1});} function onAppOperationResult(reply){gc.emit('media-method-reply',reply);}".into());
    let command = command(&mut engine, "Media Operation");
    let request = serde_json::to_string(&MediaEdit::Applications {
        edit: ApplicationEdit::SetEnabled {
            application,
            enabled: false,
        },
    })
    .unwrap();
    let parameter = engine
        .process_tree_snapshot()
        .find_child_by_decl_id(command, "request")
        .unwrap();
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Str(request),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    let before = engine.undo_len();
    call(&mut engine, command, "execute", vec![]);
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.undo_len(), before + 1);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Disabled
    );
    assert_eq!(replies(&engine).last().unwrap()["status"], "applied");
    let listener = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Media Caller")
        .unwrap()
        .0;
    let request = serde_json::to_string(&MediaEdit::Applications {
        edit: ApplicationEdit::SetEnabled {
            application,
            enabled: true,
        },
    })
    .unwrap();
    let source = format!(
        "function init(){{local.listen({{level:1}});local.editMedia('availability-enable',{request:?});}} function onAppOperationResult(reply){{gc.emit('media-method-reply',reply);}}"
    );
    engine.edits.push(Edit::RemoveNode { node: listener });
    engine.apply_edits().unwrap();
    script(&mut engine, source);
    let before = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.undo_len(), before + 1);
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Enabled
    );
    let result = replies(&engine);
    assert_eq!(result.last().unwrap()["request_id"], "availability-enable");
    assert_eq!(result.last().unwrap()["status"], "applied");
    assert!(engine.undo().unwrap());
    assert_eq!(
        crate::applications::inspect(&engine, application).unwrap().enablement,
        ApplicationEnablement::Disabled
    );
}
