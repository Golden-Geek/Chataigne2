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

        for message in messages {
            let bytes = encode_midi_message(&message);
            if let Err(error) = self.send_bytes(ctx, bytes.as_slice(), message_description(&message).as_str()) {
                logerror!("Failed to send MIDI auto feedback: {}", error);
            }
        }
    }

    pub(super) fn queue_send_request(
        &mut self,
        ctx: &mut ProcessCtx,
        request: MidiSendRequest,
    ) -> Result<String, String> {
        let mut queued = 0usize;
        for packet in request.packets {
            if packet.delay_ms == 0 {
                self.send_bytes(ctx, packet.bytes.as_slice(), packet.description.as_str())?;
                queued = queued.saturating_add(1);
                continue;
            }

            self.pending_packets.push(PendingMidiPacket {
                due_at: ctx.runtime_elapsed + Duration::from_millis(packet.delay_ms),
                bytes: packet.bytes,
                description: packet.description,
            });
            queued = queued.saturating_add(1);
        }

        Ok(format!("Queued {} MIDI packet(s)", queued))
    }

    pub(super) fn flush_pending_packets(&mut self, ctx: &mut ProcessCtx) {
        if self.pending_packets.is_empty() {
            return;
        }

        let now = ctx.runtime_elapsed;
        let packets = std::mem::take(&mut self.pending_packets);
        for packet in packets {
            if packet.due_at > now {
                self.pending_packets.push(packet);
                continue;
            }

            if let Err(error) = self.send_bytes(ctx, packet.bytes.as_slice(), packet.description.as_str()) {
                logerror!("Failed to send delayed MIDI packet: {}", error);
            }
        }
    }

    pub(super) fn send_bytes(&mut self, _ctx: &mut ProcessCtx, bytes: &[u8], description: &str) -> Result<(), String> {
        let output = self
            .output
            .as_mut()
            .ok_or_else(|| "MIDI output is not connected".to_string())?;
        output.send(bytes, description, self.base.log_outgoing_enabled())
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
        let result = match method {
            "sendNoteOn" => {
                let message = MidiMessage::NoteOn {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    velocity: script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendNoteOff" => {
                let message = MidiMessage::NoteOff {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    velocity: script_u7_arg_or(args, 2, 0),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendFullNote" => (|| -> Result<(), String> {
                let channel = script_channel_arg(args, 0, "channel");
                let note = script_u7_arg(args, 1, "note");
                let velocity = script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY);
                let duration_ms = script_nonnegative_u64_arg_or(args, 3, 100);
                let off_velocity = script_u7_arg_or(args, 4, 0);
                let note_on = MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                };
                let note_off = MidiMessage::NoteOff {
                    channel,
                    note,
                    velocity: off_velocity,
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&note_on).as_slice(),
                    message_description(&note_on).as_str(),
                )?;
                self.pending_packets.push(PendingMidiPacket {
                    due_at: ctx.runtime_elapsed + Duration::from_millis(duration_ms),
                    bytes: encode_midi_message(&note_off),
                    description: message_description(&note_off),
                });
                Ok(())
            })(),
            "sendCC" | "sendControlChange" => {
                let message = MidiMessage::ControlChange {
                    channel: script_channel_arg(args, 0, "channel"),
                    controller: script_u7_arg(args, 1, "controller"),
                    value: script_u7_arg(args, 2, "value"),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendProgramChange" => {
                let message = MidiMessage::ProgramChange {
                    channel: script_channel_arg(args, 0, "channel"),
                    program: script_u7_arg(args, 1, "program"),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendPitchBend" => {
                let message = MidiMessage::PitchBend {
                    channel: script_channel_arg(args, 0, "channel"),
                    value: script_u14_arg(args, 1, "value"),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendChannelPressure" => {
                let message = MidiMessage::ChannelPressure {
                    channel: script_channel_arg(args, 0, "channel"),
                    pressure: script_u7_arg(args, 1, "pressure"),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendPolyPressure" => {
                let message = MidiMessage::PolyPressure {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    pressure: script_u7_arg(args, 2, "pressure"),
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&message).as_slice(),
                    message_description(&message).as_str(),
                )
            }
            "sendSysEx" | "sendSysex" => script_bytes_from_args(args)
                .map(|bytes| normalize_sysex_bytes(bytes.as_slice()))
                .and_then(|bytes| self.send_bytes(ctx, bytes.as_slice(), "sysex bytes")),
            "sendRawBytes" => {
                script_bytes_from_args(args).and_then(|bytes| self.send_bytes(ctx, bytes.as_slice(), "raw MIDI bytes"))
            }
            _ => return None,
        };

        Some(result)
    }

    pub(super) fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !MIDI_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
            return;
        }

        if let Err(error) = serde_json::from_value::<MidiSendRequest>(request.payload)
            .map_err(|error| format!("invalid MIDI command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, payload))
        {
            logerror!(format!(
                "Failed to handle MIDI command {:?}: {error}",
                request.command_id
            ));
        }
    }
}
