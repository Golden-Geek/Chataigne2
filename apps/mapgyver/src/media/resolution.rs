//! Backend source validation and size policy; observations are separate from project data.

use golden_video::plan::SampleFilter;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, LinearColor, PixelFormat, TextureDescriptor};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::protocol::{ChannelDefinition, ChannelSource, MediaKind, SizePolicy};

pub const MAX_CHANNEL_DIMENSION: u32 = 16_384;

pub fn default_canvas() -> Extent2D {
    Extent2D::new(1920, 1080).expect("nonempty backend canvas")
}

impl Default for ChannelDefinition {
    fn default() -> Self {
        Self::for_media(MediaKind::SolidColor)
    }
}

impl ChannelDefinition {
    pub fn for_media(kind: MediaKind) -> Self {
        let source = match kind {
            MediaKind::Picture => ChannelSource::Picture { asset: None },
            MediaKind::Composition => ChannelSource::Composition {
                canvas: default_canvas(),
            },
            MediaKind::Matrix => ChannelSource::Matrix {
                output: crate::matrix::protocol::MatrixOutput::Final {
                    definition: crate::matrix::protocol::MatrixDefinition::default(),
                },
            },
            MediaKind::SolidColor => ChannelSource::SolidColor { rgba: [1.0; 4] },
            MediaKind::SharedTexture => ChannelSource::Spout {
                sender: None,
                maximum: TextureDescriptor {
                    extent: default_canvas(),
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
            },
        };
        Self {
            source,
            size: SizePolicy::Automatic,
            filter: SampleFilter::Linear,
        }
    }

    pub fn media_kind(&self) -> MediaKind {
        match self.source {
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
            ChannelSource::Spout { sender, maximum } => {
                validate_extent(maximum.extent)?;
                maximum.byte_len().map_err(|e| e.to_string())?;
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
        ChannelSource::Picture { asset: None } => (None, ChannelAvailability::Unconfigured),
        ChannelSource::Picture { asset: Some(_) } => match observed {
            SourceObservation::Unavailable => (None, ChannelAvailability::Unavailable),
            SourceObservation::Available(actual) => (Some(actual.extent), ChannelAvailability::Ready),
        },
        ChannelSource::Spout { sender: None, .. } => (None, ChannelAvailability::Unconfigured),
        ChannelSource::Spout { maximum, .. } => match observed {
            SourceObservation::Unavailable => (None, ChannelAvailability::Unavailable),
            SourceObservation::Available(actual)
                if actual.extent.width() <= maximum.extent.width()
                    && actual.extent.height() <= maximum.extent.height()
                    && actual.format == maximum.format
                    && actual.color_encoding == maximum.color_encoding
                    && actual.alpha_mode == maximum.alpha_mode =>
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
