//! Product schema admission before Golden's recoverable node codec can skip records.
use golden_core::engine::ProjectFile;

pub(crate) fn validate_project_file(project: &ProjectFile) -> Result<(), String> {
    let mut pending = vec![&project.root];
    while let Some(record) = pending.pop() {
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
