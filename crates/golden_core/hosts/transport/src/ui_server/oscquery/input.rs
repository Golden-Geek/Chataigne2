use super::{graph, preferences, values};
use crate::ui_server::{ServerState, ServerWorkers, save_preferences_after_change};
use golden_engine::{app::ProjectGeneration, parameter::ParamValue, ui_read_model::UiSnapshotCapture};
use golden_protocol::{UiEditIntent, UiSubscriptionScope};
use rosc::{OscPacket, OscTime};
use std::{
    io,
    sync::mpsc::{self, SyncSender},
};

pub(super) const MAX_DATAGRAM: usize = 4096;
const QUEUE_CAPACITY: usize = 32;
const MAX_INTENTS: usize = 32;
pub(super) const IMMEDIATE: OscTime = OscTime {
    seconds: 0,
    fractional: 1,
};

pub(super) struct Datagram {
    pub(super) port: u16,
    pub(super) project: ProjectGeneration,
    pub(super) bytes: Vec<u8>,
}

/// Network admission stays nonblocking while this worker resolves paths and awaits Golden receipts.
pub(super) fn spawn<T: golden_engine::app::ProjectLifecycle + 'static>(
    state: &ServerState<T>,
    workers: &mut ServerWorkers,
) -> io::Result<SyncSender<Datagram>> {
    let (sender, receiver) = mpsc::sync_channel::<Datagram>(QUEUE_CAPACITY);
    let runtime = state.runtime.clone();
    let model = state.read_model.clone();
    let settings = state.preferences.clone();
    let shutdown = workers.shutdown.clone();
    workers.spawn("golden-osc-input", move || {
        while let Ok(datagram) = receiver.recv() {
            if shutdown.is_requested() {
                break;
            }
            let Ok(([], packet)) = rosc::decoder::decode_udp(&datagram.bytes) else {
                continue;
            };
            let snapshot = model.capture_snapshot(UiSubscriptionScope::WholeGraph);
            if snapshot.project_generation() != datagram.project
                || preferences::enabled_port(&snapshot) != Some(datagram.port)
            {
                continue;
            }
            let mut intents = Vec::new();
            if collect_intents(&snapshot, &packet, &mut intents)
                && !intents.is_empty()
                && let Ok(result) = runtime.apply_ui_transaction_batch_for_project(datagram.project, intents, None)
            {
                save_preferences_after_change(&runtime, settings.as_ref(), result.preferences_changed);
            }
        }
    })?;
    Ok(sender)
}

fn collect_intents(snapshot: &UiSnapshotCapture, packet: &OscPacket, intents: &mut Vec<UiEditIntent>) -> bool {
    match packet {
        OscPacket::Message(message) => {
            if let Some(node) = graph::resolve(snapshot, &message.addr)
                && let Some(param) = graph::parameter(node).filter(|param| !param.read_only)
                && let Some(value) = values::incoming(&param.value, &message.args)
            {
                if intents.len() == MAX_INTENTS {
                    return false;
                }
                let behaviour = if matches!(value, ParamValue::Trigger()) {
                    golden_engine::parameter::ParameterEventBehaviour::Append
                } else {
                    param.event_behaviour
                };
                intents.push(UiEditIntent::SetParam {
                    node: node.node_id,
                    value,
                    behaviour,
                });
            }
            true
        }
        OscPacket::Bundle(bundle) => {
            bundle.timetag == IMMEDIATE
                && bundle
                    .content
                    .iter()
                    .all(|packet| collect_intents(snapshot, packet, intents))
        }
    }
}
