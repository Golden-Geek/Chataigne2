//! Exact-adapter file decode and asynchronous, clock-fenced GPU publication.
use super::super::control::scheduling;
use super::*;
use golden_video::decode::{DecodeStorage, HapFormat};
use golden_video::native::{GpuFrameError, GpuFrameInput, GpuFramePublisher, GpuFrameSampling};
use loop_pair::LoopPair;

#[derive(Clone, Copy)]
struct PreparedFrame {
    sample: super::super::TransportSample,
    frame: super::super::frames::Neighbours,
    timestamp: i64,
}

#[derive(Debug)]
pub(super) enum NativeFailure {
    Fallback(Failure),
    Retry(Failure),
}

impl NativeFailure {
    fn opening(error: golden_video::decode::DecodeError) -> Self {
        match error {
            golden_video::decode::DecodeError::UnsupportedCodec
            | golden_video::decode::DecodeError::UnsupportedHardware => Self::Fallback(error.into()),
            _ => Self::Retry(error.into()),
        }
    }

    fn playback(error: Failure, compressed: bool) -> Self {
        // A supported decoder must be recreated after a runtime failure. Falling back here
        // would keep the worker on a different decode/color path after a transient reset.
        if !compressed && error.0.kind == VideoSourceErrorKind::Unsupported {
            Self::Fallback(error)
        } else {
            Self::Retry(error)
        }
    }
}

