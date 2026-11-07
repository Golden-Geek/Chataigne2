//! Backend source validation and size policy; observations are separate from project data.
mod observations;
pub(crate) use observations::{ResolutionSnapshot, RuntimeResolutions};
#[cfg(test)]
mod tests;

use golden_video::{Extent2D, LinearColor, TextureDescriptor};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::protocol::{ChannelDefinition, ChannelSource, MediaKind, SizePolicy};

pub const MAX_CHANNEL_DIMENSION: u32 = 16_384;

pub fn default_canvas() -> Extent2D {
    Extent2D::new(1920, 1080).expect("nonempty backend canvas")
}

/// Authored sizing first; automatic IO sources use fenced preparation metadata, never engine IO.
pub(crate) fn channel_extent(
    engine: &crate::app::AppEngine,
    mut reference: super::protocol::ChannelReference,
) -> Result<Extent2D, String> {
    let mut seen = std::collections::HashSet::new();
    // Validate virtual channel membership once; do not resolve the full Reference chain per hop.
    let mut definition = super::channel(engine, reference)?.definition;
    loop {
        if seen.len() == 4096 || !seen.insert(reference) {
            return Err("source resolution dependency limit or cycle".into());
        }
        if let SizePolicy::Fixed { extent } = definition.size {
            return Ok(extent);
        }
        if let ChannelSource::Reference { target: Some(target) } = definition.source {
            reference = target;
            definition = super::channel(engine, reference)?.definition;
            continue;
        }
        if let Some(extent) = resolve(&definition, SourceObservation::Unavailable)?.extent {
            return Ok(extent);
        }
        let manager = super::authoring::manager(engine)?;
        let Some(crate::app::AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
            unreachable!()
        };
        return manager.resolutions.extent(reference).ok_or_else(|| {
            "source dimensions are unavailable; wait for source preparation or configure its Resolution".into()
        });
    }
}

impl Default for ChannelDefinition {
    fn default() -> Self {
        Self::for_media(MediaKind::SolidColor)
    }
}

impl ChannelDefinition {
    pub fn for_media(kind: MediaKind) -> Self {
        let source = match kind {
            MediaKind::Reference => ChannelSource::Reference { target: None },
            MediaKind::Picture => ChannelSource::Picture { file: None },
            MediaKind::Composition => ChannelSource::Composition {
                canvas: default_canvas(),
            },
            MediaKind::Matrix => ChannelSource::Matrix {
                output: crate::matrix::protocol::MatrixOutput::Final {
                    definition: crate::matrix::protocol::MatrixDefinition::default(),
                },
            },
            MediaKind::SolidColor => ChannelSource::SolidColor { rgba: [1.0; 4] },
            MediaKind::SharedTexture => ChannelSource::Spout { sender: None },
        };
        Self {
            source,
            size: SizePolicy::Automatic,
            fit_mode: super::protocol::MediaFitMode::Fit,
        }
    }

