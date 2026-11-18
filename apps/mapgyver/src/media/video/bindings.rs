//! Authored File membership outlives render demand, but not the owning channel incarnation.
use super::VideoControl;
use super::protocol::{VideoSubtitleSettings, VideoSubtitleTrackInfo};
use crate::media::protocol::{AssetLocator, ChannelId};
use golden_core::{engine::NodeWaker, node::NodeId, process_ctx::ProcessCtx};
use std::sync::{Arc, Mutex, Weak};

struct State {
    file: Option<AssetLocator>,
    target: Weak<VideoControl>,
}
pub(crate) struct Registration {
    pub channel: ChannelId,
    pub owner: uuid::Uuid,
    state: Mutex<State>,
    subtitles: Arc<ChannelSubtitles>,
}
impl Registration {
    pub fn new(channel: ChannelId, file: Option<AssetLocator>) -> Arc<Self> {
        Arc::new(Self {
            channel,
            owner: uuid::Uuid::new_v4(),
            state: Mutex::new(State {
                file,
                target: Weak::new(),
            }),
            subtitles: Arc::default(),
        })
    }
    pub fn announce(self: &Arc<Self>, ctx: &mut ProcessCtx, node: NodeId, parent: NodeId) {
        let binding = Arc::downgrade(self);
        ctx.register_ancestor_runtime(node, move |owner| {
            let Some(media) = owner.as_any().downcast_ref::<crate::media::MediaNode>() else {
                return Err("Video channel must belong directly to a media".into());
            };
            if owner.id() != parent {
                return Err("Video channel owner changed during registration".into());
            }
            if let Some(binding) = binding.upgrade() {
                binding.bind(
                    &media
                        .video_control()
                        .ok_or("Video channel belongs to a different media kind")?,
                )?;
            }
            Ok(true)
        });
    }
    pub fn set_file(&self, file: Option<AssetLocator>) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Video binding poisoned")?;
        state.file = file;
        if let Some(target) = state.target.upgrade() {
            target.bind_file_with_subtitles(self.channel, self.owner, state.file.clone(), self.subtitles.clone())?;
        }
        Ok(())
    }
    pub fn bind(&self, control: &Arc<VideoControl>) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Video binding poisoned")?;
        control.bind_file_with_subtitles(self.channel, self.owner, state.file.clone(), self.subtitles.clone())?;
        state.target = Arc::downgrade(control);
        Ok(())
    }
    pub(crate) fn set_subtitle_settings(&self, settings: VideoSubtitleSettings) -> Result<(), String> {
        settings.validate()?;
        let mut state = self.subtitles.state.lock().map_err(|_| "Video subtitles poisoned")?;
        if *state.settings != settings {
            state.settings = Arc::new(settings);
            self.subtitles.revision.advance();
        }
        Ok(())
    }

    pub(crate) fn watch_subtitle_tracks(&self, waker: NodeWaker) -> Result<(), String> {
        self.subtitles
            .state
            .lock()
            .map_err(|_| "Video subtitles poisoned")?
            .waker = Some(waker);
        Ok(())
    }

    pub(crate) fn subtitle_tracks(&self) -> Arc<[VideoSubtitleTrackInfo]> {
        self.subtitles
            .state
            .lock()
            .map(|state| state.tracks.clone())
            .unwrap_or_default()
    }
    pub fn retire_media(&self) {
        if let Ok(state) = self.state.lock()
            && let Some(target) = state.target.upgrade()
        {
            target.retire_media();
        }
    }
    pub fn release(&self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(target) = state.target.upgrade() {
                target.unbind_file(self.channel, self.owner);
            }
            state.target = Weak::new();
        }
    }
}

#[derive(Default)]
struct SubtitleState {
    settings: Arc<VideoSubtitleSettings>,
    tracks: Arc<[VideoSubtitleTrackInfo]>,
    waker: Option<NodeWaker>,
}

/// One authored File binding owns one independent subtitle selection and catalog.
#[derive(Default)]
pub(crate) struct ChannelSubtitles {
    state: Mutex<SubtitleState>,
    revision: golden_io::RevisionSource,
}

impl ChannelSubtitles {
    #[cfg(all(windows, feature = "native-video", feature = "video-files"))]
    pub(super) fn settings(&self) -> Result<Arc<VideoSubtitleSettings>, String> {
        Ok(self
            .state
            .lock()
            .map_err(|_| "Video subtitles poisoned")?
            .settings
            .clone())
    }

    #[cfg(all(windows, feature = "native-video", feature = "video-files"))]
    pub(super) fn subscribe(&self, notifier: golden_io::WorkerNotifier) -> golden_io::RevisionSubscription {
        let reader = self.revision.reader();
        reader.guard_at(reader.current()).subscribe(notifier)
    }

    pub(super) fn set_tracks(&self, tracks: Vec<VideoSubtitleTrackInfo>) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Video subtitles poisoned")?;
        if *state.tracks == tracks {
            return Ok(());
        }
        state.tracks = Arc::from(tracks);
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            // The node can retire after a current metadata probe. No retired inbox can receive it.
            let _ = waker.wake();
        }
        Ok(())
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Ok(state) = self.state.get_mut()
            && let Some(target) = state.target.upgrade()
        {
            target.unbind_file(self.channel, self.owner);
        }
    }
}

pub(super) struct File {
    pub owner: uuid::Uuid,
    #[cfg(any(feature = "video-files", test))]
    pub generation: uuid::Uuid,
    pub locator: Option<AssetLocator>,
    pub duration: Option<std::time::Duration>,
    pub subtitles: Arc<ChannelSubtitles>,
    pub metadata_error: Option<super::protocol::VideoSourceError>,
}