impl From<GpuFrameError> for Failure {
    fn from(error: GpuFrameError) -> Self {
        let kind = match &error {
            GpuFrameError::Unsupported => VideoSourceErrorKind::Unsupported,
            GpuFrameError::Capacity => VideoSourceErrorKind::Capacity,
            GpuFrameError::Layout | GpuFrameError::Decode(_) | GpuFrameError::Native(_) => VideoSourceErrorKind::Decode,
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
                max_cpu_frame_bytes: MAX_BYTES,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: golden_video::decode::SoftwareDecodeThreads::One,
            },
            hardware.adapter,
            hardware.budget.clone(),
            move || cancel.is_cancelled(),
        )
        .map_err(NativeFailure::opening)?;
        let compressed = decoder.storage() == DecodeStorage::Hap;
        self.drive_native(worker, owner, hardware, decoder, None)
            .map_err(|error| NativeFailure::playback(error, compressed))
    }
    pub(super) fn drive_native(
        &self,
        worker: &WorkerContext<(), ()>,
        owner: uuid::Uuid,
        hardware: &Hardware,
        mut decoder: Decoder,
        cpu_lease: Option<Arc<Lease>>,
    ) -> Result<(), Failure> {
        let compressed = decoder.storage() == DecodeStorage::Hap;
        let software = decoder.storage() == DecodeStorage::Software;
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
        let mut cpu_lease = match cpu_lease {
            Some(lease) => lease,
            None => Arc::new(
                self.budget
                    .reserve_bytes(bytes)
                    .map_err(|error| Failure(VideoSourceError::new(VideoSourceErrorKind::Capacity, error)))?,
            ),
        };
        if compressed {
            // The source charge already includes three worst-case compressed blocks and
            // a separate packet allowance. Held upload clones share each block's charge.
            decoder.tighten_cpu_frame_limit(cpu_lease.bytes - 16 * 1024 * 1024)?;
        }
        let mut reader = Reader::new(decoder)?;
        self.control
            .channel_duration(self.reference.channel, owner, reader.duration)?;
        let mut publisher: Option<GpuFramePublisher> = None;
        let mut pending = None;
        let mut ready = None;
        let mut cached = None;
        let mut retry = None;
        let mut cursor = None;
        let mut loop_cursor = None;
        let mut loop_pair: Option<LoopPair> = None;
        let mut epoch = None;
        let mut software_image: Option<(i64, Arc<golden_video::image::ImageData>)> = None;
        let mut installed = false;
        let mut subtitles = subtitles::Overlay::default();
        while !worker.cancellation().is_cancelled() {
            let turn_started = Instant::now();
            let checkpoint = worker.wake_checkpoint();
            let revision = self.control.revisions();
            let _subscription = revision.guard_at(revision.current()).subscribe(worker.wake_notifier());
            let _subtitles = self.subtitle_channel.subscribe(worker.wake_notifier());
            let sample = self.control.sample()?;
            subtitles.service(self, worker, &reader, sample);
            if epoch != Some(sample.epoch) {
                epoch = Some(sample.epoch);
                cursor = None;
                loop_cursor = None;
                retry = None;
                if let Some(publisher) = &publisher {
                    // Old windows plus a completed rejected image can otherwise pin all three
                    // slots and prevent the new epoch from ever producing its replacement.
                    drop(publisher.stream().begin_epoch(sample.epoch)?);
                    if sample.source_range.is_none() {
                        publisher.stream().publish(None)?;
                        reader.invalidate_publication();
                    }
                }
            }
            let mut waiting = false;
            if !sample.needs_loop_companion() {
                loop_pair = None;
            }
            if sample.has_loop_overlap() {
                // Fade publications never use timestamp-only reuse. Let the former ordinary
                // image release its ring slot once its consumers and clock window retire.
                cached = None;
            }
            if let Some(publisher) = &mut publisher {
                if let Some(image) = publisher.poll()? {
                    let expected = pending.take().expect("one clock fence per submitted conversion");
                    let expected: PreparedFrame = expected;
                    if !expected.sample.has_loop_overlap() {
                        cached = Some((expected.timestamp, image.clone()));
                    }
                    ready = Some((expected, image));
                }
                if let Some((expected, image)) = &ready {
                    let expected: &PreparedFrame = expected;
                    let stream = publisher.stream();
                    if self.control.publish_gpu_scheduled(
                        self.reference.channel,
                        owner,
                        expected.sample,
                        &stream,
                        image.clone(),
                        expected.frame,
                    )? {
                        if !installed {
                            let mut output = self.output.lock().unwrap();
                            output.stream = Some(Stream::Hardware(stream));
                            output.error = None;
                            drop(output);
                            installed = true;
                            self.report_cpu_layouts(
                                &reader,
                                &cpu_lease,
                                software_image.as_ref().map(|(_, image)| image.pixels().len() as u64),
                            );
                            eprintln!(
                                "[mapgyver-video] {} playback: {} on {:?}",
                                if software {
                                    "software decode/source-worker GPU upload"
                                } else if compressed {
                                    "HAP compressed GPU"
                                } else {
                                    "D3D11 resident"
                                },
                                self.file.display(),
                                hardware.adapter
                            );
                            self.changed();
                        }
                        ready = None;
                    } else if self.control.sample()?.epoch != expected.sample.epoch {
                        ready = None;
                        cursor = None;
                        loop_cursor = None;
                    }
                }
                waiting = publisher.is_pending() || ready.is_some();
                if !waiting && !sample.needs_loop_companion() {
                    let _ = publisher.release_crossfade();
                }
            }
            if sample.source_range.is_none() {
                cached = None;
                software_image = None;
                worker.wait_until(
                    checkpoint,
                    subtitles.deadline(pacing::deadline(turn_started, false, waiting)),
                );
                continue;
            }
            if !waiting {
                let target = if let Some(target) = retry {
                    Some(target)
                } else if sample.timeline.is_some() {
                    scheduling::loop_target(sample, loop_cursor)?
                } else {
                    scheduling::target(
                        sample,
                        cursor,
                        reader.neighbours()?,
                        reader.frame_is_published(),
                        reader.duration,
                    )?
                };
                if let Some(target) = target {
                    let blend = if target.needs_loop_companion() {
                        let pair = match &mut loop_pair {
                            Some(pair) => pair,
                            slot @ None => slot.insert(LoopPair::open(self, worker, &reader, &cpu_lease)?),
                        };
                        pair.orient(target, &mut reader, &mut cpu_lease);
                        let (prepared, blend) = pair.prepare(target)?;
                        if !prepared && blend.is_some() {
                            worker.wait_until(checkpoint, pacing::deadline(turn_started, sample.playing, true));
                            continue;
                        }
                        blend
                    } else {
                        None
                    };
                    let position = target.channel_position(reader.duration);
                    let changed = reader.advance_for_playback(position, target.forward_catch_up_start())?;
                    let fade_changed = target.has_loop_overlap()
                        && !reader.catching_up
                        && (scheduling::timed(target) || cursor != Some(target));
                    if changed || fade_changed {
                        retry = Some(target);
                        let image = if software {
                            let timestamp = reader.frame().timestamp();
                            if software_image.as_ref().is_none_or(|(stamp, _)| *stamp != timestamp) {
                                software_image =
                                    Some((timestamp, Arc::new(reader.software_image()?.retain(cpu_lease.clone()))));
                            }
                            Some(software_image.as_ref().unwrap().1.clone())
                        } else {
                            None
                        };
                        let frame = reader.frame();
                        if publisher.is_none() {
                            publisher = Some(if let Some(image) = &image {
                                GpuFramePublisher::new_software(
                                    image.descriptor(),
                                    hardware.adapter,
                                    &hardware.budget,
                                    Some(worker.wake_notifier()),
                                )?
                            } else if let Some(frame) = frame.hap_frame() {
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
                            let sampling = if reader.seeks_from_start() {
                                GpuFrameSampling::ForwardHold
                            } else {
                                GpuFrameSampling::Due
                            };
                            publisher
                                .as_mut()
                                .unwrap()
                                .set_presentation_clock(self.control.clone(), sampling)?;
                            drop(publisher.as_ref().unwrap().stream().begin_epoch(target.epoch)?);
                            if hardware.submission_sink.is_some() {
                                self.report_source(publisher.as_ref().unwrap().stream().id());
                            }
                            if let Some(sink) = &hardware.submission_sink {
                                publisher.as_mut().unwrap().enable_diagnostics(sink)?;
                            } else if std::env::var_os("MAPGYVER_NATIVE_DIAGNOSTICS")
                                .is_some_and(|value| !value.is_empty())
                            {
                                publisher.as_mut().unwrap().record_software_upload_timing();
                            }
                        }
                        let submitted = if let Some(blend) = blend {
                            let publisher = publisher.as_mut().expect("loop publisher ready");
                            publisher.prepare_crossfade()?;
                            let pair = loop_pair.as_mut().expect("loop companion ready");
                            let (primary, secondary) = if let Some(image) = image {
                                (
                                    GpuFrameInput::Software(image),
                                    GpuFrameInput::Software(pair.software_image()?),
                                )
                            } else if let Some(frame) = frame.hap_frame() {
                                (
                                    GpuFrameInput::Hap(frame),
                                    GpuFrameInput::Hap(
                                        pair.reader()
                                            .frame()
                                            .hap_frame()
                                            .ok_or("loop companion HAP storage changed")?,
                                    ),
                                )
                            } else {
                                (
                                    GpuFrameInput::Decoded(
                                        frame.d3d11_frame().ok_or("native decoder returned software storage")?,
                                    ),
                                    GpuFrameInput::Decoded(
                                        pair.reader()
                                            .frame()
                                            .d3d11_frame()
                                            .ok_or("loop companion native storage changed")?,
                                    ),
                                )
                            };
                            publisher.submit_crossfade(primary, secondary, blend.secondary_weight)?
                        } else if let Some(image) = image {
                            publisher.as_mut().unwrap().submit_software(image)?
                        } else if let Some(frame) = frame.hap_frame() {
                            publisher.as_mut().unwrap().submit_hap(frame)?
                        } else {
                            publisher
                                .as_mut()
                                .unwrap()
                                .submit(frame.d3d11_frame().ok_or("native decoder returned software storage")?)?
                        };
                        if submitted {
                            if !target.has_loop_overlap() {
                                software_image = None;
                            }
                            let frame = reader
                                .selected_neighbours()?
                                .ok_or("Selected Video timestamps unavailable")?;
                            let window = super::super::frames::loop_window(target, frame).ok();
                            if let Some(sink) = &hardware.submission_sink {
                                // Submission is recorded before completion/publication, including
                                // frames later superseded or never sampled by a renderer.
                                // Unprojectable diagnostic timing stays unknown; it must not
                                // turn an otherwise valid paused conversion into a source fault.
                                publisher.as_mut().unwrap().trace_pending_submission(sink, window);
                            }
                            pending = Some(PreparedFrame {
                                sample: target,
                                frame,
                                timestamp: reader.frame().timestamp(),
                            });
                            retry = None;
                            cursor = Some(target);
                            loop_cursor = if target.timeline.is_some() { window } else { None };
                            reader.mark_published();
                            if software && target.playing && target.timeline.is_none() {
                                let now = self.control.sample()?;
                                if now.playing && now.epoch == target.epoch {
                                    reader.prepare_next(now.channel_position(reader.duration))?;
                                }
                            }
                        }
                        // Retry a full conversion ring even while paused. Slot release also wakes us.
                        waiting = true;
                    } else {
                        if let Some(frame) = reader.neighbours()? {
                            // A clock discontinuity or a one-image loop can reuse immutable pixels.
                            // The new window is still fenced under the current media epoch.
                            if cursor.is_none_or(|cursor: super::super::TransportSample| {
                                (cursor.epoch, cursor.cycle) != (target.epoch, target.cycle)
                                    || scheduling::timed(cursor) != scheduling::timed(target)
                            }) && let Some((timestamp, image)) = &cached
                                && *timestamp == reader.frame().timestamp()
                            {
                                ready = Some((
                                    PreparedFrame {
                                        sample: target,
                                        frame,
                                        timestamp: *timestamp,
                                    },
                                    image.clone(),
                                ));
                                cursor = Some(target);
                                if target.timeline.is_some() {
                                    loop_cursor = Some(super::super::frames::loop_window(target, frame)?);
                                }
                                waiting = true;
                            } else {
                                self.control.refresh_gpu_neighbours(
                                    self.reference.channel,
                                    owner,
                                    cursor.unwrap_or(sample),
                                    frame,
                                )?;
                            }
                        }
                        if !sample.playing {
                            reader.prepare_previous()?;
                        }
                    }
                }
            }
            worker.wait_until(
                checkpoint,
                subtitles.deadline(pacing::deadline(
                    turn_started,
                    sample.playing,
                    waiting || reader.catching_up,
                )),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
