use crate::app::module::common::keyboard::{KeyboardKey, KeyboardKeyAction};
use rdev::{simulate, EventType, SimulateError};
pub(crate) struct KeyboardOutputController;

impl KeyboardOutputController {
    pub(crate) fn create() -> Result<Self, String> {
        if cfg!(test) {
            return Err("keyboard native output is disabled in test builds".into());
        }
        Ok(Self)
    }

    pub(crate) fn execute_key(&mut self, key: KeyboardKey, action: KeyboardKeyAction) -> Result<String, String> {
        let output_key = key.to_rdev();
        match action {
            KeyboardKeyAction::Tap => {
                simulate_event(&EventType::KeyPress(output_key))
                    .map_err(|error| format!("failed to press the keyboard key: {error}"))?;
                simulate_event(&EventType::KeyRelease(output_key))
                    .map_err(|error| format!("failed to release the keyboard key: {error}"))?;
            }
            KeyboardKeyAction::Press => simulate_event(&EventType::KeyPress(output_key))
                .map_err(|error| format!("failed to press the keyboard key: {error}"))?,
            KeyboardKeyAction::Release => simulate_event(&EventType::KeyRelease(output_key))
                .map_err(|error| format!("failed to release the keyboard key: {error}"))?,
        }

        Ok(format!(
            "{} {} key",
            action.summary_label(),
            key.label().to_ascii_lowercase()
        ))
    }
}

fn simulate_event(event: &EventType) -> Result<(), SimulateError> {
    if cfg!(test) {
        return Err(SimulateError);
    }
    simulate(event)
}

impl super::Provider for KeyboardOutputController {
    fn execute(&mut self, request: super::Request) -> Result<String, String> {
        self.execute_key(request.key, request.action)
    }
}
