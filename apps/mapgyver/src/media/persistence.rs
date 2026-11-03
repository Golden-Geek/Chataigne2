//! Product schema admission before Golden's recoverable node codec can skip records.
use golden_core::engine::ProjectFile;

pub(crate) fn validate_project_file(project: &ProjectFile) -> Result<(), String> {
    let mut pending = vec![&project.root];
    while let Some(record) = pending.pop() {
        if record.node_type.starts_with("mapgyver.matrix.cell")
            && record.node_type != crate::matrix::cell_node::CELL_TYPE
        {
            return Err(format!("Unsupported Matrix Cell schema '{}'", record.node_type));
        }
        if record.node_type.starts_with("mapgyver.composition.placement")
            && record.node_type != super::composition::PLACEMENT_TYPE
        {
            return Err(format!(
                "Unsupported Composition Placement schema '{}'",
                record.node_type
            ));
        }
        if record.node_type == "mapgyver.composition.instance" {
            return Err(
                "Unsupported reference-only Composition instance schema; use child media with Placement v1".into(),
            );
        }
        if record.node_type.starts_with("mapgyver.media.reference") && record.node_type != super::reference::NODE_TYPE {
            return Err(format!("Unsupported Reference media schema '{}'", record.node_type));
        }
        if (record.node_type.starts_with("mapgyver.media.command") && record.node_type != super::commands::NODE_TYPE)
            || record.node_type == "mapgyver_media_command"
        {
            return Err(format!(
                "Unsupported MapGyver command schema '{}': typed commands require {}",
                record.node_type,
                super::commands::NODE_TYPE
            ));
        }
        if record.node_type == "mapgyver.media"
            || (record.node_type.starts_with("mapgyver.media.v") && record.node_type != "mapgyver.media.v5")
        {
            return Err(format!(
                "Unsupported MapGyver media schema '{}': media-owned procedural settings require mapgyver.media.v5",
                record.node_type
            ));
        }
        pending.extend(&record.children);
    }
    Ok(())
}
