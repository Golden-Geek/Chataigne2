use super::*;

impl KeyboardModule {
    pub(super) fn clear_input_values(&mut self, ctx: &mut ProcessCtx) {
        self.pressed_keys.clear();
        self.sync_pressed_key_values(ctx);
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

    pub(super) fn sync_pressed_key_values(&mut self, ctx: &mut ProcessCtx) {
        let held_keys = self
            .pressed_keys
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.set_held_keys(ctx, held_keys.as_str());
        self.set_held_key_count(ctx, self.pressed_keys.len());

        let left_shift = self.pressed_keys.contains(&KeyboardKey::LeftShift);
        if self.left_shift.is_bound() && self.left_shift.get() != left_shift {
            self.left_shift.set(ctx, left_shift);
        }
        let right_shift = self.pressed_keys.contains(&KeyboardKey::RightShift);
        if self.right_shift.is_bound() && self.right_shift.get() != right_shift {
            self.right_shift.set(ctx, right_shift);
        }
        let left_control = self.pressed_keys.contains(&KeyboardKey::LeftControl);
        if self.left_control.is_bound() && self.left_control.get() != left_control {
            self.left_control.set(ctx, left_control);
        }
        let right_control = self.pressed_keys.contains(&KeyboardKey::RightControl);
        if self.right_control.is_bound() && self.right_control.get() != right_control {
            self.right_control.set(ctx, right_control);
        }
        let left_alt = self.pressed_keys.contains(&KeyboardKey::LeftAlt);
        if self.left_alt.is_bound() && self.left_alt.get() != left_alt {
            self.left_alt.set(ctx, left_alt);
        }
        let right_alt = self.pressed_keys.contains(&KeyboardKey::RightAlt);
        if self.right_alt.is_bound() && self.right_alt.get() != right_alt {
            self.right_alt.set(ctx, right_alt);
        }
        let left_meta = self.pressed_keys.contains(&KeyboardKey::LeftMeta);
        if self.left_meta.is_bound() && self.left_meta.get() != left_meta {
            self.left_meta.set(ctx, left_meta);
        }
        let right_meta = self.pressed_keys.contains(&KeyboardKey::RightMeta);
        if self.right_meta.is_bound() && self.right_meta.get() != right_meta {
            self.right_meta.set(ctx, right_meta);
        }
        let caps_lock = self.pressed_keys.contains(&KeyboardKey::CapsLock);
        if self.caps_lock.is_bound() && self.caps_lock.get() != caps_lock {
            self.caps_lock.set(ctx, caps_lock);
        }
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
