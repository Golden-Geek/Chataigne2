#[derive(Debug)]
pub(crate) struct AudioThreadPriorityGuard {
    #[cfg(feature = "realtime")]
    handle: Option<audio_thread_priority::RtPriorityHandle>,
}

impl AudioThreadPriorityGuard {
    pub(crate) fn promote(buffer_frames: u32, sample_rate: u32) -> Result<Self, String> {
        #[cfg(feature = "realtime")]
        {
            let handle = audio_thread_priority::promote_current_thread_to_real_time(buffer_frames, sample_rate)
                .map_err(|error| error.to_string())?;
            Ok(Self { handle: Some(handle) })
        }
        #[cfg(not(feature = "realtime"))]
        {
            let _ = (buffer_frames, sample_rate);
            Ok(Self {})
        }
    }
}

impl Drop for AudioThreadPriorityGuard {
    fn drop(&mut self) {
        #[cfg(feature = "realtime")]
        if let Some(handle) = self.handle.take() {
            let _ = audio_thread_priority::demote_current_thread_from_real_time(handle);
        }
    }
}
