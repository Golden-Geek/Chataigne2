//! Timestamp sampling and bounded decode turns on an owning file worker.
use super::failure::Failure;
use golden_video::{
    decode::{DecodedFrame, Decoder, SoftwareImageConverter, TimeBase},
    image::ImageData,
};
use std::time::Duration;

pub(super) struct Reader {
    decoder: Decoder,
    converter: SoftwareImageConverter,
    prepared: Option<(i64, ImageData)>,
    current: Option<DecodedFrame>,
    next: Option<DecodedFrame>,
    previous: Option<i64>,
    probed_previous: Option<PreviousProbe>,
    eof: bool,
    origin: i64,
    requested: Option<i64>,
    published: Option<i64>,
    pub catching_up: bool,
    pub duration: Duration,
}
struct PreviousProbe {
    target: i64,
    before: i64,
    distance: i64,
}

#[derive(serde::Serialize)]
pub(super) struct CpuLayouts {
    decoder_pool: Option<golden_video::image::ImageStorageAllocation>,
    conversion_storage: golden_video::image::ImageStorageAllocation,
    current: Option<golden_video::decode::SoftwareFrameLayout>,
    lookahead: Option<golden_video::decode::SoftwareFrameLayout>,
    scratch: Option<golden_video::decode::SoftwareFrameLayout>,
    prepared_image_bytes: usize,
    current_hap_bytes: Option<usize>,
    lookahead_hap_bytes: Option<usize>,
}
pub(super) fn duration(ticks: i64, base: TimeBase) -> Result<Duration, String> {
    let nanos = i128::from(ticks) * i128::from(base.numerator) * 1_000_000_000 / i128::from(base.denominator);
    let nanos = u64::try_from(nanos).map_err(|_| "Video timestamp exceeds the admitted timeline")?;
    if nanos > i64::MAX as u64 {
        return Err("Video timeline exceeds signed nanoseconds".into());
    }
    Ok(Duration::from_nanos(nanos))
}
impl Reader {
    pub fn cpu_layouts(&self) -> CpuLayouts {
        CpuLayouts {
            decoder_pool: self.decoder.software_frame_budget().map(|budget| budget.snapshot()),
            conversion_storage: self.converter.storage_budget().snapshot(),
            current: self.current.as_ref().and_then(DecodedFrame::software_layout),
            lookahead: self.next.as_ref().and_then(DecodedFrame::software_layout),
            scratch: self.converter.scratch_layout(),
            prepared_image_bytes: self.prepared.as_ref().map_or(0, |(_, image)| image.pixels().len()),
            current_hap_bytes: self
                .current
                .as_ref()
                .and_then(|frame| frame.hap_frame())
                .map(|frame| frame.blocks().len()),
            lookahead_hap_bytes: self
                .next
                .as_ref()
                .and_then(|frame| frame.hap_frame())
                .map(|frame| frame.blocks().len()),
        }
    }
    #[cfg(all(windows, feature = "native-video"))]
    pub(super) fn frame_is_published(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|frame| self.published == Some(frame.timestamp()))
    }
    pub fn invalidate_publication(&mut self) {
        self.published = None;
    }
    pub fn new(decoder: Decoder) -> Result<Self, Failure> {
        let info = decoder.info();
        let duration = duration(
            info.duration
                .filter(|duration| *duration > 0)
                .ok_or("Video stream duration is unavailable")?,
            info.time_base,
        )?;
        let origin = info.start.unwrap_or(0);
        let conversion_bytes = u64::from(info.width)
            .checked_mul(u64::from(info.height))
            .and_then(|pixels| pixels.checked_mul(16))
            .and_then(|bytes| bytes.checked_add(8 * 1024 * 1024))
            .ok_or("Video conversion storage overflow")?;
        Ok(Self {
            decoder,
            converter: SoftwareImageConverter::new(conversion_bytes)?,
            prepared: None,
            current: None,
            next: None,
            previous: None,
            probed_previous: None,
            eof: false,
            origin,
            requested: None,
            published: None,
            catching_up: true,
            duration,
        })
    }
    /// At most eight frames per turn, then return to cancellation/control inspection.
    /// Samples the last presentation timestamp at/before the shared position; EOF holds it.
    pub fn poll(&mut self, position: Duration) -> Result<Option<ImageData>, Failure> {
        if !self.advance(position)? {
            return Ok(None);
        }
        let image = self.software_image()?;
        self.mark_published();
        Ok(Some(image))
    }
    #[cfg(all(windows, feature = "native-video"))]
    pub fn frame(&self) -> &DecodedFrame {
        self.current.as_ref().expect("advance selected a frame")
    }
    pub fn software_image(&mut self) -> Result<ImageData, Failure> {
        let frame = self.current.as_ref().expect("advance selected a frame");
        if let Some((timestamp, image)) = self.prepared.take()
            && timestamp == frame.timestamp()
        {
            return Ok(image);
        }
        Ok(self.converter.convert(frame)?)
    }
    /// Convert the already-decoded lookahead while the selected frame uploads on the GPU.
    /// One prepared image replaces the next; no decoded frame or image queue is added.
    #[cfg(all(windows, feature = "native-video"))]
    pub fn prepare_next(&mut self, position: Duration) -> Result<(), Failure> {
        let Some(frame) = &self.next else {
            self.prepared = None;
            return Ok(());
        };
        let timestamp = frame.timestamp();
        if self.position(timestamp)? <= position {
            // Sampling is already due again. Let advance select the current timestamp rather
            // than spending a second conversion on lookahead that may already be obsolete.
            return Ok(());
        }
        if self.prepared.as_ref().is_none_or(|(stamp, _)| *stamp != timestamp) {
            self.prepared = Some((timestamp, self.converter.convert(frame)?));
        }
        Ok(())
    }
    pub fn mark_published(&mut self) {
        self.published = self.current.as_ref().map(DecodedFrame::timestamp);
    }
    pub fn advance(&mut self, position: Duration) -> Result<bool, Failure> {
        self.catching_up = true;
        let base = self.decoder.info().time_base;
        let ticks = position.min(self.duration).as_nanos() * u128::from(base.denominator)
            / (u128::from(base.numerator) * 1_000_000_000);
        let target = i64::try_from(i128::from(self.origin) + i128::try_from(ticks).map_err(|_| "Video seek overflow")?)
            .map_err(|_| "Video seek overflow")?;
        let jump = self.requested.is_none_or(|old| {
            i128::from(target) - i128::from(old) > i128::from(base.denominator) / i128::from(base.numerator) / 2
        });
        if self.current.as_ref().is_some_and(|frame| frame.timestamp() > target) || (jump && target > self.origin) {
            self.decoder.seek_before(target)?;
            self.current = None;
            self.next = None;
            self.previous = None;
            self.probed_previous = None;
            self.prepared = None;
            self.eof = false;
            self.published = None;
        }
        self.requested = Some(target);
        let mut decoded = 0;
        while decoded < 8 {
            if self.current.is_none() {
                self.current = self.decoder.next_frame()?;
                decoded += 1;
                if self.current.is_none() {
                    return Err("Video contains no timestamped frame".into());
                }
            }
            if self.next.is_none() && !self.eof {
                if decoded == 8 {
                    return Ok(false);
                }
                self.next = self.decoder.next_frame()?;
                decoded += 1;
                self.eof = self.next.is_none();
            }
            if self.next.as_ref().is_some_and(|frame| frame.timestamp() <= target) {
                self.previous = self.current.as_ref().map(DecodedFrame::timestamp);
                self.current = self.next.take();
                continue;
            }
            let frame = self.current.as_ref().unwrap();
            self.catching_up = false;
            if self.published == Some(frame.timestamp()) {
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
    }
    fn position(&self, timestamp: i64) -> Result<Duration, String> {
        let ticks = u128::try_from(i128::from(timestamp) - i128::from(self.origin))
            .map_err(|_| "Video timestamp precedes its origin")?;
        let base = self.decoder.info().time_base;
        let nanos = (ticks * u128::from(base.numerator) * 1_000_000_000).div_ceil(u128::from(base.denominator));
        Ok(Duration::from_nanos(
            u64::try_from(nanos).map_err(|_| "Video timestamp overflow")?,
        ))
    }
    pub fn neighbours(&self) -> Result<Option<super::frames::Neighbours>, String> {
        if self.catching_up {
            return Ok(None);
        }
        let Some(frame) = &self.current else {
            return Ok(None);
        };
        let timestamp = frame.timestamp();
        Ok(Some(super::frames::Neighbours {
            position: self.position(timestamp)?,
            previous: self.previous.map(|timestamp| self.position(timestamp)).transpose()?,
            previous_ready: self.previous.is_some()
                || timestamp <= self.origin
                || self
                    .probed_previous
                    .as_ref()
                    .is_some_and(|probe| probe.target == timestamp && probe.before <= self.origin),
            next: self
                .next
                .as_ref()
                .map(|frame| self.position(frame.timestamp()))
                .transpose()?,
        }))
    }
    /// A keyframe seek may not reveal its predecessor. Publish the requested image first,
    /// then resolve the exact predecessor in ordinary bounded decode turns while paused.
    pub fn prepare_previous(&mut self) -> Result<(), String> {
        if self.catching_up || self.previous.is_some() {
            return Ok(());
        }
        let Some(frame) = &self.current else {
            return Ok(());
        };
        let timestamp = frame.timestamp();
        if timestamp <= self.origin {
            return Ok(());
        }
        let prior = self.probed_previous.as_ref().filter(|probe| probe.target == timestamp);
        if prior.is_some_and(|probe| probe.before <= self.origin) {
            return Ok(());
        }
        let base = self.decoder.info().time_base;
        // Seek indexes may use decode timestamps before a keyframe's presentation timestamp.
        // Search earlier in time, then determine the neighbour from actual decoded timestamps.
        let distance = prior.map_or(
            (i64::from(base.denominator) / i64::from(base.numerator) / 4).max(1),
            |probe| probe.distance.saturating_mul(2),
        );
        let before = timestamp.saturating_sub(distance).max(self.origin);
        self.decoder.seek_before(before).map_err(|e| e.to_string())?;
        self.current = None;
        self.next = None;
        self.prepared = None;
        self.eof = false;
        self.probed_previous = Some(PreviousProbe {
            target: timestamp,
            before,
            distance,
        });
        self.catching_up = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
