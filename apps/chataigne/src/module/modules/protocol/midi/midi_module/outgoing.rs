use super::*;

impl MidiModule {
    pub(super) fn handle_feedback_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: ParamValue,
    ) {
        if !self.auto_feedback.get() {
            return;
        }
        let Some(values_id) = self.base.values_id() else {
            return;
        };
        if !is_descendant_of(snapshot, param, values_id) {
            return;
        }

        let Some(messages) = feedback_messages_for_param(snapshot, param, old_value) else {
            return;
        };
        if messages.is_empty() {
            return;
        }

        let result = ctx
            .node_waker()
            .map_err(|e| e.to_string())
            .and_then(|producer| {
                midi_output::MidiOutputRequest::feedback(messages, producer, self.base.log_outgoing_enabled())
            })
            .and_then(|request| self.enqueue_output(request));
        if let Err(error) = result {
            logerror!("MIDI auto feedback was not admitted: {}", error);
        }
    }

    pub(super) fn emit_midi_received_callbacks(&self, ctx: &mut ProcessCtx, message: &MidiMessage) {
        use crate::app::module::script_api;

        script_api::emit_script_callback(
            ctx,
            self.id(),
            MIDI_MESSAGE_RECEIVED_CALLBACK,
            vec![midi_message_script_payload(message)],
        );

        match message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if *velocity == 0 => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(0),
                ],
            ),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_ON_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            ),
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            ),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_CC_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(controller),
                    serde_json::json!(value),
                ],
            ),
            MidiMessage::System(MidiSystemMessage::Sysex { bytes }) => {
                script_api::emit_script_callback(
                    ctx,
                    self.id(),
                    MIDI_SYSEX_RECEIVED_CALLBACK,
                    vec![script_api::bytes_arg(bytes.as_slice())],
                );
            }
            _ => {}
        }
    }

    pub(super) fn handle_script_send_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        midi_output::MidiOutputRequest::script(ctx, method, args, self.base.log_outgoing_enabled())
            .map(|result| result.and_then(|request| self.enqueue_output(request)))
    }
    pub(super) fn enqueue_output(&self, mut request: midi_output::MidiOutputRequest) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("MIDI module is disabled; request was not admitted".into());
        }
        request.log = self.base.log_outgoing_enabled();
        self.output
            .as_ref()
            .ok_or("MIDI output is not connected; request was not admitted")?
            .enqueue(request)
    }
    pub(crate) fn dispatch_command(
        ctx: &mut ProcessCtx,
        invocation: crate::app::module_command::CapturedInvocation,
    ) -> Result<(), String> {
        let module = invocation.module;
        let request = midi_output::MidiOutputRequest::command(
            invocation,
            ctx.node_waker().map_err(|e| e.to_string())?,
            std::time::Instant::now(),
        );
        ctx.call_node_mutation_without_snapshot(module, move |node, _| {
            let Some(module) = node.as_any_mut().downcast_mut::<MidiModule>() else {
                return Err("MIDI command target is unavailable".into());
            };
            if let Err(error) = module.enqueue_output(request) {
                logerror!(format!("MIDI command was not admitted: {error}"));
            }
            Ok(())
        });
        Ok(())
    }
}
