use super::*;

pub(super) fn script_move_request(args: &[ParamValue]) -> Option<Result<MouseMoveRequest, String>> {
    let Some(x) = args.first().and_then(param_value_as_number) else {
        return Some(Err("method 'moveMouse' expects a numeric x argument".to_string()));
    };
    let Some(y) = args.get(1).and_then(param_value_as_number) else {
        return Some(Err("method 'moveMouse' expects a numeric y argument".to_string()));
    };

    let coordinate = match args.get(2).and_then(ParamValue::as_str) {
        Some(value) => match MouseMoveCoordinate::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        },
        None => MouseMoveCoordinate::Absolute,
    };
    let units = match args.get(3).and_then(ParamValue::as_str) {
        Some(value) => match MouseMoveUnits::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(error)),
        },
        None => MouseMoveUnits::Pixels,
    };
    if coordinate == MouseMoveCoordinate::Relative && units == MouseMoveUnits::Normalized {
        return Some(Err(
            "method 'moveMouse' only supports normalized coordinates with absolute movement".to_string(),
        ));
    }

    Some(Ok(MouseMoveRequest {
        x,
        y,
        coordinate,
        units,
        description: "move mouse".to_string(),
    }))
}

pub(super) fn script_button_request(
    method: &str,
    args: &[ParamValue],
    action: MouseButtonAction,
) -> Option<Result<MouseButtonRequest, String>> {
    let button = match args.first().and_then(ParamValue::as_str) {
        Some(value) => match MouseButtonKind::parse(&value) {
            Ok(value) => value,
            Err(error) => return Some(Err(format!("method '{method}' {error}"))),
        },
        None => MouseButtonKind::Left,
    };

    Some(Ok(MouseButtonRequest {
        button,
        action,
        description: format!("{} {} mouse button", action.as_str(), button.as_str()),
    }))
}

pub(super) fn script_scroll_request(args: &[ParamValue]) -> Option<Result<MouseScrollRequest, String>> {
    let Some(vertical) = args.first().and_then(param_value_as_i32) else {
        return Some(Err("method 'scroll' expects a numeric vertical argument".to_string()));
    };

    Some(Ok(MouseScrollRequest {
        vertical,
        horizontal: args.get(1).and_then(param_value_as_i32).unwrap_or(0),
        description: "scroll mouse".to_string(),
    }))
}

pub(super) fn param_value_as_number(value: &ParamValue) -> Option<f64> {
    value.as_float().or_else(|| value.as_int().map(f64::from))
}

pub(super) fn param_value_as_i32(value: &ParamValue) -> Option<i32> {
    let number = param_value_as_number(value)?;
    if !number.is_finite() {
        return None;
    }
    let rounded = number.round();
    (rounded >= f64::from(i32::MIN) && rounded <= f64::from(i32::MAX)).then_some(rounded as i32)
}
