use super::*;
impl MouseModule {
    pub(super) fn handle_input_events(&mut self, ctx: &mut ProcessCtx, events: Vec<MouseInputEvent>) {
        if events.is_empty() {
            if self.delta_reset_at.is_some_and(|at| Instant::now() >= at) {
                self.delta_reset_at = None;
                self.set_delta(ctx, 0, 0);
            }
            return;
        }

        let device_label = self
            .selected_input_device()
            .map(|device| device.label)
            .unwrap_or_else(|| "Selected Mouse".to_string());
        self.set_delta(ctx, 0, 0);
        for event in events {
            match event {
                MouseInputEvent::Moved { x, y, dx, dy } => {
                    self.delta_reset_at = Some(Instant::now() + Duration::from_millis(8));
                    self.set_position(ctx, x, y);
                    self.set_delta(ctx, dx, dy);
                    self.set_last_event(ctx, format!("{device_label}: moved mouse to ({x}, {y})"));
                    if self.base.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Observed {device_label} move to ({x}, {y}) with delta ({dx}, {dy})")
                        );
                    }
                    self.emit_mouse_callback(
                        ctx,
                        MOUSE_MOVED_CALLBACK,
                        vec![
                            serde_json::json!({ "x": x, "y": y }),
                            serde_json::json!({ "x": dx, "y": dy }),
                            self.mouse_state_arg(),
                        ],
                    );
                }
                MouseInputEvent::ButtonChanged { button, pressed } => {
                    self.set_input_button(ctx, button, pressed);
                    let action = if pressed { "pressed" } else { "released" };
                    self.set_last_event(ctx, format!("{device_label}: {} button {action}", button.label()));
                    if self.base.log_incoming_enabled() {
                        golden_core::log!(
                            origin = self.id();
                            format!("Observed {device_label} {} mouse button {action}", button.label().to_ascii_lowercase())
                        );
                    }
                    self.emit_mouse_callback(
                        ctx,
                        MOUSE_BUTTON_CHANGED_CALLBACK,
                        vec![
                            serde_json::json!(button.as_str()),
                            serde_json::json!(pressed),
                            self.mouse_state_arg(),
                        ],
                    );
                    self.emit_mouse_callback(
                        ctx,
                        if pressed {
                            MOUSE_BUTTON_PRESSED_CALLBACK
                        } else {
                            MOUSE_BUTTON_RELEASED_CALLBACK
                        },
                        vec![serde_json::json!(button.as_str()), self.mouse_state_arg()],
                    );
                }
            }
        }

        self.base.emit_incoming_traffic(ctx);
    }
}
