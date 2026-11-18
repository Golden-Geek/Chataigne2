//! Cell authoring and the mixer-facing projection of the committed Matrix timeline.
use crate::{audio::voice, media::protocol::AssetLocator};
use std::sync::Mutex;

#[derive(Clone)]
pub(crate) struct CellAudio {
    pub enabled: bool,
    pub override_media: bool,
    pub file: Option<AssetLocator>,
    pub routing: voice::Routing,
}

#[derive(Default)]
pub(crate) struct CellBinding {
    state: Mutex<Option<CellAudio>>,
    pub options: voice::Binding,
}

impl CellBinding {
    pub fn publish(&self, audio: CellAudio) {
        *self.state.lock().unwrap() = Some(audio);
    }
    pub fn disable(&self) {
        self.state.lock().unwrap().take();
    }
    #[cfg(feature = "video-files")]
    pub(super) fn snapshot(&self) -> Option<CellAudio> {
        self.state.lock().unwrap().clone()
    }
}

#[cfg(feature = "video-files")]
mod runtime;
#[cfg(feature = "video-files")]
pub(crate) use runtime::*;
