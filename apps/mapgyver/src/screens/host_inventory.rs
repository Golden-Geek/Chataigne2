//! One-shot candidate metadata preparation; live refresh stays on the preview worker.
use super::protocol::{DisplayInfo, DisplayRefreshRate, PhysicalRectangle};
use golden_video::Extent2D;
use std::io;

pub(crate) fn scan() -> io::Result<Vec<DisplayInfo>> {
    golden_core::app::native_displays()?
        .into_iter()
        .map(|display| {
            Ok(DisplayInfo {
                id: display.id,
                name: display.name,
                edid: display.edid,
                index: display.index,
                primary: display.primary,
                internal: display.internal,
                refresh_rate: display
                    .refresh_rate
                    .map(|(numerator, denominator)| DisplayRefreshRate { numerator, denominator }),
                bounds: PhysicalRectangle {
                    left: display.bounds.left(),
                    top: display.bounds.top(),
                    extent: Extent2D::new(display.bounds.width(), display.bounds.height()).map_err(io::Error::other)?,
                },
            })
        })
        .collect()
}

/// New candidates are detached from the live engine. Query the OS once on a dedicated
/// worker before authoring defaults; no polling, shared cache or post-load retargeting.
pub(super) fn initial() -> Vec<DisplayInfo> {
    let result = std::thread::Builder::new()
        .name("mapgyver-new-displays".into())
        .spawn(scan)
        .and_then(|worker| {
            worker
                .join()
                .map_err(|_| io::Error::other("display preparation worker panicked"))
        })
        .and_then(|result| result);
    match result {
        Ok(displays) => displays,
        Err(error) => {
            eprintln!("[mapgyver] initial display catalog unavailable: {error}; Fullscreen remains unselected");
            Vec::new()
        }
    }
}
