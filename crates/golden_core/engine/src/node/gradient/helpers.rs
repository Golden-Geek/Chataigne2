use super::prelude::*;

pub(super) fn make_position_parameter(default_value: f64) -> Parameter {
    let mut parameter = Parameter::new(
        "Position",
        ParamValue::Float(default_value.clamp(0.0, 1.0)),
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id = DeclId(GRADIENT_STOP_POSITION_DECL_ID.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter.constraints.range = Some(RangeConstraint::Uniform {
        min: Some(0.0),
        max: Some(1.0),
    });
    parameter
}

pub(super) fn make_color_parameter(color: Color) -> Parameter {
    let mut parameter = Parameter::new(
        "Color",
        ParamValue::Color(color.r(), color.g(), color.b(), color.a()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id = DeclId(GRADIENT_STOP_COLOR_DECL_ID.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter
}

pub(super) fn make_interpolation_parameter(interpolation: GradientInterpolation) -> Parameter {
    let mut parameter = Parameter::new(
        "Interpolation",
        ParamValue::Enum(interpolation.variant_id().to_string()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.decl_id = DeclId(GRADIENT_STOP_INTERPOLATION_DECL_ID.to_string());
    parameter.node_data_mut().meta.can_be_disabled = false;
    parameter.constraints.enum_options = vec![
        enum_option(GradientInterpolation::None.variant_id(), "None", 0),
        enum_option(GradientInterpolation::Linear.variant_id(), "Linear", 1),
        enum_option(GradientInterpolation::Smooth.variant_id(), "Smooth", 2),
    ];
    parameter
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

pub(super) fn read_child_param_value<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    let child = snapshot.find_child(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

pub(super) fn read_child_param_f64(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: f64,
) -> f64 {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(|value| {
            value
                .as_float()
                .or_else(|| value.as_int().map(|int_value| int_value as f64))
        })
        .unwrap_or(default_value)
}

pub(super) fn read_child_param_color(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: Color,
) -> Color {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(ParamValue::as_color)
        .map(|(red, green, blue, alpha)| Color::new(red, green, blue, alpha))
        .unwrap_or(default_value)
}

pub(super) fn read_child_param_enum(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
    default_value: &str,
) -> String {
    read_child_param_value(snapshot, parent, decl_id)
        .and_then(ParamValue::as_enum)
        .unwrap_or_else(|| default_value.to_string())
}
