//! Importing a file builds one owned Media tree or edits an existing channel's typed File.
use super::{
    authoring::{channel, configure_channel, label_with_reserved, manager, media_tree},
    protocol::*,
};
use crate::app::AppEngine;
use crate::app::AppNode;
use golden_assets::AbsoluteAssetPath;
use golden_core::edit::{Edit, NodeTree};
use golden_core::node::{Node, NodeId, NodeUuid};
use golden_video::shader::FragmentFormat;
use std::path::Path;

/// A native file classified by MapGyver, ready to become a detached, configured Media.
pub(crate) struct FileMedia {
    source: ChannelSource,
    label: Option<String>,
}

impl FileMedia {
    pub(crate) fn new(path: AbsoluteAssetPath) -> Result<Self, String> {
        let native = Path::new(path.as_str());
        let shader = native
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(super::shader::file_format);
        let label = shader
            .map(|(label, _)| label)
            .or_else(|| native.file_stem().and_then(|stem| stem.to_str()))
            .filter(|stem| !stem.trim().is_empty() && stem.len() <= 240)
            .map(str::to_owned);
        let format = shader.map(|(_, format)| format);
        Ok(Self {
            source: file_source(path, format)?,
            label,
        })
    }

    pub(crate) fn tree(
        self,
        engine: &AppEngine,
        parent: Option<NodeId>,
    ) -> Result<(NodeTree, ChannelReference), String> {
        prepare_tree(engine, parent, self.source, self.label)
    }
}

pub(super) fn picture(
    engine: &mut AppEngine,
    locator: AssetLocator,
    target: Option<ChannelReference>,
    label: Option<String>,
) -> Result<MediaEditResult, String> {
    let source = ChannelSource::Picture { file: Some(locator) };
    let reference = if let Some(reference) = target {
        if label.is_some() {
            return Err("media label is only valid when creating Picture media".into());
        }
        if channel(engine, reference)?.definition.media_kind() != MediaKind::Picture {
            return Err("Picture import requires a Picture channel".into());
        }
        configure_channel(engine, reference, source)?;
        reference
    } else {
        let parent = manager(engine)?;
        append(engine, parent, source, label)?
    };
    Ok(MediaEditResult::PictureImported { reference })
}

pub(super) fn drop_file(
    engine: &mut AppEngine,
    path: AbsoluteAssetPath,
    target: Option<NodeUuid>,
) -> Result<MediaEditResult, String> {
    let file = FileMedia::new(path)?;
    let parent = match target {
        Some(uuid) => {
            let node = engine.node_id_by_uuid(uuid).ok_or("File drop target is missing")?;
            match engine.nodes.get(node) {
                Some(AppNode::MediaFolder(_)) => super::folders::parent(engine, Some(uuid))?,
                Some(AppNode::Media(_)) => {
                    let reference = super::MediaSource {
                        media: MediaId(uuid),
                        channel: None,
                    }
                    .output(engine)?
                    .ok_or("File drop target has no channel")?;
                    configure_channel(engine, reference, file.source)?;
                    return Ok(MediaEditResult::Changed);
                }
                _ => return Err("Drop a file on Medias, a folder, or a compatible Media".into()),
            }
        }
        None => manager(engine)?,
    };
    let reference = append(engine, parent, file.source, file.label)?;
    Ok(MediaEditResult::Created { reference })
}

pub(super) fn library_shader(
    engine: &mut AppEngine,
    shader: LibraryShader,
    folder: Option<NodeUuid>,
) -> Result<MediaEditResult, String> {
    super::shader::validate_code(&shader.code)?;
    if shader.code.trim().is_empty() {
        return Err("Shader code is empty".into());
    }
    let parent = super::folders::parent(engine, folder)?;
    let source = ChannelSource::Shader {
        format: shader.format,
        input: ShaderInput::Embedded { code: shader.code },
        textures: Default::default(),
    };
    let reference = append(engine, parent, source, Some(shader.label))?;
    Ok(MediaEditResult::Created { reference })
}

fn file_source(path: AbsoluteAssetPath, shader: Option<FragmentFormat>) -> Result<ChannelSource, String> {
    if let Some(format) = shader {
        return Ok(ChannelSource::Shader {
            format,
            input: ShaderInput::File {
                file: Some(AssetLocator::Absolute { path }),
            },
            textures: Default::default(),
        });
    }
    let extension = Path::new(path.as_str())
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let file = Some(AssetLocator::Absolute { path });
    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "webp" => Ok(ChannelSource::Picture { file }),
        "mp4" | "mov" | "mkv" | "m4v" | "webm" | "avi" | "mpg" | "mpeg" | "ts" | "m2ts" | "wmv"
            if cfg!(feature = "video-files") =>
        {
            Ok(ChannelSource::Video { file })
        }
        _ => Err("This file is not a supported picture, video, or shader".into()),
    }
}

fn append(
    engine: &mut AppEngine,
    parent: NodeId,
    source: ChannelSource,
    label: Option<String>,
) -> Result<ChannelReference, String> {
    let (tree, reference) = prepare_tree(engine, Some(parent), source, label)?;
    engine.edits.push(Edit::AddUserItemTree {
        parent,
        prev_sibling: engine.nodes.get(parent).and_then(|node| node.node_data().last_child),
        tree,
    });
    Ok(reference)
}

fn prepare_tree(
    engine: &AppEngine,
    parent: Option<NodeId>,
    source: ChannelSource,
    label: Option<String>,
) -> Result<(NodeTree, ChannelReference), String> {
    let definition = ChannelDefinition {
        source,
        ..ChannelDefinition::default()
    };
    let fallback = definition.media_kind().default_label();
    let label = match parent {
        Some(parent) => label_with_reserved(engine, parent, label, fallback, &[])?,
        None => label.as_deref().unwrap_or(fallback).trim().to_owned(),
    };
    Ok(media_tree(label, definition))
}
