use super::*;
use device_query::{DeviceQuery, DeviceState};
const GLOBAL_MOUSE_VARIANT: &str = "system|System Mouse";
pub(super) struct Global {
    state: DeviceState,
    previous: Option<Observed>,
    initial: bool,
}
#[derive(Default)]
struct Observed {
    x: i32,
    y: i32,
    buttons: [bool; 3],
}
impl Global {
    pub(super) fn create(config: MouseInputConfig) -> Result<Self, String> {
        if config.capture_os_input {
            return Err("blocking OS mouse input is only supported on Windows".into());
        }
        let state = DeviceState::checked_new().ok_or("failed to access local mouse input")?;
        Ok(Self {
            state,
            previous: None,
            initial: true,
        })
    }
}
impl Provider for Global {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        if std::mem::take(&mut self.initial) {
            return Ok(vec![MouseRuntimeEvent::DevicesChanged(vec![DiscoveredMouseDevice {
                index: 0,
                variant_id: GLOBAL_MOUSE_VARIANT.into(),
                label: "System Mouse".into(),
                details: "Global system mouse input".into(),
            }])]);
        }
        let state = self.state.get_mouse();
        let next = Observed {
            x: state.coords.0,
            y: state.coords.1,
            buttons: [1, 2, 3].map(|index| state.button_pressed.get(index).copied().unwrap_or(false)),
        };
        let mut events = Vec::new();
        if self
            .previous
            .as_ref()
            .is_none_or(|previous| previous.x != next.x || previous.y != next.y)
        {
            events.push(MouseRuntimeEvent::Input {
                device: GLOBAL_MOUSE_VARIANT.into(),
                event: MouseInputEvent::Moved {
                    x: next.x,
                    y: next.y,
                    dx: self
                        .previous
                        .as_ref()
                        .map_or(0, |previous| next.x.saturating_sub(previous.x)),
                    dy: self
                        .previous
                        .as_ref()
                        .map_or(0, |previous| next.y.saturating_sub(previous.y)),
                },
            });
        }
        if let Some(previous) = &self.previous {
            for (index, button) in [MouseButtonKind::Left, MouseButtonKind::Middle, MouseButtonKind::Right]
                .into_iter()
                .enumerate()
            {
                if previous.buttons[index] != next.buttons[index] {
                    events.push(MouseRuntimeEvent::Input {
                        device: GLOBAL_MOUSE_VARIANT.into(),
                        event: MouseInputEvent::ButtonChanged {
                            button,
                            pressed: next.buttons[index],
                        },
                    });
                }
            }
        }
        self.previous = Some(next);
        Ok(events)
    }
}
