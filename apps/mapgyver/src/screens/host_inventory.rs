//! Candidate metadata preparation and host-worker refresh; never authored target reassignment.
use super::protocol::{DisplayInfo, DisplayRefreshRate, PhysicalRectangle};
use golden_video::Extent2D;
use std::io;

#[cfg(windows)]
pub(crate) mod headless;

/// Loaded candidates have no persisted inventory. Fresh candidates retain their prepared
/// catalog, including a prepared-empty fallback, rather than doing another OS query.
pub(crate) fn prepare(engine: &mut crate::app::AppEngine) -> Result<(), String> {
    let id = super::authoring::manager(engine)?;
    let Some(crate::app::AppNode::ScreenManager(manager)) = engine.nodes.get_mut(id) else {
        unreachable!()
    };
    if !manager.has_display_catalog() {
        manager.set_displays(initial().into());
    }
    Ok(())
}

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
