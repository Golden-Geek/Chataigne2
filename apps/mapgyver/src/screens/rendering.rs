//! Capture on the actor; solve mapping and build layers on the preparation worker.

use super::{
    authoring,
    protocol::{Calibration, ScreenId, ScreenTarget, SurfaceDefinition},
};
use crate::{
    app::{AppEngine, AppNode},
    media::{capture_channels, protocol::ChannelReference},
};
use golden_core::node::Node;
use golden_io::RevisionGuard;
use golden_video::plan::{BlendMode, Layer, ProjectiveMapping, SampleFilter};
use golden_video::{Extent2D, Opacity, OutputId, TextureId};
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
pub struct CapturedScreen {
    pub(crate) project: uuid::Uuid,
    pub(crate) id: ScreenId,
    pub(crate) guards: Vec<RevisionGuard>,
    surfaces: Vec<SurfaceDefinition>,
    target_extent: Option<Extent2D>,
}

/// Demand for a composed Screen texture. Target activation/placement is a separate host owner;
/// editor previews may demand this texture while the authored physical output is disabled.
#[derive(Clone)]
pub struct ScreenRoute {
    pub output: Option<OutputId>,
    pub extent: Extent2D,
    pub screen: CapturedScreen,
}

impl CapturedScreen {
    pub fn preview_extent(&self) -> Extent2D {
        self.target_extent
            .unwrap_or_else(crate::media::resolution::default_canvas)
    }
    pub fn preview_error(&self) -> Option<String> {
        self.surfaces
            .iter()
            .any(|surface| surface.enabled && surface.calibration != Calibration::Off)
            .then(|| "Calibration pattern rendering is not available yet".into())
    }
    pub fn id(&self) -> ScreenId {
        self.id
    }
    pub fn is_current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }

    pub fn references(&self) -> Vec<ChannelReference> {
        let mut seen = HashSet::new();
        self.surfaces
            .iter()
            .filter(|surface| surface.enabled && surface.calibration == Calibration::Off)
            .filter_map(|surface| surface.source)
            .filter(|source| seen.insert(*source))
            .collect()
    }

    pub(crate) fn layers(&self, channels: &HashMap<ChannelReference, TextureId>) -> Result<Vec<Layer>, String> {
        if !self.is_current() {
            return Err("Screen capture is stale".into());
        }
        let mut layers = Vec::with_capacity(self.surfaces.len());
        for surface in self.surfaces.iter().filter(|surface| surface.enabled) {
            if surface.calibration != Calibration::Off {
                return Err("calibration pattern rendering is not implemented yet".into());
            }
            let Some(source) = surface.source else { continue };
            layers.push(Layer {
                source: *channels.get(&source).ok_or("Surface source was not captured")?,
                opacity: Opacity::new(surface.opacity).map_err(|error| error.to_string())?,
                blend: BlendMode::Normal,
                filter: SampleFilter::Linear,
                mapping: Some(ProjectiveMapping::new(surface.crop, surface.quad).map_err(|error| error.to_string())?),
            });
        }
        Ok(layers)
    }
}

pub fn capture_screen(engine: &AppEngine, screen: ScreenId, maximum_surfaces: usize) -> Result<CapturedScreen, String> {
    let manager = authoring::manager(engine)?;
    let Some(AppNode::ScreenManager(owner)) = engine.nodes.get(manager) else {
        unreachable!()
    };
    let id = engine.node_id_by_uuid(screen.0).ok_or("Screen reference is missing")?;
    let Some(AppNode::Screen(node)) = engine.nodes.get(id) else {
        return Err("reference is not a Screen".into());
    };
    if node.node_data().parent != Some(manager) {
        return Err("Screen is outside its manager".into());
    }
    let project = capture_channels(engine, &[], 0)?;
    let mut guards = vec![owner.guard(), node.guard()];
    guards.extend(project.guards.iter().cloned());
    let mut surfaces = Vec::new();
    let mut child = node.node_data().first_child;
    while let Some(id) = child {
        if surfaces.len() == maximum_surfaces {
            return Err("Screen exceeds admitted Surface count".into());
        }
        let Some(AppNode::Surface(surface)) = engine.nodes.get(id) else {
            return Err("Screen children must be Surfaces".into());
        };
        let mut definition = surface.definition().clone();
        definition.enabled &= surface.node_data().effective_enabled;
        surfaces.push(definition);
        guards.push(surface.guard());
        child = surface.node_data().next_sibling;
    }
    Ok(CapturedScreen {
        project: project.project,
        id: screen,
        guards,
        surfaces,
        target_extent: match &node.definition().target {
            Some(ScreenTarget::Rectangle { bounds }) => Some(bounds.extent),
            Some(ScreenTarget::Spout { extent, .. }) => Some(*extent),
            _ => None,
        },
    })
}

pub(crate) fn membership_guard(engine: &AppEngine) -> Result<RevisionGuard, String> {
    let id = authoring::manager(engine)?;
    let Some(AppNode::ScreenManager(manager)) = engine.nodes.get(id) else {
        unreachable!()
    };
    Ok(manager.guard())
}

pub(crate) fn screen_guard(engine: &AppEngine, screen: ScreenId) -> Option<RevisionGuard> {
    let node = engine.node_id_by_uuid(screen.0).and_then(|id| engine.nodes.get(id))?;
    let AppNode::Screen(node) = node else { return None };
    Some(node.guard())
}
