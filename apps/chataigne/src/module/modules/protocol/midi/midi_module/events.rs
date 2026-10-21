use super::*;

impl MidiModule {
    pub(super) fn on_param_change_inner(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: ParamValue,
    ) {
        if self.ignored_param_changes.remove(&param) {
            return;
        }

        if let Some(channel) = self.channel_14_bit_toggle_channel(param) {
            ctx.call_node_mutation(self.id(), move |node, inner_ctx| {
                let Some(module) = node.as_any_mut().downcast_mut::<MidiModule>() else {
                    return Err("expected MidiModule during deferred 14-bit channel conversion".to_string());
                };
                let Some(snapshot_arc) = inner_ctx.tree_snapshot_arc() else {
                    return Ok(());
                };
                let enabled = module.cc_channel_is_14_bit(snapshot_arc.as_ref(), channel);
                module.set_channel_14_bit_mode(inner_ctx, snapshot_arc.as_ref(), channel, enabled);
                Ok(())
            });
            return;
        }

        if self.param_affects_transport(param) {
            if self.input_port.is_bound() && self.input_port.id() == param {
                self.input_dirty = true;
            }
            if self.output_port.is_bound() && self.output_port.id() == param {
                self.output_dirty = true;
            }
            self.refresh_data_capabilities(ctx);
            return;
        }

        if self.clock_bpm_precision.is_bound() && self.clock_bpm_precision.id() == param {
            let _ = self.apply_midi_clock_values(ctx, snapshot);
            return;
        }

        self.handle_feedback_param_change(ctx, snapshot, param, old_value);
    }

    pub(super) fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.input_port.is_bound() && self.input_port.id() == param)
            || (self.output_port.is_bound() && self.output_port.id() == param)
    }

    pub(super) fn set_input_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.input_port.is_bound() {
            self.input_port
                .set_warning_with(ctx, Some(MIDI_INPUT_WARNING_ID), message, None);
        }
    }

    pub(super) fn clear_input_warning(&self, ctx: &mut ProcessCtx) {
        if self.input_port.is_bound() {
            self.input_port.clear_warning(ctx, Some(MIDI_INPUT_WARNING_ID));
        }
    }

    pub(super) fn set_output_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.output_port.is_bound() {
            self.output_port
                .set_warning_with(ctx, Some(MIDI_OUTPUT_WARNING_ID), message, None);
        }
    }

    pub(super) fn clear_output_warning(&self, ctx: &mut ProcessCtx) {
        if self.output_port.is_bound() {
            self.output_port.clear_warning(ctx, Some(MIDI_OUTPUT_WARNING_ID));
        }
    }

    pub(super) fn log_incoming_message(&self, message: &MidiMessage, bytes: &[u8]) {
        if !self.base.log_incoming_enabled() {
            return;
        }
        golden_core::log!(
            origin = self.id();
            format!(
                "Received MIDI {} ({})",
                message_description(message),
                format_midi_bytes(bytes)
            )
        );
    }
}
