//! Capture on the actor; solve mapping and build layers on the preparation worker.

use super::{
    authoring,
    protocol::{Calibration, DisplayInfo, PhysicalRectangle, ScreenId, ScreenTarget, SurfaceDefinition},
};
use crate::{
    app::{AppEngine, AppNode},
    media::{capture_channels, protocol::ChannelReference},
};
use golden_core::node::Node;
use golden_io::RevisionGuard;
use golden_video::plan::{BlendMode, Layer, LayerSource, ProjectiveMapping, SampleFilter, UnitRect};
use golden_video::{Extent2D, Opacity, OutputId, TextureId};
use std::collections::{HashMap, HashSet};

/// The shared renderer admits eight consumers, with one reserved for the host preview window.
pub const MAX_SCREEN_OUTPUTS: usize = 7;
pub const MAX_SCREEN_CATALOG: usize = 128;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedOutput {
    pub screen: ScreenId,
    pub authored: ScreenTarget,
    pub target: ResolvedTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedTarget {
    Spout { sender: String, extent: Extent2D },
    Rectangle(PhysicalRectangle),
    Fullscreen { display: String, bounds: PhysicalRectangle },
}

impl CapturedOutput {
    pub fn extent(&self) -> Extent2D {
        match &self.target {
            ResolvedTarget::Spout { extent, .. } => *extent,
            ResolvedTarget::Rectangle(bounds) | ResolvedTarget::Fullscreen { bounds, .. } => bounds.extent,
        }
    }
}

pub struct CapturedOutputs {
    pub catalog: Vec<ScreenId>,
    pub outputs: Vec<CapturedOutput>,
    pub guards: Vec<RevisionGuard>,
    pub unavailable: Vec<(ScreenId, String)>,
    pub preview_extents: HashMap<ScreenId, Extent2D>,
}

/// Capture output policy independently of editor registrations. Disabled Screens still contribute
/// guards, so enabling one invalidates the snapshot without scanning the actor every worker tick.
pub fn capture_outputs(
    engine: &AppEngine,
    maximum_screens: usize,
    maximum_outputs: usize,
    displays: &[DisplayInfo],
) -> Result<CapturedOutputs, String> {
    let manager = authoring::manager(engine)?;
    let owner = engine.nodes.get(manager).ok_or("Screen Manager is missing")?;
    let AppNode::ScreenManager(screen_manager) = owner else {
        unreachable!()
    };
    let mut child = owner.node_data().first_child;
    let mut screens = Vec::new();
    let mut outputs = Vec::new();
    let mut guards = vec![membership_guard(engine)?];
    let mut names = HashSet::new();
    let mut unavailable = Vec::new();
    let mut preview_extents = HashMap::new();
    let mut physical_displays = HashSet::new();
    while let Some(id) = child {
        let node = engine.nodes.get(id).ok_or("Screen Manager child is missing")?;
        child = node.node_data().next_sibling;
        let AppNode::Screen(node) = node else { continue };
        if screens.len() == maximum_screens {
            return Err("Screen catalog exceeds admitted count".into());
        }
        let screen = ScreenId(node.node_data().meta.uuid);
        screens.push(screen);
        guards.push(node.guard());
        let hint = node.display_hint();
        let resolved = screen_manager
            .resolved_display(screen, &hint)
            .and_then(|resolved| displays.iter().find(|display| *display == resolved));
        let exact = match &node.definition().target {
            Some(ScreenTarget::Fullscreen { display }) if !screen_manager.has_runtime_allocation() => {
                displays.iter().find(|found| {
                    found.id == *display
                        && (screen_manager.displays().is_empty()
                            || super::protocol::DisplayIdentityHint::from(*found) == hint)
                })
            }
            _ => None,
        };
        if let Some(bounds) = hint.bounds {
            preview_extents.insert(screen, bounds.extent);
        }
        if let Some(ScreenTarget::Fullscreen { .. }) = &node.definition().target
            && let Some(found) = resolved.or(exact)
            && found.bounds.extent.width() <= 8192
            && found.bounds.extent.height() <= 8192
        {
            preview_extents.insert(screen, found.bounds.extent);
        }
        if node.definition().enabled && node.node_data().effective_enabled {
            let Some(target) = &node.definition().target else {
                continue;
            };
            let authored = target.clone();
            let target = match target {
                ScreenTarget::Spout { sender, extent } => {
                    if !names.insert(sender.clone()) {
                        return Err("duplicate Shared Texture output sender name".into());
                    }
                    ResolvedTarget::Spout {
                        sender: sender.clone(),
                        extent: *extent,
                    }
                }
                ScreenTarget::Rectangle { bounds } => ResolvedTarget::Rectangle(*bounds),
                ScreenTarget::Fullscreen { .. } => {
                    let Some(found) = resolved.or(exact) else {
                        unavailable.push((
                            screen,
                            "Intended fullscreen display is unavailable; output is inactive".into(),
                        ));
                        continue;
                    };
                    if found.bounds.extent.width() > 8192 || found.bounds.extent.height() > 8192 {
                        unavailable.push((
                            screen,
                            "Fullscreen display exceeds the admitted 8192-pixel limit; output is inactive".into(),
                        ));
                        continue;
                    }
                    if !physical_displays.insert(found.id.clone()) {
                        unavailable.push((
                            screen,
                            "Fullscreen display reassignment is waiting for a current allocation".into(),
                        ));
                        continue;
                    }
                    ResolvedTarget::Fullscreen {
                        display: found.id.clone(),
                        bounds: found.bounds,
                    }
                }
            };
            if outputs.len() == maximum_outputs {
                return Err("Screen outputs exceed admitted count".into());
            }
            outputs.push(CapturedOutput {
                screen,
                authored,
                target,
            });
        }
    }
    Ok(CapturedOutputs {
        catalog: screens,
        outputs,
        guards,
        unavailable,
        preview_extents,
    })
}

impl CapturedScreen {
    pub(crate) fn layer_count(&self) -> usize {
        self.surfaces.iter().filter(|surface| surface.enabled).count()
    }
    pub fn preview_extent(&self) -> Extent2D {
        self.target_extent
            .unwrap_or_else(crate::media::resolution::default_canvas)
    }
    pub fn has_content(&self) -> bool {
        self.surfaces
            .iter()
            .any(|surface| surface.enabled && (surface.source.is_some() || surface.calibration != Calibration::Off))
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
            let (source, crop) = if let Some(pattern) = super::calibration::pattern(surface.calibration) {
                // Calibration covers the complete authored quad; the saved source crop is retained.
                (LayerSource::Pattern(pattern), UnitRect::FULL)
            } else {
                let Some(source) = surface.source else { continue };
                (
                    (*channels.get(&source).ok_or("Surface source was not captured")?).into(),
                    surface.crop,
                )
            };
            layers.push(Layer {
                source,
                opacity: Opacity::new(surface.opacity).map_err(|error| error.to_string())?,
                opacity_envelope: None,
                blend: BlendMode::Normal,
                filter: SampleFilter::Linear,
                mapping: Some(ProjectiveMapping::new(crop, surface.quad).map_err(|error| error.to_string())?),
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
        let child_node = engine.nodes.get(id).ok_or("Screen child is missing")?;
        child = child_node.node_data().next_sibling;
        if crate::parameters::is_parameter_folder(child_node) {
            continue;
        }
        if surfaces.len() == maximum_surfaces {
            return Err("Screen exceeds admitted Surface count".into());
        }
        let AppNode::Surface(surface) = child_node else {
            return Err("Screen children must be Surfaces".into());
        };
        let mut definition = surface.definition();
        definition.enabled &= surface.node_data().effective_enabled;
        surfaces.push(definition);
        guards.push(surface.guard());
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
