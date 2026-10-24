use super::*;

pub(super) fn value_folder(label: &str, decl_id: &str, uuid: NodeUuid) -> Folder {
    let mut folder = Folder::new(label);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    let meta = &mut folder.node_data_mut().meta;
    meta.uuid = uuid;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    meta.user_permissions.can_edit_name = false;
    folder
}

pub(super) fn source_value_param(
    target: &SpatializerEndpointConfig,
    source: &SpatializerEndpointConfig,
    value: f64,
) -> Parameter {
    let mut parameter = read_only_param(
        source.label.as_str(),
        source.value_decl_id.as_str(),
        source_value_uuid(target.item_uuid, source.item_uuid),
        ParamValue::Float(value),
    );
    parameter.constraints = unit_value_constraints();
    parameter
}

pub(super) fn target_value_param(
    source: &SpatializerEndpointConfig,
    target: &SpatializerEndpointConfig,
    value: f64,
) -> Parameter {
    let mut parameter = read_only_param(
        target.label.as_str(),
        target.value_decl_id.as_str(),
        target_value_uuid(source.item_uuid, target.item_uuid),
        ParamValue::Float(value),
    );
    parameter.constraints = unit_value_constraints();
    parameter
}

pub(super) fn read_only_param(label: &str, decl_id: &str, uuid: NodeUuid, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.uuid = uuid;
    meta.decl_id = DeclId(decl_id.to_string());
    meta.short_name = decl_id.to_string();
    meta.user_permissions.can_edit_name = false;
    parameter
}

pub(super) fn position_param(dimension: SpatializerDimension, position: SpatialPoint) -> Parameter {
    let value = match dimension {
        SpatializerDimension::Two => ParamValue::Vec2(position.x, position.y),
        SpatializerDimension::Three => ParamValue::Vec3(position.x, position.y, position.z),
    };
    let mut parameter = Parameter::new("Position", value, ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(dimension.position_decl_id().to_string());
    meta.short_name = dimension.position_decl_id().to_string();
    parameter
}

pub(super) fn radius_param(radius: f64) -> Parameter {
    let mut parameter = Parameter::new(
        "Radius",
        ParamValue::Float(radius.max(0.0)),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.range = RangeConstraint::uniform(Some(0.0), None);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(RADIUS_DECL_ID.to_string());
    meta.short_name = RADIUS_DECL_ID.to_string();
    parameter
}

pub(super) fn freeze_radius_param(radius: f64) -> Parameter {
    let mut parameter = Parameter::new(
        "Freeze Radius",
        ParamValue::Float(radius.max(0.0)),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.range = RangeConstraint::uniform(Some(0.0), None);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    let meta = &mut parameter.node_data_mut().meta;
    meta.decl_id = DeclId(FREEZE_RADIUS_DECL_ID.to_string());
    meta.short_name = FREEZE_RADIUS_DECL_ID.to_string();
    parameter
}

pub(super) fn unit_value_constraints() -> golden_core::parameter::ParameterConstraints {
    golden_core::parameter::ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        ..Default::default()
    }
}
