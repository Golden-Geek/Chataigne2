//! Project-scoped weak media registrations; authored nodes retain the actual transport owners.
use super::VideoControl;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::MediaId,
};
use golden_io::RevisionGuard;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

#[derive(Default)]
pub(crate) struct Registry(Mutex<HashMap<MediaId, Weak<VideoControl>>>);
impl Registry {
    pub fn register(&self, media: MediaId, control: Weak<VideoControl>) -> Result<(), String> {
        let mut owners = self.0.lock().map_err(|_| "Video registry poisoned")?;
        if !owners.contains_key(&media) && owners.len() >= 4096 {
            owners.retain(|_, owner| owner.strong_count() != 0);
            if owners.len() >= 4096 {
                return Err("Video metadata exceeds 4096 media owners".into());
            }
        }
        owners.insert(media, control);
        Ok(())
    }
    pub(super) fn controls(&self) -> Vec<Arc<VideoControl>> {
        let mut owners = self.0.lock().unwrap();
        owners.retain(|_, owner| owner.strong_count() != 0);
        owners
            .values()
            .filter_map(Weak::upgrade)
            .filter(|control| control.needs_metadata())
            .collect()
    }
}
pub(super) struct Source {
    pub registry: Arc<Registry>,
    pub guard: RevisionGuard,
}
impl Source {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        let id = crate::media::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(id) else {
            unreachable!()
        };
        Ok(Self {
            registry: manager.videos.clone(),
            guard: manager.project_guard(),
        })
    }
}
