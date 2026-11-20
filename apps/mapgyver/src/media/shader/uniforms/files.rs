//! Published shader input layouts for the current project's prepared file sources.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use golden_video::shader::{FragmentFormat, FragmentUniform};

use crate::media::protocol::{AssetLocator, ChannelReference};

pub(in crate::media) struct PreparedFile {
    pub reference: ChannelReference,
    pub format: FragmentFormat,
    pub file: AssetLocator,
    pub uniforms: Arc<[FragmentUniform]>,
}

struct Entry {
    file: PreparedFile,
    revision: uuid::Uuid,
}

#[derive(Default)]
struct Snapshot {
    owner: Weak<()>,
    entries: HashMap<ChannelReference, Entry>,
}

#[derive(Default)]
pub(crate) struct Catalog(Mutex<Snapshot>);

impl Catalog {
    pub(in crate::media) fn publish(&self, owner: &Arc<()>, files: Vec<PreparedFile>) {
        let mut current = self.0.lock().expect("shader files lock");
        let owner = Arc::downgrade(owner);
        let same_owner = owner.ptr_eq(&current.owner);
        current.entries = files
            .into_iter()
            .map(|file| {
                let previous = same_owner.then(|| current.entries.get(&file.reference)).flatten();
                let revision = previous
                    .filter(|entry| {
                        entry.file.file == file.file
                            && entry.file.format == file.format
                            && Arc::ptr_eq(&entry.file.uniforms, &file.uniforms)
                    })
                    .map_or_else(uuid::Uuid::new_v4, |entry| entry.revision);
                (file.reference, Entry { file, revision })
            })
            .collect();
        current.owner = owner;
    }

    pub(in crate::media) fn clear(&self, owner: &Arc<()>) {
        let mut current = self.0.lock().expect("shader files lock");
        if current.owner.ptr_eq(&Arc::downgrade(owner)) {
            current.entries.clear();
        }
    }

    pub(in crate::media) fn get(
        &self,
        reference: ChannelReference,
        format: FragmentFormat,
        file: &AssetLocator,
    ) -> Option<(uuid::Uuid, Arc<[FragmentUniform]>)> {
        let current = self.0.lock().expect("shader files lock");
        let _owner = current.owner.upgrade()?;
        let entry = current.entries.get(&reference)?;
        if entry.file.format != format || entry.file.file != *file {
            return None;
        }
        Some((entry.revision, entry.file.uniforms.clone()))
    }

    pub(crate) fn revision(&self, reference: ChannelReference) -> Option<uuid::Uuid> {
        let current = self.0.lock().expect("shader files lock");
        let _owner = current.owner.upgrade()?;
        Some(current.entries.get(&reference)?.revision)
    }
}

#[cfg(test)]
mod tests;
