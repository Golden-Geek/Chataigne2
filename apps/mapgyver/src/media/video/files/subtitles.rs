//! One source-clock subtitle stream per demanded video channel, shared by all render consumers.
use super::{ImageStream, Reader, Run, WorkerContext};
use crate::media::{
    assets,
    video::{
        TransportSample,
        protocol::{VideoSubtitleSettings, VideoSubtitleTrack},
    },
};
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor,
    decode::subtitles::{SubtitleTimeline, load_embedded, load_local},
    image::{SubtitleRasterizer, SubtitleStyle},
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

#[derive(PartialEq, Eq)]
enum Input {
    Embedded(usize),
    Local(PathBuf),
}

#[derive(Default)]
pub(super) struct Overlay {
    settings: Option<Arc<VideoSubtitleSettings>>,
    input: Option<Input>,
    timeline: Option<SubtitleTimeline>,
    fingerprint: Option<(u64, SystemTime)>,
    poll_at: Option<Instant>,
    render_at: Option<Instant>,
    font: Option<SubtitleRasterizer>,
    stream: Option<ImageStream>,
    rendered: Option<String>,
    cue: String,
    failure: Option<String>,
}

impl Overlay {
    pub(super) fn deadline(&self, video_deadline: Instant) -> Instant {
        let mut deadline = video_deadline;
        if (self.timeline.is_none() || matches!(self.input, Some(Input::Local(_))))
            && let Some(poll_at) = self.poll_at
        {
            deadline = deadline.min(poll_at);
        }
        if self.timeline.is_some()
            && let Some(render_at) = self.render_at
        {
            deadline = deadline.min(render_at);
        }
        deadline
    }

    pub(super) fn service(
        &mut self,
        run: &Run,
        worker: &WorkerContext<(), ()>,
        reader: &Reader,
        sample: TransportSample,
    ) {
        self.service_sequence(
            run,
            worker,
            reader,
            sample,
            sample.channel_position(reader.duration),
            false,
        );
    }

    pub(super) fn service_sequence(
        &mut self,
        run: &Run,
        worker: &WorkerContext<(), ()>,
        intro: &Reader,
        sample: TransportSample,
        position: Duration,
        in_repeat: bool,
    ) {
        let position = sample.source_range.map(|_| (position, in_repeat));
        let result = self.update(run, worker, intro, position);
        if let Err(error) = result {
            if self.failure.as_ref() != Some(&error) {
                eprintln!("[mapgyver-video] Subtitles for {}: {error}", run.file.display());
            }
            self.failure = Some(error);
            self.clear(run);
        }
    }

