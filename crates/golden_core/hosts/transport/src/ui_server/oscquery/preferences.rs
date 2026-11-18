use golden_engine::{
    app::{PREFERENCES_APP_DATA_TAG, PREFERENCES_DECL_ID},
    edit::NodeTree,
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, RangeConstraint},
    ui_read_model::UiReadModel,
};
use golden_protocol::{UiNodeDataDto, UiSubscriptionScope};

const CATEGORY: &str = "oscquery";
const PORT: &str = "port";

pub(crate) fn preferences(port: u16) -> NodeTree {
    let mut folder = Folder::new("OSCQuery");
    let metadata = &mut folder.node_data_mut().meta;
    metadata.decl_id = DeclId(CATEGORY.into());
    metadata.can_be_disabled = true;
    metadata.enabled = false;
    metadata.tags.push(PREFERENCES_APP_DATA_TAG.into());
    metadata.description = Some("Publish this app's OSCQuery hierarchy on the local machine.".into());
    let mut port = Parameter::new(
        "Port",
        ParamValue::Int(i32::from(port.max(1))),
        ParameterChangeCheck::ValueChange,
    );
    port.node_data_mut().meta.decl_id = DeclId(PORT.into());
    port.node_data_mut().meta.tags.push(PREFERENCES_APP_DATA_TAG.into());
    port.constraints.range = Some(RangeConstraint::Uniform {
        min: Some(1.0),
        max: Some(f64::from(u16::MAX)),
    });
    port.control_modes_enabled = false;
    NodeTree::new(folder).with_child(NodeTree::new(port))
}

pub(super) fn enabled_port(model: &UiReadModel) -> Option<u16> {
    let snapshot = model.capture_snapshot(UiSubscriptionScope::WholeGraph);
    let mut node = snapshot.node(snapshot.root())?;
    for declaration in [PREFERENCES_DECL_ID, CATEGORY, PORT] {
        if !node.meta.enabled {
            return None;
        }
        node = node
            .children
            .iter()
            .filter_map(|id| snapshot.node(*id))
            .find(|child| child.decl_id.0 == declaration)?;
    }
    let UiNodeDataDto::Parameter { param } = &node.data else {
        return None;
    };
    match param.value {
        ParamValue::Int(port) if node.meta.enabled => u16::try_from(port).ok().filter(|port| *port != 0),
        _ => None,
    }
}
