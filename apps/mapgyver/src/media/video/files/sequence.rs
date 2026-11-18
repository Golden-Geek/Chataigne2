//! Independently seekable clips on one media timeline and a stable intro-sized canvas.
use super::{DecodeLimits, Decoder, Failure, ImageStream, Lease, MAX_BYTES, Reader, Run, Stream, WorkerContext};
use crate::media::video::{Neighbours, TransportSample, frames};
use golden_video::{Extent2D, image::ImageData, scheduling::FrameWindow};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Source {
    pub repeat: Option<PathBuf>,
}

struct Clip {
    reader: Reader,
    lease: Arc<Lease>,
    canvas_lease: Option<Arc<Lease>>,
}

impl Clip {
    fn open(
        path: &Path,
        run: &Run,
        worker: &WorkerContext<(), ()>,
        extent: Option<Extent2D>,
    ) -> Result<(Self, Extent2D), Failure> {
        let cancel = worker.cancellation().clone();
        let mut decoder = Decoder::open_software_with_interrupt(
            path,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_cpu_frame_bytes: MAX_BYTES,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: golden_video::decode::SoftwareDecodeThreads::Four,
            },
            move || cancel.is_cancelled(),
        )?;
        let lease = Arc::new(run.budget.admit_software(&mut decoder)?);
        let input = Extent2D::new(decoder.info().width, decoder.info().height).map_err(|err| err.to_string())?;
        let extent = extent.unwrap_or(input);
        let mut reader = Reader::new(decoder)?;
        let canvas_lease = if extent != input {
            let bytes = u64::from(extent.width()) * u64::from(extent.height()) * 16 + 8 * 1024 * 1024;
            let reservation = run.budget.reserve_bytes(bytes).map_err(|err| {
                Failure(crate::media::video::protocol::VideoSourceError::new(
                    crate::media::video::protocol::VideoSourceErrorKind::Capacity,
                    err,
                ))
            })?;
            reader = reader.with_output_extent(extent, bytes)?;
            Some(Arc::new(reservation))
        } else {
            None
        };
        Ok((
            Self {
                reader,
                lease,
                canvas_lease,
            },
            extent,
        ))
    }

    fn image(&mut self) -> Result<ImageData, Failure> {
        Ok(self
            .reader
            .software_image()?
            .retain(self.canvas_lease.as_ref().unwrap_or(&self.lease).clone()))
    }
}

#[derive(Default)]
struct Repeat {
    main: Option<Clip>,
    incoming: Option<Clip>,
    fingerprint: Option<(u64, SystemTime)>,
    retry: Option<Instant>,
    last_error: Option<String>,
    fade_retry: Option<Instant>,
    warmed: bool,
}

impl Repeat {
    fn reconcile(&mut self, path: &Path, run: &Run, worker: &WorkerContext<(), ()>, extent: Extent2D) -> bool {
        let now = Instant::now();
        if self.retry.is_some_and(|retry| retry > now) {
            return false;
        }
        self.retry = Some(now + Duration::from_secs(1));
        let next = fs::metadata(path).and_then(|info| Ok((info.len(), info.modified()?)));
        if self.main.is_some() && next.as_ref().ok() == self.fingerprint.as_ref() {
            return false;
        }
        let was_ready = self.main.take().is_some();
        self.incoming = None;
        self.fade_retry = None;
        self.warmed = false;
        match next.map_err(|error| error.to_string()).and_then(|stamp| {
            let (clip, _) = Clip::open(path, run, worker, Some(extent)).map_err(|failure| failure.0.message)?;
            Ok((clip, stamp))
        }) {
            Ok((clip, stamp)) => {
                self.main = Some(clip);
                self.fingerprint = Some(stamp);
                self.last_error = None;
                true
            }
            Err(error) => {
                if self.last_error.as_ref() != Some(&error) {
                    eprintln!("[mapgyver-video] Loop File {}: {error}", path.display());
                    self.last_error = Some(error);
                }
                was_ready
            }
        }
    }

    fn incoming(
        &mut self,
        path: &Path,
        run: &Run,
        worker: &WorkerContext<(), ()>,
        extent: Extent2D,
    ) -> Option<&mut Clip> {
        if self.incoming.is_none() && self.fade_retry.is_none_or(|retry| retry <= Instant::now()) {
            match Clip::open(path, run, worker, Some(extent)) {
                Ok((clip, _)) => {
                    self.fade_retry = None;
                    self.incoming = Some(clip);
                }
                Err(error) => {
                    if self.fade_retry.is_none() {
                        eprintln!(
                            "[mapgyver-video] Loop File overlap for {}: {}",
                            path.display(),
                            error.0.message
                        );
                    }
                    self.fade_retry = Some(Instant::now() + Duration::from_secs(1));
                }
            }
        }
        self.incoming.as_mut()
    }
}

