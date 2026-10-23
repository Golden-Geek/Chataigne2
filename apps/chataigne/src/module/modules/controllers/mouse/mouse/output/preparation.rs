use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{parameter::ParamValue, script::ScriptArguments};
fn text(value: &ParamValue) -> Result<&str, String> {
    let value = match value {
        ParamValue::Str(value) | ParamValue::Enum(value) => value.as_str(),
        _ => return Err("Mouse option must be a string or enum".into()),
    };
    if value.len() > 64 {
        return Err("Mouse option text exceeds 64 bytes".into());
    }
    Ok(value)
}
fn number(value: &ParamValue) -> Result<f64, String> {
    let value = value
        .as_float()
        .or_else(|| value.as_int().map(f64::from))
        .ok_or("Mouse coordinate must be numeric")?;
    if !value.is_finite() {
        return Err("Mouse coordinate must be finite".into());
    }
    Ok(value)
}
fn integer(value: &ParamValue) -> Result<i32, String> {
    let value = number(value)?.round();
    if value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err("Mouse scroll exceeds 32-bit range".into());
    }
    Ok(value as i32)
}
fn movement(x: f64, y: f64, coordinate: MouseMoveCoordinate, units: MouseMoveUnits) -> Result<Request, String> {
    if coordinate == MouseMoveCoordinate::Relative && units == MouseMoveUnits::Normalized {
        return Err("normalized mouse movement only supports absolute coordinates".into());
    }
    if !x.is_finite() || !y.is_finite() {
        return Err("Mouse coordinates must be finite".into());
    }
    if units == MouseMoveUnits::Pixels
        && [x, y]
            .into_iter()
            .any(|v| v.round() < f64::from(i32::MIN) || v.round() > f64::from(i32::MAX))
    {
        return Err("Mouse pixel coordinate exceeds 32-bit range".into());
    }
    Ok(Request::Move {
        x,
        y,
        coordinate,
        units,
    })
}
pub(super) fn script(method: &str, args: &ScriptArguments) -> Result<Request, String> {
    if args.len() > 4 || args.retained_bytes() > 64 * 1024 {
        return Err("Mouse scripts accept up to four arguments / 64 KiB".into());
    }
    match method {
        "moveMouse" => {
            let x = number(args.first().ok_or("moveMouse requires x and y")?)?;
            let y = number(args.get(1).ok_or("moveMouse requires x and y")?)?;
            let coordinate = args
                .get(2)
                .map(|v| MouseMoveCoordinate::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseMoveCoordinate::Absolute);
            let units = args
                .get(3)
                .map(|v| MouseMoveUnits::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseMoveUnits::Pixels);
            movement(x, y, coordinate, units)
        }
        "click" | "pressButton" | "releaseButton" => {
            if args.len() > 1 {
                return Err("Mouse button methods accept at most one argument".into());
            }
            let button = args
                .first()
                .map(|v| MouseButtonKind::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseButtonKind::Left);
            let action = match method {
                "click" => MouseButtonAction::Click,
                "pressButton" => MouseButtonAction::Press,
                _ => MouseButtonAction::Release,
            };
            Ok(Request::Button { button, action })
        }
        "scroll" => {
            if args.len() > 2 {
                return Err("scroll accepts at most two arguments".into());
            }
            Ok(Request::Scroll {
                vertical: integer(args.first().ok_or("scroll requires vertical amount")?)?,
                horizontal: args.get(1).map(integer).transpose()?.unwrap_or(0),
            })
        }
        _ => Err("Unknown Mouse script method".into()),
    }
}
pub(super) fn command(invocation: &CapturedInvocation) -> Result<Request, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|event| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**event).clone()).map_err(|error| error.to_string())
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("Mouse invocation targets another command".into());
    }
    let field = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|event| event.param_overrides.iter().rev().find(|value| value.param_id == id))
            .map(|value| &value.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    match invocation
        .snapshot
        .node(invocation.command)
        .ok_or("Mouse command source is unavailable")?
        .node_type
        .as_str()
    {
        MOUSE_MOVE_COMMAND_NODE_TYPE => movement(
            field("x").map(number).transpose()?.unwrap_or(0.0),
            field("y").map(number).transpose()?.unwrap_or(0.0),
            field("coordinate")
                .map(|v| MouseMoveCoordinate::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseMoveCoordinate::Absolute),
            field("units")
                .map(|v| MouseMoveUnits::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseMoveUnits::Pixels),
        ),
        MOUSE_BUTTON_COMMAND_NODE_TYPE => Ok(Request::Button {
            button: field("button")
                .map(|v| MouseButtonKind::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseButtonKind::Left),
            action: field("action")
                .map(|v| MouseButtonAction::parse(text(v)?))
                .transpose()?
                .unwrap_or(MouseButtonAction::Click),
        }),
        MOUSE_SCROLL_COMMAND_NODE_TYPE => Ok(Request::Scroll {
            vertical: field("vertical").map(integer).transpose()?.unwrap_or(0),
            horizontal: field("horizontal").map(integer).transpose()?.unwrap_or(0),
        }),
        _ => Err("Unsupported Mouse command type".into()),
    }
}
