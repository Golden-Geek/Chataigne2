//! Typed command configuration projects to existing source intents; execution has no idle work.
mod kinds;
mod node;
pub(crate) use kinds::catalog;
pub use kinds::{MediaCommandCatalogItem, MediaCommandKind};
pub use node::MediaCommandNode;

pub(crate) const NODE_TYPE: &str = "mapgyver.media.command.v8";

fn kind_options() -> Vec<golden_core::parameter::ParameterEnumOption> {
    crate::parameters::enum_options(&MediaCommandKind::ALL.iter().map(|kind| kind.key()).collect::<Vec<_>>())
}

pub(crate) fn operation(
    engine: &crate::app::AppEngine,
    command: golden_core::node::NodeUuid,
) -> Result<super::protocol::MediaEdit, String> {
    use golden_core::node::Node;
    let id = engine.node_id_by_uuid(command).ok_or("media command is missing")?;
    let Some(crate::app::AppNode::MediaCommand(node)) = engine.nodes.get(id) else {
        return Err("reference is not a media command".into());
    };
    if node.node_data().parent != Some(super::authoring::manager(engine)?) {
        return Err("command is outside Medias".into());
    }
    node.operation()
}

#[cfg(test)]
mod tests;
