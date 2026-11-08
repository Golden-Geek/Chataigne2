//! Authored File membership outlives render demand, but not the owning channel incarnation.
use super::VideoControl;
use crate::media::protocol::{AssetLocator, ChannelId};
use golden_core::{node::NodeId, process_ctx::ProcessCtx};
use std::sync::{Arc, Mutex, Weak};

struct State {
    file: Option<AssetLocator>,
    target: Weak<VideoControl>,
}
pub(crate) struct Registration {
    pub channel: ChannelId,
    pub owner: uuid::Uuid,
    state: Mutex<State>,
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
            target.bind_file(self.channel, self.owner, state.file.clone())?;
        }
        Ok(())
    }
    pub fn bind(&self, control: &Arc<VideoControl>) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|_| "Video binding poisoned")?;
        control.bind_file(self.channel, self.owner, state.file.clone())?;
        state.target = Arc::downgrade(control);
        Ok(())
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
    pub metadata_error: Option<super::protocol::VideoSourceError>,
}
