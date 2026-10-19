use super::super::input::{self, InputSnapshot, UdpInputConfig};
use super::*;
use crate::app::module::common::streaming::parser::StreamingParser;
use golden_core::{
    edit::Edit,
    events::EventKind,
    node::{Folder, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

#[test]
fn discarded_callback_publication_retries_only_the_unpublished_event() {
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let snapshot = InputSnapshot {
        generation: 0,
        config: UdpInputConfig {
            module: engine.root,
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    };
    let packet = input::prepare(&mut StreamingParser::default(), &snapshot, b"value 7\n").unwrap();
    let mut delivery = InputDelivery::new(packet);
    let mut stream = StreamingModuleBase::create();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut ctx, &mut stream));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    assert!(!delivery.service(&mut ctx, &mut stream));
    engine.edits.push(Edit::SetParam {
        node: NodeId(u64::MAX),
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    for _ in 0..8 {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        let done = delivery.service(&mut ctx, &mut stream);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
    }
    let callbacks: Vec<_> = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| {
            if let EventKind::Custom(custom) = &event.kind {
                custom
                    .payload
                    .get("callback")
                    .and_then(|name| name.as_str())
                    .map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(callbacks, ["dataReceived", "dataReceive", "textReceived"]);
    assert!(stream.has_pending_messages());
}
