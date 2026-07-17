use golden_core::node::Node;

use crate::app::module_modules_protocol_dmx_commands::{
    DmxCommandRequest, DMX_BLACKOUT_COMMAND_NODE_TYPE, DMX_SEND_FRAME_COMMAND_NODE_TYPE,
    DMX_SET_CHANNEL_COMMAND_NODE_TYPE,
};
use crate::app::{DmxBlackoutCommand, DmxSendFrameCommand, DmxSetChannelCommand};

#[test]
fn dmx_commands_are_project_creatable() {
    assert!(DmxSetChannelCommand::project_create(DMX_SET_CHANNEL_COMMAND_NODE_TYPE).is_some());
    assert!(DmxSendFrameCommand::project_create(DMX_SEND_FRAME_COMMAND_NODE_TYPE).is_some());
    assert!(DmxBlackoutCommand::project_create(DMX_BLACKOUT_COMMAND_NODE_TYPE).is_some());
}

#[test]
fn dmx_command_request_round_trips_with_typed_semantics() {
    let request = DmxCommandRequest::SetChannel {
        channel: 512,
        value: 255,
    };
    let encoded = serde_json::to_value(&request).unwrap();
    assert_eq!(
        serde_json::from_value::<DmxCommandRequest>(encoded).unwrap(),
        request
    );
}
