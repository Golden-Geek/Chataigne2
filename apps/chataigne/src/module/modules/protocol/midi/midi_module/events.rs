use super::*;

impl MidiModule {
    pub(super) fn on_param_change_inner(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: ParamValue,
    ) {
        if matches!(ctx.current_event().map(|event| &event.kind), Some(golden_core::events::EventKind::ParamChanged {source: Some(source), ..}) if *source == self.id())
        {
            return;
        }

        if let Some(channel) = self.channel_14_bit_toggle_channel(param) {
            if let ParamValue::Bool(previous) = old_value {
                self.queue_cc_conversion(ctx, channel, previous);
            }
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
}
