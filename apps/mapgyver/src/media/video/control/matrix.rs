//! Matrix holds outgoing uses until its actual transition boundary; Video owns the shared clock.
use super::{State, VideoControl};
use crate::media::video::protocol::{VideoOnActivate, VideoOnDeactivate, VideoPlaybackMode};
use std::{collections::HashMap, sync::Arc};

#[derive(Default)]
pub(super) struct Uses {
    entries: HashMap<uuid::Uuid, Counts>,
    active: usize,
    pending: Option<Pending>,
}
#[derive(Clone, Copy, Default)]
struct Counts {
    total: usize,
    active: usize,
}
struct Pending {
    manual_revision: u64,
    policy: VideoOnDeactivate,
}

/// One Matrix consumer of a shared Video clock. Channels and References use the same owner
/// and activation identity. Keep an outgoing lease alive until the actual fade ends; dropping
/// an active lease is an immediate withdrawal. Neither operation retires decoded/GPU frames.
#[must_use = "retain the Video use until its outgoing Matrix transition completes"]
pub struct MatrixVideoUse {
    control: Option<Arc<VideoControl>>,
    activation: uuid::Uuid,
    generation: uuid::Uuid,
    active: bool,
}
impl MatrixVideoUse {
    pub(crate) fn is_current(&self) -> bool {
        self.control.as_ref().is_some_and(|control| {
            control
                .0
                .lock()
                .is_ok_and(|state| !state.retired && state.matrix_generation == self.generation)
        })
    }
    /// Begin outgoing retention. The last active consumer captures the deactivation policy
    /// and manual-intent fence. Repeated calls are harmless; no clock action happens here.
    pub fn deactivate(&mut self) -> Result<(), String> {
        if self.active {
            let control = self.control.as_ref().expect("live Matrix Video use");
            let mut state = control.0.lock().map_err(|_| "Video control poisoned")?;
            if state.matrix_generation == self.generation {
                state.deactivate_matrix(self.activation);
            }
            self.active = false;
        }
        Ok(())
    }

    /// Explicit completion returns transport errors to the owning Matrix service. Drop is
    /// the cancellation/withdrawal fallback and releases the exact same use once.
    pub fn finish(mut self) -> Result<(), String> {
        self.release()
    }

    fn release(&mut self) -> Result<(), String> {
        let Some(control) = self.control.take() else {
            return Ok(());
        };
        let mut state = control.0.lock().map_err(|_| "Video control poisoned")?;
        if state.matrix_generation != self.generation {
            return Ok(());
        }
        if self.active {
            state.deactivate_matrix(self.activation);
        }
        let counts = state
            .matrix_uses
            .entries
            .get_mut(&self.activation)
            .expect("registered Matrix Video use");
        counts.total -= 1;
        if counts.total == 0 {
            state.matrix_uses.entries.remove(&self.activation);
        }
        let pending = state
            .matrix_uses
            .entries
            .is_empty()
            .then(|| state.matrix_uses.pending.take())
            .flatten();
        let changed = if let Some(pending) =
            pending.filter(|pending| !state.retired && pending.manual_revision == state.manual_revision)
        {
            match pending.policy {
                VideoOnDeactivate::Nothing => false,
                VideoOnDeactivate::Stop => {
                    state.settings.mode = VideoPlaybackMode::Stopped;
                    state.retain_play_position = false;
                    state.settings.position_ms = 0.0;
                    state.seek_pending = false;
                    state.explicit_mode = true;
                    state.load_pending = false;
                    state.apply(false)?;
                    true
                }
                VideoOnDeactivate::Pause => {
                    state.settings.mode = VideoPlaybackMode::Paused;
                    state.retain_play_position = false;
                    state.explicit_mode = true;
                    state.load_pending = false;
                    state.apply(false)?;
                    true
                }
            }
        } else {
            false
        };
        drop(state);
        if changed {
            control.1.advance();
            control.publish_feedback()?;
        }
        Ok(())
    }
}
impl Drop for MatrixVideoUse {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            eprintln!("[mapgyver-video-matrix] release: {error}");
        }
    }
}

impl VideoControl {
    /// Acquire a committed Matrix activation. One identity shared by multiple channels or
    /// References applies On Activate once while any of its uses remain alive. Reactivation
    /// uses a fresh identity. An entirely outgoing identity admits only outgoing retention.
    /// Admit incoming uses before releasing outgoing ones on a Cut.
    pub fn activate_for_matrix(self: &Arc<Self>, activation: uuid::Uuid) -> Result<MatrixVideoUse, String> {
        let mut state = self.0.lock().map_err(|_| "Video control poisoned")?;
        if state.retired {
            return Err("Video media is retired".into());
        }
        let new = !state.matrix_uses.entries.contains_key(&activation);
        let current = state.matrix_uses.entries.get(&activation).copied().unwrap_or_default();
        let incoming = new || current.active != 0;
        let counts = Counts {
            total: current.total.checked_add(1).ok_or("Matrix Video use count exhausted")?,
            active: current
                .active
                .checked_add(usize::from(incoming))
                .ok_or("Matrix Video use count exhausted")?,
        };
        let active = state
            .matrix_uses
            .active
            .checked_add(usize::from(incoming))
            .ok_or("Matrix Video use count exhausted")?;
        let changed = if new {
            match state.settings.on_activate {
                VideoOnActivate::Nothing => false,
                policy => {
                    if policy == VideoOnActivate::PlayFromStart {
                        state.settings.position_ms = 0.0;
                        state.seek_pending = true;
                    }
                    state.settings.mode = VideoPlaybackMode::Playing;
                    state.retain_play_position = true;
                    state.explicit_mode = true;
                    state.load_pending = false;
                    state.apply(true)?;
                    true
                }
            }
        } else {
            false
        };
        state.matrix_uses.entries.insert(activation, counts);
        state.matrix_uses.active = active;
        // Late mirrors retain the outgoing intent fence; they never invent a reactivation.
        if incoming {
            state.matrix_uses.pending = None;
        }
        let generation = state.matrix_generation;
        drop(state);
        let lease = MatrixVideoUse {
            control: Some(self.clone()),
            activation,
            generation,
            active: incoming,
        };
        if changed {
            self.1.advance();
            self.publish_feedback()?;
        }
        Ok(lease)
    }
}

impl State {
    fn deactivate_matrix(&mut self, activation: uuid::Uuid) {
        let counts = self
            .matrix_uses
            .entries
            .get_mut(&activation)
            .expect("registered Matrix Video use");
        counts.active -= 1;
        self.matrix_uses.active -= 1;
        if self.matrix_uses.active == 0 {
            self.matrix_uses.pending = Some(Pending {
                manual_revision: self.manual_revision,
                policy: self.settings.on_deactivate,
            });
        }
    }
}
