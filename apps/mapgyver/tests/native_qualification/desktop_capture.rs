//! One native clock for pre-input acknowledgements and readback completion, never input injection.
use super::{config::Arguments, roles};
use golden_video::native::fixtures::{DesktopCapture, DesktopCaptureConfig, DesktopGeometry};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Read, Write};
use std::num::NonZeroU32;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Request {
    Stamp { id: NonZeroU32 },
    Stop,
}

#[derive(Serialize)]
#[serde(tag = "kind")]
enum Event {
    Ready {
        frequency: String,
        geometry: DesktopGeometry,
        region: [i32; 4],
    },
    Stamped {
        id: u32,
        qpc: String,
    },
    Frame {
        serial: u64,
        acquire_begin_qpc: String,
        readback_end_qpc: String,
        desktop_update_qpc: String,
        accumulated_frames: u32,
        bgra: Vec<u8>,
    },
    Stopped,
}

fn emit(writer: &mut impl Write, event: Event) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, &event)?;
    writer.write_all(b"\n")?;
    writer.flush()
}

fn read_request(reader: &mut impl BufRead) -> io::Result<Option<Request>> {
    let mut line = String::new();
    if (&mut *reader).take(257).read_line(&mut line)? == 0 {
        return Ok(None);
    }
    if line.len() > 256 {
        return Err(io::Error::other("oversized capture command"));
    }
    serde_json::from_str(&line).map(Some).map_err(io::Error::other)
}

pub(super) fn run(mut args: Arguments) -> io::Result<()> {
    let adapter = roles::selected(args.number("adapter", "0")?)?;
    let left: i32 = args.number("left", "0")?;
    let top: i32 = args.number("top", "0")?;
    let width: i32 = args.number("width", "128")?;
    let height: i32 = args.number("height", "8")?;
    let config = DesktopCaptureConfig {
        adapter,
        output: args.number("output", "0")?,
        hwnd: args.number("hwnd", "0")?,
        pid: args.number("pid", "0")?,
        region: [
            left,
            top,
            left.checked_add(width)
                .ok_or_else(|| io::Error::other("region x overflow"))?,
            top.checked_add(height)
                .ok_or_else(|| io::Error::other("region y overflow"))?,
        ],
    };
    let seconds: u64 = args.number("seconds", "120")?;
    if !(1..=120).contains(&seconds) {
        return Err(io::Error::other("capture lifetime must be1..120 seconds"));
    }
    args.done()?;
    let mut capture = DesktopCapture::new(config).map_err(io::Error::other)?;
    let (tx, rx) = mpsc::sync_channel(8);
    std::thread::spawn(move || {
        let mut reader = io::stdin().lock();
        loop {
            let request = match read_request(&mut reader) {
                Ok(None) => break,
                Ok(Some(request)) => Ok(request),
                Err(error) => Err(error),
            };
            let stop = !matches!(&request, Ok(Request::Stamp { .. }));
            if tx.send(request).is_err() || stop {
                return;
            }
        }
        let _ = tx.send(Ok(Request::Stop));
    });
    let mut writer = io::BufWriter::new(io::stdout().lock());
    emit(
        &mut writer,
        Event::Ready {
            frequency: capture.frequency().to_string(),
            geometry: capture.geometry(),
            region: config.region,
        },
    )?;
    let deadline = Instant::now() + Duration::from_secs(seconds);
    let mut stamps = 0;
    let mut last_id = 0;
    while Instant::now() < deadline {
        while let Ok(request) = rx.try_recv() {
            match request? {
                Request::Stop => {
                    emit(&mut writer, Event::Stopped)?;
                    return Ok(());
                }
                Request::Stamp { id } => {
                    stamps += 1;
                    if stamps > 128 || id.get() <= last_id {
                        return Err(io::Error::other("capture stamp identity/count exceeded"));
                    }
                    last_id = id.get();
                    emit(
                        &mut writer,
                        Event::Stamped {
                            id: id.get(),
                            qpc: capture.stamp().map_err(io::Error::other)?.to_string(),
                        },
                    )?;
                }
            }
        }
        if let Some(frame) = capture.next_frame().map_err(io::Error::other)? {
            emit(
                &mut writer,
                Event::Frame {
                    serial: frame.serial,
                    acquire_begin_qpc: frame.acquire_begin_qpc.to_string(),
                    readback_end_qpc: frame.readback_end_qpc.to_string(),
                    desktop_update_qpc: frame.desktop_update_qpc.to_string(),
                    accumulated_frames: frame.accumulated_frames,
                    bgra: frame.bgra,
                },
            )?;
        }
    }
    Err(io::Error::other("bounded desktop capture lifetime expired"))
}

#[cfg(test)]
mod tests;
