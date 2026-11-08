//! Timestamp sampling and bounded decode turns on an owning file worker.
use golden_video::{
    decode::{DecodedFrame, SoftwareDecoder, SoftwareImageConverter, TimeBase},
    image::ImageData,
};
use std::time::Duration;

pub(super) struct Reader {
    decoder: SoftwareDecoder,
    converter: SoftwareImageConverter,
    current: Option<DecodedFrame>,
    next: Option<DecodedFrame>,
    eof: bool,
    origin: i64,
    requested: Option<i64>,
    published: Option<i64>,
    pub catching_up: bool,
    pub duration: Duration,
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
    pub fn invalidate_publication(&mut self) {
        self.published = None;
    }
    pub fn new(decoder: SoftwareDecoder) -> Result<Self, String> {
        let info = decoder.info();
        let duration = duration(
            info.duration
                .filter(|duration| *duration > 0)
                .ok_or("Video stream duration is unavailable")?,
            info.time_base,
        )?;
        let origin = info.start.unwrap_or(0);
        Ok(Self {
            decoder,
            converter: Default::default(),
            current: None,
            next: None,
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
    pub fn poll(&mut self, position: Duration) -> Result<Option<ImageData>, String> {
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
            self.decoder.seek_before(target).map_err(|e| e.to_string())?;
            self.current = None;
            self.next = None;
            self.eof = false;
            self.published = None;
        }
        self.requested = Some(target);
        for _ in 0..8 {
            if self.current.is_none() {
                self.current = self.decoder.next_frame().map_err(|e| e.to_string())?;
                if self.current.is_none() {
                    return Err("Video contains no timestamped frame".into());
                }
            }
            if self.next.is_none() && !self.eof {
                self.next = self.decoder.next_frame().map_err(|e| e.to_string())?;
                self.eof = self.next.is_none();
            }
            if self.next.as_ref().is_some_and(|frame| frame.timestamp() <= target) {
                self.current = self.next.take();
                continue;
            }
            let frame = self.current.as_ref().unwrap();
            self.catching_up = false;
            if self.published == Some(frame.timestamp()) {
                return Ok(None);
            }
            let image = self.converter.convert(frame)?;
            self.published = Some(frame.timestamp());
            return Ok(Some(image));
        }
        Ok(None)
    }
}
