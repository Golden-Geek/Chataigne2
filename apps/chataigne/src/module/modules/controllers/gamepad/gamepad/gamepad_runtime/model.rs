use gilrs::{Axis, Button};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DiscoveredGamepad {
    pub index: usize,
    pub variant_id: String,
    pub label: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct GamepadStateSnapshot {
    pub device: DiscoveredGamepad,
    pub axes: [f64; 8],
    pub buttons: [(bool, f64); 19],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GamepadRuntimeEvent {
    Connected {
        device: DiscoveredGamepad,
    },
    Disconnected {
        device: DiscoveredGamepad,
    },
    AxisChanged {
        device: DiscoveredGamepad,
        axis: GamepadAxis,
        value: f64,
    },
    ButtonPressed {
        device: DiscoveredGamepad,
        button: GamepadButton,
    },
    ButtonReleased {
        device: DiscoveredGamepad,
        button: GamepadButton,
    },
    ButtonChanged {
        device: DiscoveredGamepad,
        button: GamepadButton,
        value: f64,
    },
}

pub(crate) fn selected_gamepad(selection: &str, gamepads: &[DiscoveredGamepad]) -> Option<DiscoveredGamepad> {
    gamepads
        .iter()
        .find(|gamepad| selection_matches_gamepad(selection, gamepad))
        .cloned()
}

pub(crate) fn selection_matches_gamepad(selection: &str, gamepad: &DiscoveredGamepad) -> bool {
    match selection.trim() {
        "auto" => true,
        "none" | "" => false,
        selected if selected == gamepad.variant_id => true,
        selected => selected
            .split_once('|')
            .is_some_and(|(_, label)| label == gamepad.label),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum GamepadAxis {
    LeftStickX,
    LeftStickY,
    LeftZ,
    RightStickX,
    RightStickY,
    RightZ,
    DPadX,
    DPadY,
}

impl GamepadAxis {
    pub(crate) const ALL: [Self; 8] = [
        Self::LeftStickX,
        Self::LeftStickY,
        Self::LeftZ,
        Self::RightStickX,
        Self::RightStickY,
        Self::RightZ,
        Self::DPadX,
        Self::DPadY,
    ];

    pub(crate) fn decl_id(self) -> &'static str {
        match self {
            Self::LeftStickX => "left_stick_x",
            Self::LeftStickY => "left_stick_y",
            Self::LeftZ => "left_z",
            Self::RightStickX => "right_stick_x",
            Self::RightStickY => "right_stick_y",
            Self::RightZ => "right_z",
            Self::DPadX => "dpad_x",
            Self::DPadY => "dpad_y",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::LeftStickX => "Left Stick X",
            Self::LeftStickY => "Left Stick Y",
            Self::LeftZ => "Left Z",
            Self::RightStickX => "Right Stick X",
            Self::RightStickY => "Right Stick Y",
            Self::RightZ => "Right Z",
            Self::DPadX => "D-Pad X",
            Self::DPadY => "D-Pad Y",
        }
    }

    pub(super) fn from_gilrs(axis: Axis) -> Option<Self> {
        match axis {
            Axis::LeftStickX => Some(Self::LeftStickX),
            Axis::LeftStickY => Some(Self::LeftStickY),
            Axis::LeftZ => Some(Self::LeftZ),
            Axis::RightStickX => Some(Self::RightStickX),
            Axis::RightStickY => Some(Self::RightStickY),
            Axis::RightZ => Some(Self::RightZ),
            Axis::DPadX => Some(Self::DPadX),
            Axis::DPadY => Some(Self::DPadY),
            Axis::Unknown => None,
        }
    }

    pub(super) fn to_gilrs(self) -> Axis {
        match self {
            Self::LeftStickX => Axis::LeftStickX,
            Self::LeftStickY => Axis::LeftStickY,
            Self::LeftZ => Axis::LeftZ,
            Self::RightStickX => Axis::RightStickX,
            Self::RightStickY => Axis::RightStickY,
            Self::RightZ => Axis::RightZ,
            Self::DPadX => Axis::DPadX,
            Self::DPadY => Axis::DPadY,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum GamepadButton {
    South,
    East,
    North,
    West,
    C,
    Z,
    LeftTrigger,
    LeftTrigger2,
    RightTrigger,
    RightTrigger2,
    Select,
    Start,
    Mode,
    LeftThumb,
    RightThumb,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
}

impl GamepadButton {
    pub(crate) const ALL: [Self; 19] = [
        Self::South,
        Self::East,
        Self::North,
        Self::West,
        Self::C,
        Self::Z,
        Self::LeftTrigger,
        Self::LeftTrigger2,
        Self::RightTrigger,
        Self::RightTrigger2,
        Self::Select,
        Self::Start,
        Self::Mode,
        Self::LeftThumb,
        Self::RightThumb,
        Self::DPadUp,
        Self::DPadDown,
        Self::DPadLeft,
        Self::DPadRight,
    ];

    pub(crate) fn decl_id(self) -> &'static str {
        match self {
            Self::South => "south",
            Self::East => "east",
            Self::North => "north",
            Self::West => "west",
            Self::C => "c",
            Self::Z => "z",
            Self::LeftTrigger => "left_trigger",
            Self::LeftTrigger2 => "left_trigger_2",
            Self::RightTrigger => "right_trigger",
            Self::RightTrigger2 => "right_trigger_2",
            Self::Select => "select",
            Self::Start => "start",
            Self::Mode => "mode",
            Self::LeftThumb => "left_thumb",
            Self::RightThumb => "right_thumb",
            Self::DPadUp => "dpad_up",
            Self::DPadDown => "dpad_down",
            Self::DPadLeft => "dpad_left",
            Self::DPadRight => "dpad_right",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::South => "South",
            Self::East => "East",
            Self::North => "North",
            Self::West => "West",
            Self::C => "C",
            Self::Z => "Z",
            Self::LeftTrigger => "Left Trigger",
            Self::LeftTrigger2 => "Left Trigger 2",
            Self::RightTrigger => "Right Trigger",
            Self::RightTrigger2 => "Right Trigger 2",
            Self::Select => "Select",
            Self::Start => "Start",
            Self::Mode => "Mode",
            Self::LeftThumb => "Left Thumb",
            Self::RightThumb => "Right Thumb",
            Self::DPadUp => "D-Pad Up",
            Self::DPadDown => "D-Pad Down",
            Self::DPadLeft => "D-Pad Left",
            Self::DPadRight => "D-Pad Right",
        }
    }

    pub(super) fn from_gilrs(button: Button) -> Option<Self> {
        match button {
            Button::South => Some(Self::South),
            Button::East => Some(Self::East),
            Button::North => Some(Self::North),
            Button::West => Some(Self::West),
            Button::C => Some(Self::C),
            Button::Z => Some(Self::Z),
            Button::LeftTrigger => Some(Self::LeftTrigger),
            Button::LeftTrigger2 => Some(Self::LeftTrigger2),
            Button::RightTrigger => Some(Self::RightTrigger),
            Button::RightTrigger2 => Some(Self::RightTrigger2),
            Button::Select => Some(Self::Select),
            Button::Start => Some(Self::Start),
            Button::Mode => Some(Self::Mode),
            Button::LeftThumb => Some(Self::LeftThumb),
            Button::RightThumb => Some(Self::RightThumb),
            Button::DPadUp => Some(Self::DPadUp),
            Button::DPadDown => Some(Self::DPadDown),
            Button::DPadLeft => Some(Self::DPadLeft),
            Button::DPadRight => Some(Self::DPadRight),
            Button::Unknown => None,
        }
    }

    pub(super) fn to_gilrs(self) -> Button {
        match self {
            Self::South => Button::South,
            Self::East => Button::East,
            Self::North => Button::North,
            Self::West => Button::West,
            Self::C => Button::C,
            Self::Z => Button::Z,
            Self::LeftTrigger => Button::LeftTrigger,
            Self::LeftTrigger2 => Button::LeftTrigger2,
            Self::RightTrigger => Button::RightTrigger,
            Self::RightTrigger2 => Button::RightTrigger2,
            Self::Select => Button::Select,
            Self::Start => Button::Start,
            Self::Mode => Button::Mode,
            Self::LeftThumb => Button::LeftThumb,
            Self::RightThumb => Button::RightThumb,
            Self::DPadUp => Button::DPadUp,
            Self::DPadDown => Button::DPadDown,
            Self::DPadLeft => Button::DPadLeft,
            Self::DPadRight => Button::DPadRight,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Record {
    Catalog(Vec<DiscoveredGamepad>),
    Event(GamepadRuntimeEvent),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AxisProcessing {
    pub dead_zone: f64,
    pub offset: f64,
}
pub(crate) fn process_axis_value(raw_value: f64, dead_zone: f64, offset: f64) -> f64 {
    let value = (raw_value + offset).clamp(-1.0, 1.0);
    let dead_zone = dead_zone.clamp(0.0, 1.0);
    if dead_zone >= 1.0 || value.abs() <= dead_zone {
        return 0.0;
    }
    ((value.abs() - dead_zone) / (1.0 - dead_zone))
        .copysign(value)
        .clamp(-1.0, 1.0)
}
pub(crate) fn axis_index(axis: GamepadAxis) -> usize {
    GamepadAxis::ALL
        .iter()
        .position(|candidate| *candidate == axis)
        .unwrap()
}