    pub fn media_kind(&self) -> MediaKind {
        match self.source {
            ChannelSource::Reference { .. } => MediaKind::Reference,
            ChannelSource::Picture { .. } => MediaKind::Picture,
            ChannelSource::Composition { .. } => MediaKind::Composition,
            ChannelSource::Matrix { .. } => MediaKind::Matrix,
            ChannelSource::SolidColor { .. } => MediaKind::SolidColor,
            ChannelSource::Spout { .. } => MediaKind::SharedTexture,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if let SizePolicy::Fixed { extent } = self.size {
            validate_extent(extent)?;
        }
        match &self.source {
            ChannelSource::Reference { .. } => {}
            ChannelSource::Matrix { output } => {
                if let crate::matrix::protocol::MatrixOutput::Final { definition } = output {
                    definition.validate()?;
                }
            }
            ChannelSource::Picture { .. } => {}
            ChannelSource::Composition { canvas } => validate_extent(*canvas)?,
            ChannelSource::SolidColor { rgba } => {
                LinearColor::from_straight(rgba[0], rgba[1], rgba[2], rgba[3]).map_err(|e| e.to_string())?;
            }
            ChannelSource::Spout { sender } => {
                if sender
                    .as_ref()
                    .is_some_and(|s| s.is_empty() || s.len() > 255 || s.contains('\0'))
                {
                    return Err("Spout selector must contain 1..255 UTF-8 bytes without NUL".into());
                }
            }
        }
        Ok(())
    }
}

fn validate_extent(extent: Extent2D) -> Result<(), String> {
    if extent.width() > MAX_CHANNEL_DIMENSION || extent.height() > MAX_CHANNEL_DIMENSION {
        return Err("channel extent exceeds the 16384-texel authoring limit".into());
    }
    Ok(())
}

/// Runtime metadata supplied for the exact selected source; never serialized into a project.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SourceObservation {
    Unavailable,
    Available(TextureDescriptor),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ChannelAvailability {
    Missing,
    Disabled,
    Ready,
    Unconfigured,
    Unavailable,
    Incompatible,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ChannelResolution {
    pub extent: Option<Extent2D>,
    pub availability: ChannelAvailability,
}

pub fn resolve(definition: &ChannelDefinition, observed: SourceObservation) -> Result<ChannelResolution, String> {
    definition.validate()?;
    let (intrinsic, availability) = match &definition.source {
        ChannelSource::Reference { target } => match observed {
            SourceObservation::Available(actual) => (Some(actual.extent), ChannelAvailability::Ready),
            SourceObservation::Unavailable => (
                None,
                if target.is_some() {
                    ChannelAvailability::Missing
                } else {
                    ChannelAvailability::Unconfigured
                },
            ),
        },
        ChannelSource::Matrix { output } => match observed {
            SourceObservation::Available(actual) => (Some(actual.extent), ChannelAvailability::Ready),
            SourceObservation::Unavailable => (
                match output {
                    crate::matrix::protocol::MatrixOutput::Final { definition } => Some(definition.canvas),
                    _ => None,
                },
                ChannelAvailability::Unavailable,
            ),
        },
        ChannelSource::Composition { canvas } => (
            Some(*canvas),
            match observed {
                SourceObservation::Available(actual)
                    if actual.extent
                        == match definition.size {
                            SizePolicy::Automatic => *canvas,
                            SizePolicy::Fixed { extent } => extent,
                        } =>
                {
                    ChannelAvailability::Ready
                }
                SourceObservation::Available(_) => ChannelAvailability::Incompatible,
                SourceObservation::Unavailable => ChannelAvailability::Unavailable,
            },
        ),
        ChannelSource::SolidColor { .. } => (Some(default_canvas()), ChannelAvailability::Ready),
        ChannelSource::Picture { file: None } => (None, ChannelAvailability::Unconfigured),
        ChannelSource::Picture { file: Some(_) } => match observed {
            SourceObservation::Unavailable => (None, ChannelAvailability::Unavailable),
            SourceObservation::Available(actual) => (Some(actual.extent), ChannelAvailability::Ready),
        },
        ChannelSource::Spout { sender: None, .. } => (None, ChannelAvailability::Unconfigured),
        ChannelSource::Spout { .. } => match observed {
            SourceObservation::Unavailable => (None, ChannelAvailability::Unavailable),
            SourceObservation::Available(actual)
                if actual.extent.width() <= MAX_CHANNEL_DIMENSION
                    && actual.extent.height() <= MAX_CHANNEL_DIMENSION =>
            {
                (Some(actual.extent), ChannelAvailability::Ready)
            }
            SourceObservation::Available(_) => (None, ChannelAvailability::Incompatible),
        },
    };
    Ok(ChannelResolution {
        extent: match definition.size {
            SizePolicy::Automatic => intrinsic,
            SizePolicy::Fixed { extent } => Some(extent),
        },
        availability,
    })
}
