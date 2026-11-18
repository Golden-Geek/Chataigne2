use golden_engine::{
    app::{PREFERENCES_APP_DATA_TAG, PREFERENCES_DECL_ID},
    edit::NodeTree,
    node::{DeclId, Folder, Node},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, RangeConstraint},
    ui_read_model::UiSnapshotCapture,
};
use golden_protocol::UiNodeDataDto;

const CATEGORY: &str = "oscquery";
const PORT: &str = "port";
const ALLOW_LAN: &str = "allow_lan";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Settings {
    pub(super) port: u16,
    pub(super) allow_lan: bool,
}

pub(crate) fn preferences(port: u16) -> NodeTree {
    let mut folder = Folder::new("OSCQuery");
    let metadata = &mut folder.node_data_mut().meta;
    metadata.decl_id = DeclId(CATEGORY.into());
    metadata.can_be_disabled = true;
    metadata.enabled = false;
    metadata.tags.push(PREFERENCES_APP_DATA_TAG.into());
    metadata.description = Some("Browse and control this app with OSC.".into());
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
    let mut allow_lan = Parameter::new(
        "Allow LAN Control",
        ParamValue::Bool(false),
        ParameterChangeCheck::ValueChange,
    );
    let metadata = &mut allow_lan.node_data_mut().meta;
    metadata.decl_id = DeclId(ALLOW_LAN.into());
    metadata.tags.push(PREFERENCES_APP_DATA_TAG.into());
    metadata.description = Some("Advertise OSCQuery and allow devices on the local network to browse and edit this app. Enable on a trusted network.".into());
    allow_lan.control_modes_enabled = false;
    NodeTree::new(folder)
        .with_child(NodeTree::new(port))
        .with_child(NodeTree::new(allow_lan))
}

pub(super) fn enabled_settings(snapshot: &UiSnapshotCapture) -> Option<Settings> {
    let mut node = snapshot.node(snapshot.root()).filter(|root| root.meta.enabled)?;
    for declaration in [PREFERENCES_DECL_ID, CATEGORY] {
        node = node
            .children
            .iter()
            .filter_map(|id| snapshot.node(*id))
            .find(|child| child.decl_id.0 == declaration && child.meta.enabled)?;
    }
    let parameter = |declaration| {
        node.children
            .iter()
            .filter_map(|id| snapshot.node(*id))
            .find_map(|child| {
                let UiNodeDataDto::Parameter { param } = &child.data else {
                    return None;
                };
                (child.decl_id.0 == declaration && child.meta.enabled).then_some(&param.value)
            })
    };
    let ParamValue::Int(port) = parameter(PORT)? else {
        return None;
    };
    let port = u16::try_from(*port).ok().filter(|port| *port != 0)?;
    Some(Settings {
        port,
        allow_lan: matches!(parameter(ALLOW_LAN), Some(ParamValue::Bool(true))),
    })
}
