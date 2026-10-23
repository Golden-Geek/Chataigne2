use super::*;

impl KeyboardModule {
    pub(super) fn clear_input_values(&mut self, ctx: &mut ProcessCtx) {
        self.project_modifiers(ctx, [false; 9]);
        self.set_held_keys(ctx, "");
        self.set_held_key_count(ctx, 0);
        self.set_last_key(ctx, "");
        self.set_last_pressed(ctx, false);
    }

    pub(super) fn reset_values(&mut self, ctx: &mut ProcessCtx) {
        self.clear_input_values(ctx);
        self.set_input_active(ctx, false);
        self.set_output_active(ctx, false);
        self.set_connected_devices(ctx, 0);
        self.set_selected_device_id(ctx, "");
        self.set_selected_device_name(ctx, "");
        self.set_last_event(ctx, String::new());
    }
    pub(super) fn project_key_frame(&mut self, ctx: &mut ProcessCtx, frame: &keyboard_runtime::preparation::Frame) {
        self.project_modifiers(ctx, frame.modifiers);
        self.set_held_keys(ctx, &frame.held);
        self.set_held_key_count(ctx, frame.count);
        self.set_last_key(ctx, frame.key.as_str());
        self.set_last_pressed(ctx, frame.pressed);
        self.set_last_event(ctx, frame.message.clone());
    }
    fn project_modifiers(&mut self, ctx: &mut ProcessCtx, modifiers: [bool; 9]) {
        macro_rules! field {
            ($index:literal, $field:ident) => {
                if self.$field.is_bound() && self.$field.get() != modifiers[$index] {
                    self.$field.set(ctx, modifiers[$index]);
                }
            };
        }
        field!(0, left_shift);
        field!(1, right_shift);
        field!(2, left_control);
        field!(3, right_control);
        field!(4, left_alt);
        field!(5, right_alt);
        field!(6, left_meta);
        field!(7, right_meta);
        field!(8, caps_lock);
    }

    pub(super) fn set_input_active(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.input_active.is_bound() && self.input_active.get() != value {
            self.input_active.set(ctx, value);
        }
    }

    pub(super) fn set_output_active(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.output_active.is_bound() && self.output_active.get() != value {
            self.output_active.set(ctx, value);
        }
    }

    pub(super) fn set_last_event(&mut self, ctx: &mut ProcessCtx, value: String) {
        if self.last_event.is_bound() && self.last_event.get_ref().as_str() != value.as_str() {
            self.last_event.set(ctx, value);
        }
    }

    pub(super) fn set_last_key(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.last_key.is_bound() && self.last_key.get_ref().as_str() != value {
            self.last_key.set(ctx, value.to_string());
        }
    }

    pub(super) fn set_last_pressed(&mut self, ctx: &mut ProcessCtx, value: bool) {
        if self.last_pressed.is_bound() && self.last_pressed.get() != value {
            self.last_pressed.set(ctx, value);
        }
    }

    pub(super) fn set_held_keys(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.held_keys.is_bound() && self.held_keys.get_ref().as_str() != value {
            self.held_keys.set(ctx, value.to_string());
        }
    }

    pub(super) fn set_held_key_count(&mut self, ctx: &mut ProcessCtx, value: usize) {
        let value = clamp_usize_to_i32(value);
        if self.held_key_count.is_bound() && self.held_key_count.get() != value {
            self.held_key_count.set(ctx, value);
        }
    }

    pub(super) fn set_connected_devices(&mut self, ctx: &mut ProcessCtx, value: usize) {
        let value = clamp_usize_to_i32(value);
        if self.connected_devices.is_bound() && self.connected_devices.get() != value {
            self.connected_devices.set(ctx, value);
        }
    }

    pub(super) fn set_selected_device_id(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.device_id.is_bound() && self.device_id.get_ref().as_str() != value {
            self.device_id.set(ctx, value.to_string());
        }
    }

    pub(super) fn set_selected_device_name(&mut self, ctx: &mut ProcessCtx, value: &str) {
        if self.device_name.is_bound() && self.device_name.get_ref().as_str() != value {
            self.device_name.set(ctx, value.to_string());
        }
    }
}
