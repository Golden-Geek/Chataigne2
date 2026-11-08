//! Exact-adapter file decode and asynchronous, clock-fenced GPU publication.
use super::*;
use golden_video::native::{GpuFrameError, GpuFramePublisher};

impl From<GpuFrameError> for Failure {
    fn from(error: GpuFrameError) -> Self {
        let kind = match &error {
            GpuFrameError::Unsupported | GpuFrameError::Layout => VideoSourceErrorKind::Unsupported,
            GpuFrameError::Capacity => VideoSourceErrorKind::Capacity,
            GpuFrameError::Decode(_) | GpuFrameError::Native(_) => VideoSourceErrorKind::Decode,
        };
        Self(VideoSourceError::new(kind, error.to_string()))
    }
}

impl Run {
    pub(super) fn play_hardware(
        &self,
        worker: &WorkerContext<(), ()>,
        owner: uuid::Uuid,
        hardware: &Hardware,
    ) -> Result<(), Failure> {
        let cancel = worker.cancellation().clone();
        let decoder = Decoder::open_d3d11_with_interrupt(
            &self.file,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
            },
            hardware.adapter,
            hardware.budget.clone(),
            move || cancel.is_cancelled(),
        )?;
        let mut reader = Reader::new(decoder)?;
        self.control
            .channel_duration(self.reference.channel, owner, reader.duration)?;
        let mut publisher: Option<GpuFramePublisher> = None;
        let mut pending = None;
        let mut installed = false;
        while !worker.cancellation().is_cancelled() {
            let checkpoint = worker.wake_checkpoint();
            let revision = self.control.revisions();
            let _subscription = revision.guard_at(revision.current()).subscribe(worker.wake_notifier());
            let sample = self.control.sample()?;
            let mut waiting = false;
            if let Some(publisher) = &mut publisher {
                if let Some(image) = publisher.poll()? {
                    let expected = pending.take().expect("one clock fence per submitted conversion");
                    let stream = publisher.stream();
                    if self
                        .control
                        .publish_gpu_frame(self.reference.channel, owner, expected, &stream, image)?
                    {
                        if !installed {
                            let mut output = self.output.lock().unwrap();
                            output.stream = Some(Stream::Hardware(stream));
                            output.error = None;
                            drop(output);
                            installed = true;
                            eprintln!(
                                "[mapgyver-video] D3D11 resident playback: {} on {:?}",
                                self.file.display(),
                                hardware.adapter
                            );
                            self.changed();
                        }
                        if let Some(frame) = reader.neighbours()? {
                            self.control
                                .observe_neighbours(self.reference.channel, owner, expected, frame)?;
                        }
                    } else {
                        reader.invalidate_publication();
                    }
                }
                waiting = publisher.is_pending();
            }
            if !waiting {
                if reader.advance(sample.channel_position(reader.duration))? {
                    let frame = reader
                        .frame()
                        .d3d11_frame()
                        .ok_or("hardware decoder returned software storage")?;
                    if publisher.is_none() {
                        publisher = Some(GpuFramePublisher::new(
                            frame,
                            &hardware.budget,
                            Some(worker.wake_notifier()),
                        )?);
                    }
                    if publisher.as_mut().unwrap().submit(frame)? {
                        pending = Some(sample);
                        reader.mark_published();
                    }
                    // Retry a full conversion ring even while paused. Slot release also wakes us.
                    waiting = true;
                } else {
                    if let Some(frame) = reader.neighbours()? {
                        self.control
                            .observe_neighbours(self.reference.channel, owner, sample, frame)?;
                    }
                    if !sample.playing {
                        reader.prepare_previous()?;
                    }
                }
            }
            let delay = if waiting || reader.catching_up {
                Duration::from_millis(1)
            } else if sample.playing {
                Duration::from_millis(8)
            } else {
                Duration::from_secs(3600)
            };
            worker.wait_until(checkpoint, Instant::now() + delay);
        }
        Ok(())
    }
}
