//! Bounded CPU metadata only; revisions fence source changes, Weak ownership fences renderer stop.
use super::ChannelResolution;
use crate::media::protocol::ChannelReference;
use golden_io::RevisionGuard;
use golden_video::Extent2D;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

pub(crate) struct ResolutionSnapshot {
    guards: Arc<[RevisionGuard]>,
    owner: Weak<()>,
    channels: HashMap<ChannelReference, ChannelResolution>,
}
impl ResolutionSnapshot {
    pub fn new(
        channels: impl Iterator<Item = (ChannelReference, ChannelResolution)>,
        guards: Arc<[RevisionGuard]>,
        owner: Weak<()>,
    ) -> Result<Self, String> {
        let mut result = HashMap::new();
        for (reference, resolution) in channels {
            if result.len() == 4096 {
                return Err("source resolution metadata exceeds 4096 channels".into());
            }
            result.insert(reference, resolution);
        }
        Ok(Self {
            guards,
            owner,
            channels: result,
        })
    }
}

#[derive(Default)]
pub(crate) struct RuntimeResolutions(Mutex<Option<ResolutionSnapshot>>);
impl RuntimeResolutions {
    pub fn publish(&self, snapshot: ResolutionSnapshot) {
        *self.0.lock().expect("source resolution lock poisoned") = Some(snapshot);
    }
    pub fn extent(&self, reference: ChannelReference) -> Option<Extent2D> {
        let current = self.0.lock().expect("source resolution lock poisoned");
        let snapshot = current.as_ref()?;
        let _owner = snapshot.owner.upgrade()?;
        if !snapshot.guards.iter().all(RevisionGuard::is_current) {
            return None;
        }
        snapshot.channels.get(&reference)?.extent
    }
}
