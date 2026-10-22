use super::*;
use golden_core::node::{NodeId, NodeUuid};
#[cfg(test)]
mod tests;

const MAX_ENTRIES: usize = 1024;
const MAX_BYTES: usize = 512 * 1024;
#[derive(Default)]
pub(super) struct Budget {
    entries: usize,
    bytes: usize,
}
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    directory: bool,
    length: u64,
    modified: u64,
}
#[derive(Clone)]
pub(super) struct Stored {
    uuid: NodeUuid,
    path: String,
    entries: BTreeMap<String, Stamp>,
    last_change: u64,
}

pub(super) fn sample(
    stored: &mut HashMap<NodeId, Stored>,
    entry: &super::super::super::WatchedFolderEntry,
    budget: &mut Budget,
) -> Result<FolderObservation, String> {
    let path = entry.target_path.trim();
    let status = kind(path)?;
    let mut entries = BTreeMap::new();
    if status == PathKind::Directory {
        collect(Path::new(path), Path::new(path), &mut entries, budget, 0)?;
    }
    let time = unix_time_ms();
    let mut update = FolderWatchUpdate {
        path: path.to_owned(),
        exists: matches!(status, PathKind::Directory | PathKind::File),
        entry_count: saturating_i32(entries.len()),
        timestamp_ms: time,
        ..Default::default()
    };
    if let Some(old) = stored
        .get(&entry.item_id)
        .filter(|old| old.path == path && old.uuid == entry.uuid)
    {
        update.last_change_ms = old.last_change;
        for (name, stamp) in &entries {
            match old.entries.get(name) {
                None => update.created.push(name.clone()),
                Some(previous) if previous != stamp => update.modified.push(name.clone()),
                _ => {}
            }
        }
        for name in old.entries.keys() {
            if !entries.contains_key(name) {
                update.removed.push(name.clone());
            }
        }
        if update.has_changes() {
            update.last_change_ms = time;
        }
    }
    stored.insert(
        entry.item_id,
        Stored {
            uuid: entry.uuid,
            path: path.to_owned(),
            entries,
            last_change: update.last_change_ms,
        },
    );
    Ok(FolderObservation { update, kind: status })
}
fn collect(
    root: &Path,
    current: &Path,
    entries: &mut BTreeMap<String, Stamp>,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("App Control folder traversal exceeds 32 levels; sample withheld".into());
    }
    let reader = fs::read_dir(current).map_err(|error| format!("App Control cannot read folder: {error}"))?;
    for entry in reader {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
        if relative.len() > 4096 {
            return Err("App Control folder entry exceeds 4096 bytes; sample withheld".into());
        }
        budget.entries += 1;
        budget.bytes += relative.len() + 64;
        if budget.entries > MAX_ENTRIES || budget.bytes > MAX_BYTES {
            return Err("App Control folder inventory exceeds 1024 entries / 512 KiB; sample withheld".into());
        }
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        entries.insert(
            relative.replace('\\', "/"),
            Stamp {
                directory: kind.is_dir(),
                length: metadata.len(),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                    .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0),
            },
        );
        if kind.is_dir() && !kind.is_symlink() {
            collect(root, &path, entries, budget, depth + 1)?;
        }
    }
    Ok(())
}
