//! Surface preview policy: calibration replaces the image without altering its authored source/crop.
use super::protocol::{Calibration, ScreenDefinition, ScreenTarget, SurfaceDefinition};
use crate::{
    app::{AppEngine, AppNode},
    preview::protocol::PreviewSource,
};

pub(super) fn source(
    engine: &AppEngine,
    surface: &SurfaceDefinition,
    screen: &ScreenDefinition,
) -> Result<Option<PreviewSource>, String> {
    if surface.calibration == Calibration::Off {
        return Ok(surface.source.map(|reference| PreviewSource::Channel { reference }));
    }
    let extent = match &screen.target {
        Some(ScreenTarget::Rectangle { bounds }) => Some(bounds.extent),
        Some(ScreenTarget::Spout { extent, .. } | ScreenTarget::AutomaticSpout { extent }) => Some(*extent),
        Some(ScreenTarget::Fullscreen { display }) => {
            let manager = super::authoring::manager(engine)?;
            match engine.nodes.get(manager) {
                Some(AppNode::ScreenManager(manager)) => manager
                    .displays()
                    .iter()
                    .find(|candidate| candidate.id == *display)
                    .map(|candidate| candidate.bounds.extent),
                _ => None,
            }
        }
        None => None,
    }
    .unwrap_or_else(crate::media::resolution::default_canvas);
    Ok(Some(PreviewSource::Calibration {
        mode: surface.calibration,
        extent,
    }))
}
