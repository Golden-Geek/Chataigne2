//! Finite acceptance observations; no pixel readback occurs in ordinary rendering.
use std::fs::{self, OpenOptions};
use std::io::{self, BufWriter, Error};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use golden_io::PayloadPool;
use golden_video::Extent2D;
use golden_video::native::SpoutProbe;

use super::config::Arguments;
use super::roles::selected;

pub fn catalog(args: Arguments) -> io::Result<()> {
    args.done()?;
    println!("{}", serde_json::to_string(&displays()?).map_err(Error::other)?);
    Ok(())
}

pub fn hardware(args: Arguments) -> io::Result<()> {
    args.done()?;
    let mut power = windows::Win32::System::Power::SYSTEM_POWER_STATUS::default();
    // SAFETY: the API writes only this initialized local observation.
    unsafe { windows::Win32::System::Power::GetSystemPowerStatus(&mut power) }.map_err(Error::other)?;
    println!(
        "{}",
        serde_json::json!({
            "adapters": golden_video::native::adapters().map_err(Error::other)?,
            "preferred_adapters": golden_video::native::preferred_adapters().map_err(Error::other)?,
            "displays": displays()?,
            "webview_default_runtime_version": tauri::webview_version().map_err(Error::other)?,
            "ac_line_status": power.ACLineStatus,
        })
    );
    Ok(())
}

fn displays() -> io::Result<Vec<mapgyver::screens::protocol::DisplayInfo>> {
    golden_core::app::native_displays()?
        .into_iter()
        .map(|display| {
            Ok(mapgyver::screens::protocol::DisplayInfo {
                refresh_rate: display.refresh_rate.map(|(numerator, denominator)| {
                    mapgyver::screens::protocol::DisplayRefreshRate { numerator, denominator }
                }),
                id: display.id,
                name: display.name,
                edid: display.edid,
                index: display.index,
                primary: display.primary,
                bounds: mapgyver::screens::protocol::PhysicalRectangle {
                    left: display.bounds.left(),
                    top: display.bounds.top(),
                    extent: Extent2D::new(display.bounds.width(), display.bounds.height()).map_err(Error::other)?,
                },
            })
        })
        .collect()
}

pub fn pixels(args: Arguments) -> io::Result<()> {
    capture_pixels(args, 4 * 1024 * 1024, false)
}

/// M11/10 full-image acceptance: receiving, staging and CPU pixels for one 4K frame.
/// The original small-output observer and every production budget remain unchanged.
pub fn calibration_pixels(args: Arguments) -> io::Result<()> {
    capture_pixels(args, 128 * 1024 * 1024, true)
}

fn capture_pixels(mut args: Arguments, capacity: usize, calibration: bool) -> io::Result<()> {
    let adapter = selected(args.number("adapter", "0")?)?;
    let sender = args.required("name")?;
    if calibration && !sender.starts_with("MapGyver-M1110-") {
        return Err(Error::other("calibration capture requires an owned M1110 sender"));
    }
    let directory = PathBuf::from(args.required("directory")?);
    args.done()?;
    fs::create_dir_all(&directory)?;
    let budget = PayloadPool::new("finite-output-pixels", 4, capacity).map_err(Error::other)?;
    let mut probe = SpoutProbe::new(&sender, adapter, budget.clone()).map_err(Error::other)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let image = loop {
        if let Some(image) = probe.capture_pixels().map_err(Error::other)? {
            break image;
        }
        if Instant::now() >= deadline {
            return Err(Error::other("finite Spout pixel observation timed out"));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let extent = image.extent();
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join("pixels.png"))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), extent.width(), extent.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(Error::other)?;
    writer
        .write_image_data(image.pixels().as_flattened())
        .map_err(Error::other)?;
    writer.finish().map_err(Error::other)?;
    drop(image);
    drop(probe);
    if !budget.wait_for_idle(Duration::from_secs(5)) {
        return Err(Error::other("finite pixel payload did not retire"));
    }
    println!(
        "{}",
        serde_json::json!({"width":extent.width(),"height":extent.height(),"retired":true})
    );
    Ok(())
}