impl Run {
    pub(super) fn play_sequence(
        &self,
        worker: &WorkerContext<(), ()>,
        owner: uuid::Uuid,
        source: &Source,
    ) -> Result<(), Failure> {
        let (mut intro, extent) = Clip::open(&self.file, self, worker, None)?;
        self.control
            .channel_duration(self.reference.channel, owner, intro.reader.duration)?;
        let mut repeat = Repeat::default();
        let mut stream: Option<ImageStream> = None;
        let mut window: Option<(FrameWindow, i64, bool)> = None;
        #[cfg(all(windows, feature = "native-video"))]
        let mut subtitles = super::subtitles::Overlay::default();
        while !worker.cancellation().is_cancelled() {
            let turn = Instant::now();
            let checkpoint = worker.wake_checkpoint();
            let revisions = self.control.revisions();
            let _subscription = revisions
                .guard_at(revisions.current())
                .subscribe(worker.wake_notifier());
            #[cfg(all(windows, feature = "native-video"))]
            let _subtitles = self.subtitle_channel.subscribe(worker.wake_notifier());
            let (sample, boundary) = self.control.sequence_sample()?;
            let absolute = sample.channel_position(Duration::MAX);
            let in_repeat = boundary.is_some_and(|end| absolute >= end);
            #[cfg(all(windows, feature = "native-video"))]
            subtitles.service_sequence(self, worker, &intro.reader, sample, absolute, in_repeat);
            let mut pending = false;
            if let Some(path) = source.repeat.as_deref().filter(|_| boundary.is_some()) {
                if repeat.reconcile(path, self, worker, extent) {
                    window = None;
                }
                if !in_repeat
                    && !repeat.warmed
                    && let Some(clip) = &mut repeat.main
                {
                    clip.reader.advance(Duration::ZERO)?;
                    repeat.warmed = !clip.reader.catching_up;
                    pending = !repeat.warmed;
                }
            } else if repeat.retry.is_some() {
                repeat = Repeat::default();
            }
            if sample.loop_blend().is_some() && repeat.fade_retry.is_some_and(|retry| retry <= turn) {
                window = None;
            }
            if sample.source_range.is_none() {
                if window.take().is_some()
                    && let Some(stream) = &stream
                {
                    stream.publish(None)?;
                }
            } else if !window.is_some_and(|(window, cycle, playing)| {
                cycle == sample.cycle
                    && playing == sample.playing
                    && sample.frame_position().is_ok_and(|pos| window.contains(pos))
            }) {
                let next = self.prepare_sequence_frame(
                    worker,
                    source,
                    &mut intro,
                    &mut repeat,
                    extent,
                    sample,
                    boundary,
                    absolute,
                )?;
                pending |= next.is_none();
                if let Some((image, neighbours)) = next {
                    let stream = stream.get_or_insert_with(|| ImageStream::new(image.descriptor()));
                    if self
                        .control
                        .publish_frame(self.reference.channel, owner, sample, stream, image, neighbours)?
                    {
                        self.control
                            .observe_neighbours(self.reference.channel, owner, sample, neighbours)?;
                        window = Some((frames::loop_window(sample, neighbours)?, sample.cycle, sample.playing));
                        let installed = {
                            let mut output = self.output.lock().unwrap();
                            let installed = output.stream.is_none();
                            if installed {
                                output.stream = Some(Stream::Software(stream.clone()));
                                output.error = None;
                            }
                            installed
                        };
                        if installed {
                            self.changed();
                        }
                        if !sample.playing {
                            let reader = if in_repeat && let Some(clip) = &mut repeat.main {
                                &mut clip.reader
                            } else {
                                &mut intro.reader
                            };
                            reader.prepare_previous()?;
                            if reader.catching_up {
                                window = None;
                                pending = true;
                            }
                        }
                    }
                }
            }
            let deadline = super::pacing::deadline(turn, sample.playing, pending);
            let deadline = repeat.retry.map_or(deadline, |retry| retry.min(deadline));
            #[cfg(all(windows, feature = "native-video"))]
            let deadline = subtitles.deadline(deadline);
            worker.wait_until(checkpoint, deadline);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_sequence_frame(
        &self,
        worker: &WorkerContext<(), ()>,
        source: &Source,
        intro: &mut Clip,
        repeat: &mut Repeat,
        extent: Extent2D,
        sample: TransportSample,
        boundary: Option<Duration>,
        absolute: Duration,
    ) -> Result<Option<(ImageData, Neighbours)>, Failure> {
        let intro_last = boundary.map_or(absolute, |end| {
            absolute.min(end.saturating_sub(Duration::from_nanos(1)))
        });
        let clip_offset = boundary.filter(|end| absolute >= *end && repeat.main.is_some());
        let blend = sample.loop_blend().filter(|_| repeat.main.is_some());
        let clip = if clip_offset.is_some() {
            repeat.main.as_mut().unwrap()
        } else {
            &mut *intro
        };
        clip.reader
            .advance(clip_offset.map_or(intro_last, |start| absolute - start))?;
        let Some(mut neighbours) = clip.reader.neighbours()? else {
            return Ok(None);
        };
        let mut image = clip.image()?;
        if let Some(offset) = clip_offset {
            neighbours.position += offset;
            neighbours.previous = neighbours.previous.map(|time| offset + time);
            neighbours.next = neighbours.next.map(|time| offset + time);
            if neighbours.previous.is_none() && !sample.playing {
                intro.reader.advance(offset.saturating_sub(Duration::from_nanos(1)))?;
                if let Some(previous) = intro.reader.neighbours()? {
                    neighbours.previous = Some(previous.position);
                    neighbours.previous_ready = true;
                } else {
                    return Ok(None);
                }
            }
        } else if let Some(end) = boundary {
            neighbours.next = if absolute < end {
                Some(neighbours.next.map_or(end, |next| next.min(end)))
            } else {
                None
            };
        }
        if let Some(blend) = blend
            && blend.secondary_weight > 0.0
            && let Some(path) = &source.repeat
            && let Some(boundary) = boundary
            && let Some(incoming) = repeat.incoming(path, self, worker, extent)
        {
            let at = sample
                .source_position(blend.secondary_position, Duration::MAX)
                .saturating_sub(boundary);
            incoming.reader.advance(at)?;
            if incoming.reader.catching_up {
                return Ok(None);
            }
            image = super::loop_pair::crossfade(self, &image, &incoming.image()?, blend.secondary_weight)?;
        }
        Ok(Some((image, neighbours)))
    }
}
