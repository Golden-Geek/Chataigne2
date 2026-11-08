//! Named script methods translate arguments into the same typed media intents as other callers.
use golden_core::parameter::ParamValue;
use golden_video::Extent2D;
use serde::de::DeserializeOwned;

use super::protocol::*;

pub(super) const METHODS: &[&str] = &[
    "playVideo",
    "pauseVideo",
    "stopVideo",
    "seekVideo",
    "setVideoRate",
    "setVideoLoop",
    "stepVideo",
    "configureVideoFile",
    "inspectVideo",
    "inspectVideoFrame",
    "inspectVideoSource",
    "dropMatrixMedia",
    "transferMatrixCell",
    "dropCompositionMedia",
    "createCompositionMedia",
    "createReference",
    "configureReference",
    "selectReferenceChannel",
    "createMedia",
    "addChannel",
    "configureMediaResolution",
    "configureMediaFitMode",
    "configureMediaSenderSelection",
    "configureSolidColor",
    "configurePicture",
    "configurePictureFile",
    "reloadPicture",
    "inspectPictureFile",
    "configureSharedTexture",
    "inspectChannel",
    "inspectChannels",
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
        "playVideo" | "pauseVideo" | "stopVideo" | "inspectVideo" => 1,
        "seekVideo" | "setVideoRate" | "setVideoLoop" | "inspectVideoFrame" | "inspectVideoSource" => 2,
        "stepVideo" | "configureVideoFile" => 3,
        "createMedia"
        | "addChannel"
        | "configureMediaFitMode"
        | "configureMediaSenderSelection"
        | "inspectChannel"
        | "inspectPictureFile"
        | "reloadPicture" => 2,
        "configureMediaResolution" => 4,
        "configureSolidColor" => 6,
        "configurePicture" | "configurePictureFile" | "configureSharedTexture" => 3,
        "importPicture" => 2,
        "executeCommand" => 1,
        "inspectChannels" => 3,
        "dropCompositionMedia" => 7,
        "createCompositionMedia" => 6,
        "dropMatrixMedia" => 5,
        "transferMatrixCell" => 5,
        "createReference" | "configureReference" | "selectReferenceChannel" => 2,
        _ => return Ok(None),
    };
    if args.len() != count {
        return Err(format!("{method} expects a request id and {count} argument(s)"));
    }
    Ok(Some(match method {
        "inspectVideoSource" => MediaEdit::InspectVideoSource {
            reference: reference(args)?,
        },
        "inspectVideoFrame" => MediaEdit::InspectVideoFrame {
            reference: reference(args)?,
        },
        "playVideo" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Play,
        },
        "pauseVideo" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Pause,
        },
        "stopVideo" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Stop,
        },
        "seekVideo" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Seek {
                position_ms: number(args, 1)?,
            },
        },
        "setVideoRate" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Rate { rate: number(args, 1)? },
        },
        "setVideoLoop" => {
            let Some(ParamValue::Bool(looping)) = args.get(1) else {
                return Err("Video loop must be boolean".into());
            };
            MediaEdit::Video {
                media: MediaId(uuid(args, 0)?),
                edit: super::video::protocol::VideoEdit::Loop { looping: *looping },
            }
        }
        "stepVideo" => MediaEdit::Video {
            media: MediaId(uuid(args, 0)?),
            edit: super::video::protocol::VideoEdit::Step {
                channel: ChannelId(uuid(args, 1)?),
                direction: choice(args, 2)?,
            },
        },
        "configureVideoFile" => MediaEdit::ConfigureVideoFile {
            reference: reference(args)?,
            file: text(args, 2)?.into(),
        },
        "inspectVideo" => MediaEdit::InspectVideo {
            media: MediaId(uuid(args, 0)?),
        },
        "transferMatrixCell" => MediaEdit::Matrix {
            media: MediaId(uuid(args, 0)?),
            edit: crate::matrix::protocol::MatrixEdit::TransferCell {
                cell: crate::matrix::protocol::CellId(uuid(args, 1)?),
                layer: crate::matrix::protocol::LayerId(uuid(args, 2)?),
                column: crate::matrix::protocol::ColumnId(uuid(args, 3)?),
                action: choice(args, 4)?,
            },
        },
        "createCompositionMedia" => MediaEdit::Composition {
            reference: reference(args)?,
            edit: super::composition::protocol::CompositionEdit::CreateMedia {
                media_kind: choice(args, 2)?,
                label: optional_text(args, 3)?,
                position: golden_video::plan::Point {
                    x: number(args, 4)? as f32,
                    y: number(args, 5)? as f32,
                },
            },
        },
        "dropMatrixMedia" => {
            let Some(ParamValue::Bool(copy)) = args.get(4) else {
                return Err("copy must be boolean".into());
            };
            MediaEdit::Matrix {
                media: MediaId(uuid(args, 0)?),
                edit: crate::matrix::protocol::MatrixEdit::DropSource {
                    layer: crate::matrix::protocol::LayerId(uuid(args, 1)?),
                    column: crate::matrix::protocol::ColumnId(uuid(args, 2)?),
                    source: uuid(args, 3)?,
                    copy: *copy,
                },
            }
        }
        "dropCompositionMedia" => {
            let Some(ParamValue::Bool(copy)) = args.get(4) else {
                return Err("copy must be boolean".into());
            };
            MediaEdit::Composition {
                reference: reference(args)?,
                edit: super::composition::protocol::CompositionEdit::DropSource {
                    source: uuid(args, 2)?,
                    instance: optional_text(args, 3)?
                        .map(|value| {
                            uuid::Uuid::parse_str(&value)
                                .map(|id| super::composition::protocol::InstanceId(golden_core::node::NodeUuid(id)))
                        })
                        .transpose()
                        .map_err(|e| e.to_string())?,
                    copy: *copy,
                    position: golden_video::plan::Point {
                        x: number(args, 5)? as f32,
                        y: number(args, 6)? as f32,
                    },
                },
            }
        }
        "inspectChannels" => {
            let start = number(args, 1)?;
            let count = number(args, 2)?;
            if start.fract() != 0.0
                || !(0.0..=f64::from(u32::MAX)).contains(&start)
                || count.fract() != 0.0
                || !(1.0..=128.0).contains(&count)
            {
                return Err("channel pages require a whole nonnegative start and count in 1..128".into());
            }
            MediaEdit::InspectChannels {
                query: MediaChannelsQuery {
                    media: MediaId(uuid(args, 0)?),
                    start: start as u32,
                    count: count as u16,
                },
            }
        }
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
        "selectReferenceChannel" => MediaEdit::SelectReferenceChannel {
            media: MediaId(uuid(args, 0)?),
            channel: optional_text(args, 1)?
                .map(|value| uuid::Uuid::parse_str(&value).map(|id| ChannelId(golden_core::node::NodeUuid(id))))
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
        "configureMediaFitMode" => MediaEdit::ConfigureMediaFitMode {
            media: MediaId(uuid(args, 0)?),
            mode: choice(args, 1)?,
        },
        "configureMediaSenderSelection" => MediaEdit::ConfigureMediaSenderSelection {
            media: MediaId(uuid(args, 0)?),
            mode: choice(args, 1)?,
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
                file: super::assets::parse_file(optional_text(args, 2)?.as_deref().unwrap_or_default())?,
            },
        },
        "configurePictureFile" => MediaEdit::ConfigurePictureFile {
            reference: reference(args)?,
            file: text(args, 2)?.to_owned(),
        },
        "inspectPictureFile" => MediaEdit::InspectPictureFile {
            reference: reference(args)?,
        },
        "reloadPicture" => MediaEdit::ReloadPicture {
            reference: reference(args)?,
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
