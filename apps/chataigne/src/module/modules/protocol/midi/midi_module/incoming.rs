use super::*;

impl MidiModule {
    pub(super) fn process_pending_incoming(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
    ) -> bool {
        let mode = self.input_mode();
        let root = self.base.values_id();
        // Callback receipts keep their own canonical leases, but do not stall
        // subsequent clock pulses after this message's graph writes settle.
        self.incoming_callbacks
            .retain_mut(|delivery| !delivery.service(ctx, None, root, mode));
        for _ in 0..2 {
            if self.incoming.is_none() {
                if self.incoming_callbacks.len() >= 16 {
                    break;
                }
                let Some(snapshot) = snapshot else { break };
                let Some(input) = self.pending_incoming_messages.pop_front() else {
                    break;
                };
                if let Some(log) = &input.log {
                    golden_core::log!(origin = self.id(); log.clone());
                }
                self.incoming = Some(self.prepare_delivery(ctx, snapshot, input));
            }
            let delivery = self.incoming.as_mut().expect("current delivery");
            let complete = delivery.service(ctx, snapshot, root, mode);
            if complete {
                self.incoming = None;
            } else if delivery.graph_done() {
                self.incoming_callbacks
                    .push_back(self.incoming.take().expect("settled graph"));
            } else {
                break;
            }
        }
        self.incoming.is_some() || !self.pending_incoming_messages.is_empty() || !self.incoming_callbacks.is_empty()
    }
    pub(super) fn cc_parameter_range(
        &self,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
    ) -> Option<(i32, i32)> {
        Some(if self.cc_parameter_is_14_bit(snapshot, channel, controller) {
            MIDI_14_BIT_VALUE_RANGE
        } else {
            MIDI_7_BIT_VALUE_RANGE
        })
    }
}
