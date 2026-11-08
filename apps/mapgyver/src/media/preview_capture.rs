//! Transient Cell editor images reuse Matrix placement without authored channels or playback demand.
use super::{
    capture::{CapturedChannel, CapturedChannels},
    protocol::*,
};
use crate::{
    app::{AppEngine, AppNode},
    matrix::{
        capture::{CapturedMatrix, MatrixInput, MatrixPlacement},
        protocol::*,
    },
    preview::protocol::PreviewSource,
};
use golden_core::node::Node;
use golden_video::plan::BlendMode;

pub(crate) fn capture_editor_channels(
    engine: &AppEngine,
    references: &[ChannelReference],
    editors: &[PreviewSource],
    limit: usize,
    time: Option<golden_time::TimeSnapshot>,
) -> Result<CapturedChannels, String> {
    let mut roots = references.to_vec();
    let mut virtuals = Vec::new();
    let mut guards = Vec::new();
    for &editor in editors {
        let PreviewSource::MatrixCell { media, cell } = editor else {
            continue;
        };
        let reference = editor.reference().expect("Cell preview reference");
        if virtuals
            .iter()
            .any(|entry: &CapturedChannel| entry.reference == reference)
        {
            continue;
        }
        let mut captured = CapturedChannel {
            video: None,
            picture_reload: 0,
            public_output: true,
            reference,
            definition: None,
            enabled: false,
            owned: false,
            instances: None,
            instance_guards: vec![],
            matrix: None,
        };
        if let Ok(root) = crate::matrix::playback::root(engine, media)
            && let Some(AppNode::MediaChannel(root)) = engine.nodes.get(root)
            && let Some(control) = root.matrix_playback()
        {
            guards.push(control.guard());
            if let Some(id) = engine.node_id_by_uuid(cell.0)
                && let Some(AppNode::MatrixCell(node)) = engine.nodes.get(id)
                && let Some(parent) = node.node_data().parent
                && let Some(AppNode::MatrixElement(layer)) = engine.nodes.get(parent)
                && let Some(settings) = layer.layer_settings()?
            {
                let mut next = layer.node_data().parent;
                let mut owned = false;
                for _ in 0..64 {
                    let Some(parent) = next.and_then(|id| engine.nodes.get(id)) else {
                        break;
                    };
                    if let AppNode::Media(owner) = parent {
                        owned = owner.kind() == MediaKind::Matrix && owner.node_data().meta.uuid == media.0;
                        break;
                    }
                    next = parent.node_data().parent;
                }
                if owned {
                    node.bind_runtime(control.binding());
                    layer.bind_runtime(control.binding());
                    let source = node
                        .selection(engine)?
                        .map(|source| source.output(engine))
                        .transpose()?
                        .flatten();
                    roots.extend(source);
                    let placement = node.settings(engine);
                    captured.definition = Some(ChannelDefinition {
                        source: ChannelSource::Matrix {
                            output: MatrixOutput::Final {
                                definition: MatrixDefinition {
                                    canvas: settings.size,
                                    ..Default::default()
                                },
                            },
                        },
                        size: SizePolicy::Fixed { extent: settings.size },
                        fit_mode: MediaFitMode::Stretch,
                    });
                    captured.enabled = true;
                    captured.matrix = Some(CapturedMatrix {
                        applications: Default::default(),
                        observation: MatrixPlanStamp {
                            playback: Default::default(),
                            revision: control.authoring_revision(),
                            columns: vec![],
                            column_groups: vec![],
                            transitioning: false,
                            layers: Default::default(),
                        },
                        canvas: settings.size,
                        opacity: 1.0,
                        inputs: source
                            .map(|source| MatrixInput {
                                source,
                                blend: BlendMode::Normal,
                                weight: None,
                                opacity: placement.opacity,
                                placement: Some(MatrixPlacement {
                                    position: placement.position,
                                    size: placement.size,
                                    fit_mode: placement.fit_mode,
                                }),
                            })
                            .into_iter()
                            .collect(),
                        until: None,
                        control: None,
                        layer: None,
                    });
                }
            }
        }
        virtuals.push(captured);
    }
    if virtuals.len() > limit {
        return Err("Cell editor previews exceed admitted source count".into());
    }
    let mut seen = std::collections::HashSet::new();
    roots.retain(|reference| seen.insert(*reference));
    let mut captured = super::capture::capture_channels_with_time(engine, &roots, limit - virtuals.len(), time)?;
    if !virtuals.is_empty() {
        let manager = super::authoring::manager(engine)?;
        let Some(AppNode::MediaManager(manager)) = engine.nodes.get(manager) else {
            unreachable!()
        };
        guards.push(manager.membership_guard());
    }
    let mut retained = captured.guards.to_vec();
    retained.extend(guards);
    captured.guards = retained.into();
    captured.channels.extend(virtuals);
    Ok(captured)
}

impl CapturedChannels {
    pub(crate) fn editor_dependency(&self, reference: ChannelReference) -> Option<ChannelReference> {
        self.channels
            .iter()
            .find(|channel| channel.reference == reference)?
            .matrix
            .as_ref()?
            .inputs
            .first()
            .map(|input| input.source)
    }
}
