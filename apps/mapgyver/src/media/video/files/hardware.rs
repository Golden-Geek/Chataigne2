//! Exact-adapter file decode and asynchronous, clock-fenced GPU publication.
use super::*;
use golden_video::decode::{DecodeStorage, HapFormat};
use golden_video::native::{GpuFrameError, GpuFramePublisher};

pub(super) enum NativeFailure {
    Fallback(Failure),
    Retry(Failure),
}

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
    ) -> Result<(), NativeFailure> {
        let cancel = worker.cancellation().clone();
        let decoder = Decoder::open_native_with_interrupt(
            &self.file,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: golden_video::decode::SoftwareDecodeThreads::One,
            },
            hardware.adapter,
            hardware.budget.clone(),
            move || cancel.is_cancelled(),
        )
        .map_err(|error| NativeFailure::Fallback(error.into()))?;
        let compressed = decoder.storage() == DecodeStorage::Hap;
        self.drive_native(worker, owner, hardware, decoder).map_err(|error| {
            if compressed {
                NativeFailure::Retry(error)
            } else {
                NativeFailure::Fallback(error)
            }
        })
    }
    fn drive_native(
        &self,
        worker: &WorkerContext<(), ()>,
        owner: uuid::Uuid,
        hardware: &Hardware,
        decoder: Decoder,
    ) -> Result<(), Failure> {
        let compressed = decoder.storage() == DecodeStorage::Hap;
        let mut bytes = 16 * 1024 * 1024;
        if compressed {
            let info = decoder.info();
            let extent = golden_video::Extent2D::new(info.width, info.height).map_err(|e| e.to_string())?;
            // Current/next and the one GPU upload lease; HAP's largest admitted block format.
            bytes = (HapFormat::Bc3Rgba.byte_len(extent).ok_or("HAP storage overflow")? as u64)
                .checked_mul(3)
                .and_then(|frames| frames.checked_add(bytes))
                .ok_or("HAP storage overflow")?;
        }
        let _cpu_lease = self
            .budget
            .reserve_bytes(bytes)
            .map_err(|error| Failure(VideoSourceError::new(VideoSourceErrorKind::Capacity, error)))?;
        let mut reader = Reader::new(decoder)?;
        self.control
            .channel_duration(self.reference.channel, owner, reader.duration)?;
        let mut publisher: Option<GpuFramePublisher> = None;
        let mut pending = None;
        let mut installed = false;
        while !worker.cancellation().is_cancelled() {
            let turn_started = Instant::now();
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
                                "[mapgyver-video] {} playback: {} on {:?}",
                                if compressed {
                                    "HAP compressed GPU"
                                } else {
                                    "D3D11 resident"
                                },
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
                    let frame = reader.frame();
                    if publisher.is_none() {
                        publisher = Some(if let Some(frame) = frame.hap_frame() {
                            GpuFramePublisher::new_hap(
                                frame,
                                hardware.adapter,
                                &hardware.budget,
                                Some(worker.wake_notifier()),
                            )?
                        } else {
                            GpuFramePublisher::new(
                                frame.d3d11_frame().ok_or("native decoder returned software storage")?,
                                &hardware.budget,
                                Some(worker.wake_notifier()),
                            )?
                        });
                    }
                    let submitted = if let Some(frame) = frame.hap_frame() {
                        publisher.as_mut().unwrap().submit_hap(frame)?
                    } else {
                        publisher
                            .as_mut()
                            .unwrap()
                            .submit(frame.d3d11_frame().ok_or("native decoder returned software storage")?)?
                    };
                    if submitted {
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
            worker.wait_until(
                checkpoint,
                pacing::deadline(turn_started, sample.playing, waiting || reader.catching_up),
            );
        }
        Ok(())
    }
}
