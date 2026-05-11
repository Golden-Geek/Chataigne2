use serde::{Deserialize, Serialize};

pub(crate) const MOUSE_MOVE_COMMAND_NODE_TYPE: &str = "mouse_move_command";
pub(crate) const MOUSE_BUTTON_COMMAND_NODE_TYPE: &str = "mouse_button_command";
pub(crate) const MOUSE_SCROLL_COMMAND_NODE_TYPE: &str = "mouse_scroll_command";

pub(crate) const MOUSE_COORDINATE_ABSOLUTE: &str = "absolute";
pub(crate) const MOUSE_COORDINATE_RELATIVE: &str = "relative";

pub(crate) const MOUSE_UNITS_PIXELS: &str = "pixels";
pub(crate) const MOUSE_UNITS_NORMALIZED: &str = "normalized";

pub(crate) const MOUSE_BUTTON_LEFT: &str = "left";
pub(crate) const MOUSE_BUTTON_MIDDLE: &str = "middle";
pub(crate) const MOUSE_BUTTON_RIGHT: &str = "right";

pub(crate) const MOUSE_ACTION_CLICK: &str = "click";
pub(crate) const MOUSE_ACTION_PRESS: &str = "press";
pub(crate) const MOUSE_ACTION_RELEASE: &str = "release";

pub(crate) const MOUSE_MODULE_COMMAND_TYPES: &[&str] = &[
    MOUSE_MOVE_COMMAND_NODE_TYPE,
    MOUSE_BUTTON_COMMAND_NODE_TYPE,
    MOUSE_SCROLL_COMMAND_NODE_TYPE,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MouseMoveCoordinate {
    Absolute,
    Relative,
}

impl MouseMoveCoordinate {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case(MOUSE_COORDINATE_ABSOLUTE) {
            return Ok(Self::Absolute);
        }
        if value.eq_ignore_ascii_case(MOUSE_COORDINATE_RELATIVE) {
            return Ok(Self::Relative);
        }

        Err(format!(
            "unsupported mouse coordinate mode '{value}', expected '{MOUSE_COORDINATE_ABSOLUTE}' or '{MOUSE_COORDINATE_RELATIVE}'"
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MouseMoveUnits {
    Pixels,
    Normalized,
}

impl MouseMoveUnits {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case(MOUSE_UNITS_PIXELS) {
            return Ok(Self::Pixels);
        }
        if value.eq_ignore_ascii_case(MOUSE_UNITS_NORMALIZED) {
            return Ok(Self::Normalized);
        }

        Err(format!(
            "unsupported mouse coordinate units '{value}', expected '{MOUSE_UNITS_PIXELS}' or '{MOUSE_UNITS_NORMALIZED}'"
        ))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MouseButtonKind {
    Left,
    Middle,
    Right,
}

impl MouseButtonKind {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case(MOUSE_BUTTON_LEFT) {
            return Ok(Self::Left);
        }
        if value.eq_ignore_ascii_case(MOUSE_BUTTON_MIDDLE) {
            return Ok(Self::Middle);
        }
        if value.eq_ignore_ascii_case(MOUSE_BUTTON_RIGHT) {
            return Ok(Self::Right);
        }

        Err(format!(
            "unsupported mouse button '{value}', expected one of '{MOUSE_BUTTON_LEFT}', '{MOUSE_BUTTON_MIDDLE}', '{MOUSE_BUTTON_RIGHT}'"
        ))
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Left => MOUSE_BUTTON_LEFT,
            Self::Middle => MOUSE_BUTTON_MIDDLE,
            Self::Right => MOUSE_BUTTON_RIGHT,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Middle => "Middle",
            Self::Right => "Right",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MouseButtonAction {
    Click,
    Press,
    Release,
}

impl MouseButtonAction {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case(MOUSE_ACTION_CLICK) {
            return Ok(Self::Click);
        }
        if value.eq_ignore_ascii_case(MOUSE_ACTION_PRESS) {
            return Ok(Self::Press);
        }
        if value.eq_ignore_ascii_case(MOUSE_ACTION_RELEASE) {
            return Ok(Self::Release);
        }

        Err(format!(
            "unsupported mouse button action '{value}', expected '{MOUSE_ACTION_CLICK}', '{MOUSE_ACTION_PRESS}', or '{MOUSE_ACTION_RELEASE}'"
        ))
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Click => MOUSE_ACTION_CLICK,
            Self::Press => MOUSE_ACTION_PRESS,
            Self::Release => MOUSE_ACTION_RELEASE,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MouseMoveRequest {
    pub x: f64,
    pub y: f64,
    pub coordinate: MouseMoveCoordinate,
    pub units: MouseMoveUnits,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MouseButtonRequest {
    pub button: MouseButtonKind,
    pub action: MouseButtonAction,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MouseScrollRequest {
    pub vertical: i32,
    pub horizontal: i32,
    pub description: String,
}