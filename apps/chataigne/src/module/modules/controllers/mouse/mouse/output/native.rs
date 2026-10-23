use super::{Provider, Request};
use crate::app::module::common::mouse::{MouseButtonAction, MouseButtonKind, MouseMoveCoordinate, MouseMoveUnits};
use device_query::{DeviceQuery, DeviceState};
use rdev::{display_size, simulate, Button as RdevButton, EventType, SimulateError};

pub(super) struct MouseOutputController;

impl MouseOutputController {
    pub(super) fn create() -> Result<Self, String> {
        if cfg!(test) {
            return Err("mouse native output is disabled in test builds".into());
        }
        Ok(Self)
    }

    fn execute_move(
        &mut self,
        x: f64,
        y: f64,
        coordinate: MouseMoveCoordinate,
        units: MouseMoveUnits,
    ) -> Result<String, String> {
        let requested = (x, y);
        let (x, y) = self.resolve_move(x, y, coordinate, units)?;

        simulate_event(&EventType::MouseMove {
            x: f64::from(x),
            y: f64::from(y),
        })
        .map_err(|error| format!("failed to move the mouse: {error}"))?;

        let summary = match (coordinate, units) {
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Pixels) => {
                format!("Moved mouse to ({x}, {y}) pixels")
            }
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Normalized) => {
                format!("Moved mouse to normalized ({:.3}, {:.3})", requested.0, requested.1)
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Pixels) => {
                format!("Moved mouse by ({x}, {y}) pixels")
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Normalized) => {
                return Err("normalized mouse movement only supports absolute coordinates".to_string())
            }
        };

        Ok(summary)
    }

    fn execute_button(&mut self, button: MouseButtonKind, action: MouseButtonAction) -> Result<String, String> {
        let button_name = button.label().to_ascii_lowercase();
        let button = button_to_rdev(button)?;
        match action {
            MouseButtonAction::Click => {
                simulate_event(&EventType::ButtonPress(button))
                    .map_err(|error| format!("failed to press the mouse button: {error}"))?;
                simulate_event(&EventType::ButtonRelease(button))
                    .map_err(|error| format!("failed to release the mouse button: {error}"))?;
            }
            MouseButtonAction::Press => simulate_event(&EventType::ButtonPress(button))
                .map_err(|error| format!("failed to press the mouse button: {error}"))?,
            MouseButtonAction::Release => simulate_event(&EventType::ButtonRelease(button))
                .map_err(|error| format!("failed to release the mouse button: {error}"))?,
        }

        Ok(format!("{} {} mouse button", action.label(), button_name))
    }

    fn execute_scroll(&mut self, vertical: i32, horizontal: i32) -> Result<String, String> {
        simulate_event(&EventType::Wheel {
            delta_x: i64::from(horizontal),
            delta_y: -i64::from(vertical),
        })
        .map_err(|error| format!("failed to scroll the mouse: {error}"))?;

        Ok(format!(
            "Scrolled mouse vertical={} horizontal={}",
            vertical, horizontal
        ))
    }

    fn resolve_move(
        &self,
        x: f64,
        y: f64,
        coordinate: MouseMoveCoordinate,
        units: MouseMoveUnits,
    ) -> Result<(i32, i32), String> {
        match (coordinate, units) {
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Pixels) => {
                Ok((round_f64_to_i32(x, "mouse x")?, round_f64_to_i32(y, "mouse y")?))
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Pixels) => {
                let Some(device_state) = DeviceState::checked_new() else {
                    return Err("relative mouse movement requires a readable current mouse position".to_string());
                };
                let current = device_state.get_mouse();
                Ok((
                    current
                        .coords
                        .0
                        .checked_add(round_f64_to_i32(x, "mouse dx")?)
                        .ok_or("relative mouse x overflow")?,
                    current
                        .coords
                        .1
                        .checked_add(round_f64_to_i32(y, "mouse dy")?)
                        .ok_or("relative mouse y overflow")?,
                ))
            }
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Normalized) => {
                let (width, height) =
                    display_size().map_err(|error| format!("failed to query the main display size: {error:?}"))?;
                if width == 0 || height == 0 {
                    return Err("the main display reported a non-positive size".to_string());
                }

                let x = clamp_unit(x) * (width.saturating_sub(1) as f64);
                let y = clamp_unit(y) * (height.saturating_sub(1) as f64);
                Ok((x.round() as i32, y.round() as i32))
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Normalized) => {
                Err("normalized mouse movement only supports absolute coordinates on the main display".to_string())
            }
        }
    }
}

fn clamp_unit(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

fn round_f64_to_i32(value: f64, name: &str) -> Result<i32, String> {
    if !value.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }

    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(format!("{name} is out of range for a 32-bit mouse coordinate"));
    }

    Ok(rounded as i32)
}

fn button_to_rdev(button: MouseButtonKind) -> Result<RdevButton, String> {
    match button {
        MouseButtonKind::Left => Ok(RdevButton::Left),
        MouseButtonKind::Middle => Ok(RdevButton::Middle),
        MouseButtonKind::Right => Ok(RdevButton::Right),
    }
}

fn simulate_event(event: &EventType) -> Result<(), SimulateError> {
    if cfg!(test) {
        return Err(SimulateError);
    }
    simulate(event)
}

trait MouseButtonActionLabel {
    fn label(self) -> &'static str;
}

impl MouseButtonActionLabel for MouseButtonAction {
    fn label(self) -> &'static str {
        match self {
            MouseButtonAction::Click => "Clicked",
            MouseButtonAction::Press => "Pressed",
            MouseButtonAction::Release => "Released",
        }
    }
}

impl Provider for MouseOutputController {
    fn execute(&mut self, request: Request) -> Result<String, String> {
        if cfg!(test) {
            return Err("mouse native output is disabled in test builds".into());
        }
        match request {
            Request::Move {
                x,
                y,
                coordinate,
                units,
            } => self.execute_move(x, y, coordinate, units),
            Request::Button { button, action } => self.execute_button(button, action),
            Request::Scroll { vertical, horizontal } => self.execute_scroll(vertical, horizontal),
        }
    }
}