    fn update(
        &mut self,
        run: &Run,
        worker: &WorkerContext<(), ()>,
        reader: &Reader,
        position: Option<(Duration, bool)>,
    ) -> Result<(), String> {
        let settings = run.subtitle_channel.settings()?;
        if self.settings.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &settings)) {
            let input = if settings.enabled {
                match &settings.track {
                    VideoSubtitleTrack::None => None,
                    VideoSubtitleTrack::File => assets::parse_file(&settings.file)?
                        .map(|locator| Input::Local(locator.resolve(&run.project_root))),
                    VideoSubtitleTrack::Embedded { index } => Some(Input::Embedded(*index as usize)),
                }
            } else {
                None
            };
            if self.input != input {
                self.input = input;
                self.timeline = None;
                self.fingerprint = None;
                self.poll_at = None;
                self.clear(run);
            }
            if self.settings.as_ref().is_none_or(|old| old.font != settings.font) {
                self.font = Some(SubtitleRasterizer::new(&settings.font));
            }
            self.settings = Some(settings.clone());
            self.rendered = None;
            self.render_at = None;
        }
        let Some(input) = &self.input else { return Ok(()) };
        let now = Instant::now();
        if self.poll_at.is_none_or(|deadline| deadline <= now) {
            self.poll_at = Some(now + Duration::from_secs(1));
            if let Input::Local(path) = input {
                let fingerprint = std::fs::metadata(path).and_then(|metadata| {
                    if !metadata.is_file() {
                        return Err(std::io::Error::other("subtitle source is not a regular file"));
                    }
                    Ok((metadata.len(), metadata.modified()?))
                });
                if fingerprint.as_ref().ok() != self.fingerprint.as_ref() || fingerprint.is_err() {
                    self.timeline = None;
                }
                self.fingerprint = Some(fingerprint.map_err(|error| error.to_string())?);
            }
            if self.timeline.is_none() {
                let cancel = worker.cancellation().clone();
                let info = reader.info();
                let timeline = match input {
                    Input::Embedded(index) => {
                        load_embedded(&run.file, *index, info.start.unwrap_or(0), info.time_base, move || {
                            cancel.is_cancelled()
                        })
                    }
                    Input::Local(path) => load_local(path, move || cancel.is_cancelled()),
                }
                .map_err(|error| error.to_string())?;
                self.timeline = Some(timeline);
                self.failure = None;
            }
        }
        let Some(timeline) = &self.timeline else { return Ok(()) };
        if self.render_at.is_some_and(|deadline| now < deadline) {
            return Ok(());
        }
        self.cue.clear();
        if let Some((position, in_repeat)) = position
            && (!in_repeat || matches!(input, Input::Local(_)))
        {
            for cue in timeline.active_at(position).take(4) {
                if !self.cue.is_empty() && self.cue.len() < 4096 {
                    self.cue.push('\n');
                }
                let mut available = (4096 - self.cue.len()).min(cue.text.len());
                while !cue.text.is_char_boundary(available) {
                    available -= 1;
                }
                self.cue.push_str(&cue.text[..available]);
            }
        }
        if self.rendered.as_deref() == Some(self.cue.as_str()) {
            return Ok(());
        }
        // Keep a budget or raster failure from turning into a per-worker burst of retries.
        self.render_at = Some(now + Duration::from_millis(250));
        self.publish(run, reader, &settings)?;
        self.render_at = None;
        self.rendered = Some(self.cue.clone());
        Ok(())
    }

    fn publish(&mut self, run: &Run, reader: &Reader, settings: &VideoSubtitleSettings) -> Result<(), String> {
        let info = reader.info();
        let scale = (960.0 / f64::from(info.width))
            .min(540.0 / f64::from(info.height))
            .min(1.0);
        let extent = Extent2D::new(
            (f64::from(info.width) * scale).round().max(1.0) as u32,
            (f64::from(info.height) * scale).round().max(1.0) as u32,
        )
        .map_err(|error| error.to_string())?;
        let descriptor = TextureDescriptor {
            extent,
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        };
        let image = if self.cue.is_empty() {
            None
        } else {
            let bytes = descriptor.byte_len().map_err(|error| error.to_string())?;
            // The GDI bitmap and immutable RGBA can coexist during rasterization.
            let lease = Arc::new(run.budget.reserve_bytes(bytes * 2)?);
            let style = SubtitleStyle {
                size: (settings.size / 100.0) as f32,
                position: settings.position.map(|value| value as f32),
            };
            self.font
                .as_ref()
                .expect("accepted subtitle settings have a font")
                .rasterize(&self.cue, extent, style)
                .map_err(|error| error.to_string())?
                .map(|image| image.retain(lease))
        };
        let stream = self.stream.get_or_insert_with(|| ImageStream::new(descriptor));
        stream.publish(image)?;
        let mut output = run.output.lock().map_err(|_| "Video output poisoned")?;
        if output.subtitles.is_none() {
            output.subtitles = Some(stream.clone());
            drop(output);
            run.changed();
        }
        Ok(())
    }

    fn clear(&mut self, run: &Run) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.publish(None);
            if let Ok(mut output) = run.output.lock() {
                output.subtitles = None;
            }
            run.changed();
        }
        self.rendered = None;
    }
}
