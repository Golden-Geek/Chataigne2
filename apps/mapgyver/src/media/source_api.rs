//! Named script methods translate arguments into the same typed media intents as other callers.
use golden_core::parameter::ParamValue;
use golden_video::{Extent2D, plan::SampleFilter};
use serde::de::DeserializeOwned;

use super::protocol::*;

pub(super) const METHODS: &[&str] = &[
    "createReference",
    "configureReference",
    "createMedia",
    "addChannel",
    "configureMediaResolution",
    "configureMediaSampling",
    "configureSolidColor",
    "configurePicture",
    "configureSharedTexture",
    "inspectChannel",
    "importPicture",
    "executeCommand",
];

fn text(args: &[ParamValue], index: usize) -> Result<&str, String> {
    match args.get(index) {
        Some(ParamValue::Str(value)) => Ok(value),
        _ => Err(format!("argument {} must be a string", index + 1)),
    }
}
fn number(args: &[ParamValue], index: usize) -> Result<f64, String> {
    match args.get(index) {
        Some(ParamValue::Float(value)) if value.is_finite() => Ok(*value),
        Some(ParamValue::Int(value)) => Ok((*value).into()),
        _ => Err(format!("argument {} must be a finite number", index + 1)),
    }
}
fn uuid(args: &[ParamValue], index: usize) -> Result<golden_core::node::NodeUuid, String> {
    uuid::Uuid::parse_str(text(args, index)?)
        .map(golden_core::node::NodeUuid)
        .map_err(|error| error.to_string())
}
fn choice<T: DeserializeOwned>(args: &[ParamValue], index: usize) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(text(args, index)?.into())).map_err(|error| error.to_string())
}
fn optional_text(args: &[ParamValue], index: usize) -> Result<Option<String>, String> {
    Ok(match text(args, index)? {
        "" => None,
        value => Some(value.into()),
    })
}
fn reference(args: &[ParamValue]) -> Result<ChannelReference, String> {
    Ok(ChannelReference {
        media: MediaId(uuid(args, 0)?),
        channel: ChannelId(uuid(args, 1)?),
    })
}

/// Arguments exclude the request id; invalid argument shapes never enqueue partial operations.
pub(super) fn operation(method: &str, args: &[ParamValue]) -> Result<Option<MediaEdit>, String> {
    let count = match method {
        "createMedia" | "addChannel" | "configureMediaSampling" | "inspectChannel" => 2,
        "configureMediaResolution" => 4,
        "configureSolidColor" => 6,
        "configurePicture" | "configureSharedTexture" => 3,
        "importPicture" => 2,
        "executeCommand" => 1,
        "createReference" | "configureReference" => 2,
        _ => return Ok(None),
    };
    if args.len() != count {
        return Err(format!("{method} expects a request id and {count} argument(s)"));
    }
    Ok(Some(match method {
        "createReference" => MediaEdit::CreateReference {
            target: optional_text(args, 0)?
                .map(|value| uuid::Uuid::parse_str(&value).map(|id| MediaId(golden_core::node::NodeUuid(id))))
                .transpose()
                .map_err(|error| error.to_string())?,
            label: optional_text(args, 1)?,
        },
        "configureReference" => MediaEdit::ConfigureReference {
            media: MediaId(uuid(args, 0)?),
            target: optional_text(args, 1)?
                .map(|value| uuid::Uuid::parse_str(&value).map(|id| MediaId(golden_core::node::NodeUuid(id))))
                .transpose()
                .map_err(|error| error.to_string())?,
        },
        "executeCommand" => MediaEdit::ExecuteCommand {
            command: uuid(args, 0)?,
        },
        "createMedia" => MediaEdit::CreateMedia {
            media_kind: choice(args, 0)?,
            label: optional_text(args, 1)?,
        },
        "addChannel" => MediaEdit::AddChannel {
            media: MediaId(uuid(args, 0)?),
            label: optional_text(args, 1)?,
        },
        "configureMediaSampling" => MediaEdit::ConfigureMediaSampling {
            media: MediaId(uuid(args, 0)?),
            filter: choice::<SampleFilter>(args, 1)?,
        },
        "configureMediaResolution" => {
            let Some(ParamValue::Bool(enabled)) = args.get(1) else {
                return Err("Resolution enabled must be boolean".into());
            };
            let size = if *enabled {
                let width = number(args, 2)?;
                let height = number(args, 3)?;
                if [width, height]
                    .iter()
                    .any(|value| value.fract() != 0.0 || !(1.0..=f64::from(u32::MAX)).contains(value))
                {
                    return Err("Resolution arguments require whole nonzero 32-bit pixel dimensions".into());
                }
                SizePolicy::Fixed {
                    extent: Extent2D::new(width as u32, height as u32).map_err(|error| error.to_string())?,
                }
            } else {
                // Automatic does not overwrite the media's retained authored pixel values.
                number(args, 2)?;
                number(args, 3)?;
                SizePolicy::Automatic
            };
            MediaEdit::ConfigureMediaResolution {
                media: MediaId(uuid(args, 0)?),
                size,
            }
        }
        "configureSolidColor" => MediaEdit::ConfigureChannel {
            reference: reference(args)?,
            source: ChannelSource::SolidColor {
                rgba: [
                    number(args, 2)? as f32,
                    number(args, 3)? as f32,
                    number(args, 4)? as f32,
                    number(args, 5)? as f32,
                ],
            },
        },
        "configurePicture" => MediaEdit::ConfigureChannel {
            reference: reference(args)?,
            source: ChannelSource::Picture {
                asset: optional_text(args, 2)?
                    .map(|value| uuid::Uuid::parse_str(&value).map(AssetId::from_uuid))
                    .transpose()
                    .map_err(|error| error.to_string())?,
            },
        },
        "configureSharedTexture" => MediaEdit::ConfigureChannel {
            reference: reference(args)?,
            source: ChannelSource::Spout {
                sender: optional_text(args, 2)?,
            },
        },
        "inspectChannel" => MediaEdit::InspectChannel {
            reference: reference(args)?,
        },
        "importPicture" => {
            let path = text(args, 0)?.to_owned();
            let Some(ParamValue::Bool(absolute)) = args.get(1) else {
                return Err("absolute path selector must be boolean".into());
            };
            let locator = if *absolute {
                AssetLocator::Absolute {
                    path: path
                        .try_into()
                        .map_err(|error: golden_assets::LocatorError| error.to_string())?,
                }
            } else {
                AssetLocator::ProjectRelative {
                    path: path
                        .try_into()
                        .map_err(|error: golden_assets::LocatorError| error.to_string())?,
                }
            };
            MediaEdit::ImportPicture {
                locator,
                target: None,
                label: None,
            }
        }
        _ => unreachable!("known method"),
    }))
}
