//! Render-worker telemetry, separate from the engine and its authored document state.
use golden_time::TimeSnapshot;
use std::{fmt::Write, time::Duration};
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Default)]
pub(crate) struct FrameText {
    pub local: [u16; 4],
    pub fps: Option<f64>,
}
#[derive(Clone, Copy, Default)]
pub(crate) struct FrameSampler {
    start: Option<Duration>,
    previous: Option<Duration>,
    frames: u32,
    fps: Option<f64>,
}
impl FrameSampler {
    pub fn begin(&mut self, time: TimeSnapshot) -> FrameText {
        let now = Duration::from_nanos(time.monotonic().as_nanos());
        self.observe(now);
        // SAFETY: GetLocalTime returns a value, owns no handles and runs only on this renderer.
        let local = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        FrameText {
            local: [local.wHour, local.wMinute, local.wSecond, local.wMilliseconds],
            fps: self.fps,
        }
    }
    fn observe(&mut self, now: Duration) {
        if self
            .previous
            .is_none_or(|old| now < old || now - old > Duration::from_secs(2))
        {
            *self = Self {
                start: Some(now),
                ..Self::default()
            };
        }
    }
    pub fn submitted(&mut self, time: TimeSnapshot) {
        self.record(Duration::from_nanos(time.monotonic().as_nanos()));
    }
    pub(super) fn record(&mut self, now: Duration) {
        self.previous = Some(now);
        let start = *self.start.get_or_insert(now);
        let elapsed = now.saturating_sub(start);
        if elapsed >= Duration::from_secs(1) {
            self.fps = Some(f64::from(self.frames) / elapsed.as_secs_f64());
            self.start = Some(now);
            self.frames = 0;
        }
        self.frames = self.frames.saturating_add(1);
    }
}
pub(super) struct AsciiLine {
    pub bytes: [u8; 64],
    pub len: usize,
}
impl AsciiLine {
    pub fn new() -> Self {
        Self { bytes: [0; 64], len: 0 }
    }
    pub fn clock(frame: FrameText) -> Self {
        let mut line = Self::new();
        let [hour, minute, second, millis] = frame.local;
        write!(line, "{hour:02}:{minute:02}:{second:02}.{millis:03}").expect("bounded local clock");
        line
    }
    pub fn fps(frame: FrameText) -> Self {
        let mut line = Self::new();
        match frame.fps.filter(|value| value.is_finite() && *value >= 0.) {
            Some(value) => write!(line, "{:.1} FPS", value.min(99999.)).expect("bounded measured FPS"),
            None => line.write_str("--.- FPS").unwrap(),
        }
        line
    }
}
impl Write for AsciiLine {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self
            .len
            .checked_add(text.len())
            .filter(|end| *end <= self.bytes.len())
            .ok_or(std::fmt::Error)?;
        self.bytes[self.len..end].copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}
