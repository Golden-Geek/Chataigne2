use super::*;

impl MidiModule {
    pub(super) fn ensure_cc_rotary_mode_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        cc_id: NodeId,
    ) {
        match snapshot.find_child_by_decl_id(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "enum") => {
                self.clear_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID);
            }
            Some(existing_id) => {
                if self.mark_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                    ctx.replace_node_boxed(existing_id, Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)));
                }
            }
            None => {
                if self.mark_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                    ctx.add_child_boxed(cc_id, Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)), None);
                }
            }
        }
    }

    pub(super) fn find_cc_folder(&self, snapshot: &ProcessTreeSnapshot, channel: u8) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        let channel_id = snapshot.find_child_by_decl_id(values_id, channel_decl_id(channel).as_str())?;
        snapshot.find_child_by_decl_id(channel_id, CONTROL_CHANGE_FOLDER_DECL_ID)
    }

    pub(super) fn cc_channel_is_14_bit(&self, _snapshot: &ProcessTreeSnapshot, channel: u8) -> bool {
        self.cc_channel_is_14_bit_handle(channel)
    }
    pub(super) fn cc_channel_is_14_bit_handle(&self, channel: u8) -> bool {
        match channel {
            1 => self.channel_1.get(),
            2 => self.channel_2.get(),
            3 => self.channel_3.get(),
            4 => self.channel_4.get(),
            5 => self.channel_5.get(),
            6 => self.channel_6.get(),
            7 => self.channel_7.get(),
            8 => self.channel_8.get(),
            9 => self.channel_9.get(),
            10 => self.channel_10.get(),
            11 => self.channel_11.get(),
            12 => self.channel_12.get(),
            13 => self.channel_13.get(),
            14 => self.channel_14.get(),
            15 => self.channel_15.get(),
            16 => self.channel_16.get(),
            _ => false,
        }
    }

    pub(super) fn cc_parameter_is_14_bit(&self, snapshot: &ProcessTreeSnapshot, channel: u8, controller: u8) -> bool {
        cc_supports_14_bit(controller) && self.cc_channel_is_14_bit(snapshot, channel)
    }

    pub(super) fn channel_14_bit_toggle_channel(&self, param: NodeId) -> Option<u8> {
        if self.channel_1.is_bound() && self.channel_1.id() == param {
            return Some(1);
        }
        if self.channel_2.is_bound() && self.channel_2.id() == param {
            return Some(2);
        }
        if self.channel_3.is_bound() && self.channel_3.id() == param {
            return Some(3);
        }
        if self.channel_4.is_bound() && self.channel_4.id() == param {
            return Some(4);
        }
        if self.channel_5.is_bound() && self.channel_5.id() == param {
            return Some(5);
        }
        if self.channel_6.is_bound() && self.channel_6.id() == param {
            return Some(6);
        }
        if self.channel_7.is_bound() && self.channel_7.id() == param {
            return Some(7);
        }
        if self.channel_8.is_bound() && self.channel_8.id() == param {
            return Some(8);
        }
        if self.channel_9.is_bound() && self.channel_9.id() == param {
            return Some(9);
        }
        if self.channel_10.is_bound() && self.channel_10.id() == param {
            return Some(10);
        }
        if self.channel_11.is_bound() && self.channel_11.id() == param {
            return Some(11);
        }
        if self.channel_12.is_bound() && self.channel_12.id() == param {
            return Some(12);
        }
        if self.channel_13.is_bound() && self.channel_13.id() == param {
            return Some(13);
        }
        if self.channel_14.is_bound() && self.channel_14.id() == param {
            return Some(14);
        }
        if self.channel_15.is_bound() && self.channel_15.id() == param {
            return Some(15);
        }
        if self.channel_16.is_bound() && self.channel_16.id() == param {
            return Some(16);
        }
        None
    }

    pub(super) fn set_or_create_cc_param(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        cc_folder_id: NodeId,
        update: MidiCcUpdate,
    ) {
        let decl_id = cc_decl_id(update.controller);
        let label = cc_label(update.controller);
        let range = self.cc_parameter_range(snapshot, update.channel, update.controller);
        match update.existing_id {
            Some(node_id) if snapshot.node(node_id).is_some_and(|node| node.node_type == "int") => {
                sync_int_parameter_constraints(ctx, snapshot, node_id, range, self.id());
                self.ensure_cc_rotary_mode_parameter(ctx, snapshot, node_id);
                self.set_internal_param(ctx, node_id, ParamValue::Int(update.value));
            }
            Some(node_id) => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                ctx.replace_node_boxed(
                    node_id,
                    Box::new(create_int_parameter(
                        label.as_str(),
                        decl_id.as_str(),
                        update.value,
                        range,
                        false,
                    )),
                );
            }
            None => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                ctx.add_child_boxed(
                    cc_folder_id,
                    Box::new(create_int_parameter(
                        label.as_str(),
                        decl_id.as_str(),
                        update.value,
                        range,
                        false,
                    )),
                    None,
                );
                self.schedule_ordered_value_rebuild(ctx, cc_folder_id);
            }
        }
    }

    pub(super) fn schedule_ordered_value_rebuild(&self, ctx: &mut ProcessCtx, parent_id: NodeId) {
        ctx.call_node_mutation(self.id(), move |_node, inner_ctx| {
            let Some(snapshot_arc) = inner_ctx.tree_snapshot_arc() else {
                return Ok(());
            };
            reorder_ordered_midi_value_children(inner_ctx, snapshot_arc.as_ref(), parent_id);
            Ok(())
        });
    }

    pub(super) fn set_channel_14_bit_mode(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        enable_14_bit: bool,
    ) {
        let channel_index = usize::from(channel.saturating_sub(1).min(15));
        self.cc_14_bit_state[channel_index] = [Cc14BitState::default(); 32];

        let Some(cc_folder_id) = self.find_cc_folder(snapshot, channel) else {
            return;
        };

        for base_controller in 0..32u8 {
            let lsb_controller = base_controller + 32;
            let base_decl_id = cc_decl_id(base_controller);
            let lsb_decl_id = cc_decl_id(lsb_controller);
            let base_id = snapshot.find_child_by_decl_id(cc_folder_id, base_decl_id.as_str());
            let lsb_id = snapshot.find_child_by_decl_id(cc_folder_id, lsb_decl_id.as_str());

            if enable_14_bit {
                if base_id.is_none() && lsb_id.is_none() {
                    continue;
                }

                let msb = base_id
                    .and_then(|node_id| snapshot.node(node_id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_int)
                    .map(clamp_i32_to_u7)
                    .unwrap_or(0);
                let lsb = lsb_id
                    .and_then(|node_id| snapshot.node(node_id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_int)
                    .map(clamp_i32_to_u7)
                    .unwrap_or(0);
                let combined = i32::from((u16::from(msb) << 7) | u16::from(lsb));

                self.set_or_create_cc_param(
                    ctx,
                    snapshot,
                    cc_folder_id,
                    MidiCcUpdate {
                        channel,
                        controller: base_controller,
                        existing_id: base_id,
                        value: combined,
                    },
                );
                if let Some(node_id) = lsb_id {
                    self.clear_pending_auto_child(cc_folder_id, lsb_decl_id.as_str());
                    ctx.edits.push(Edit::RemoveNode { node: node_id });
                }

                self.cc_14_bit_state[channel_index][usize::from(base_controller)] = Cc14BitState {
                    msb: Some(msb),
                    lsb: Some(lsb),
                };
                continue;
            }

            if base_id.is_none() && lsb_id.is_none() {
                continue;
            }

            let combined = base_id
                .and_then(|node_id| snapshot.node(node_id))
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .map(clamp_i32_to_u14)
                .unwrap_or_else(|| {
                    lsb_id
                        .and_then(|node_id| snapshot.node(node_id))
                        .and_then(|node| node.param_value.as_ref())
                        .and_then(ParamValue::as_int)
                        .map(clamp_i32_to_u7)
                        .map(u16::from)
                        .unwrap_or(0)
                });
            let msb = i32::from((combined >> 7) & 0x7F);
            let lsb = i32::from(combined & 0x7F);

            self.set_or_create_cc_param(
                ctx,
                snapshot,
                cc_folder_id,
                MidiCcUpdate {
                    channel,
                    controller: base_controller,
                    existing_id: base_id,
                    value: msb,
                },
            );
            self.set_or_create_cc_param(
                ctx,
                snapshot,
                cc_folder_id,
                MidiCcUpdate {
                    channel,
                    controller: lsb_controller,
                    existing_id: lsb_id,
                    value: lsb,
                },
            );
        }
    }

    pub(super) fn lock_existing_value_tree(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(values_id) = self.base.values_id() else {
            return;
        };

        let mut pending = vec![values_id];
        while let Some(node_id) = pending.pop() {
            lock_midi_value_node(ctx, node_id);
            pending.extend(snapshot.child_ids(node_id));
        }
    }

    pub(super) fn set_internal_param(&mut self, ctx: &mut ProcessCtx, param_id: NodeId, value: ParamValue) {
        ctx.set_param_from(self.id(), param_id, value, ParameterEventBehaviour::Append);
    }
}
