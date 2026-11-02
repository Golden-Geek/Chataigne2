//! Product schema admission before Golden's recoverable node codec can skip records.
use golden_core::engine::ProjectFile;

pub(crate) fn validate_project_file(project: &ProjectFile) -> Result<(), String> {
    let mut pending = vec![&project.root];
    while let Some(record) = pending.pop() {
        if record.node_type == "mapgyver.media" {
            return Err(concat!(
                "Unsupported MapGyver media schema 'mapgyver.media': ",
                "per-channel sizing projects cannot be loaded as shared-Resolution media"
            )
            .into());
        }
        pending.extend(&record.children);
    }
    Ok(())
}
