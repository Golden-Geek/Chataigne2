//! App-owned derived-state reconciliation.
mod channels;
mod migration;
mod service;
#[cfg(test)]
mod tests;
pub(super) use service::DerivedWork;
use service::{Position, Repair, Stage};

use golden_core::{
    edit::{Edit, NodeTree},
    node::{DeclId, Folder, Node, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions, NodeUuid},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, RangeConstraint},
    process_ctx::{ProcessCtx, ProcessTreeNodeSnapshot, ProcessTreeSnapshot},
};
use uuid::Uuid;

use super::{find_child_by_key, find_path, SoundCardModule, NO_AUDIO_DEVICE};
use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardInputValues, SoundCardOutputValues, SoundCardPitchValues, SoundCardSpectralValues,
};

pub(super) const SPECTRAL_PARAMETERS_UUID_KEY: &[u8] = b"spectral-analysis-parameters";

const INTENT_PATHS: [&str; 8] = [
    "connection/audio_driver",
    "connection/device",
    "connection/input_device",
    "connection/output_device",
    "connection/input_routing/channel_count",
    "connection/output_routing/channel_count",
    "parameters/processing/pitch_detection",
    "parameters/processing/spectral_analysis",
];

fn advance(pos: &mut Position) {
    *pos = Position {
        stage: pos.stage.next(),
        ..Position::default()
    };
}

fn direction_used(snapshot: &ProcessTreeSnapshot, module: NodeId, direction: &str) -> bool {
    let value = |path: &str| {
        find_path(snapshot, module, path)
            .and_then(|id| snapshot.node(id))
            .and_then(|n| n.param_value.as_ref())
            .and_then(|v| match v {
                ParamValue::Enum(v) | ParamValue::Str(v) => Some(v.as_str()),
                _ => None,
            })
    };
    let driver = value("connection/audio_driver");
    if driver == Some(super::NO_AUDIO_DRIVER) {
        return false;
    }
    let path = if driver == Some(super::ASIO_AUDIO_DRIVER) {
        "connection/device"
    } else if direction == "input" {
        "connection/input_device"
    } else {
        "connection/output_device"
    };
    value(path).is_some_and(|v| v != NO_AUDIO_DEVICE)
}

fn scan_child<'a>(
    source: &ProcessTreeSnapshot,
    snapshot: &'a ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
    pos: &mut Position,
) -> Option<&'a ProcessTreeNodeSnapshot> {
    let parent = find_path(source, module, path);
    let children = parent.map(|p| source.child_ids_slice(p)).unwrap_or_default();
    let Some(&id) = children.get(pos.row) else {
        advance(pos);
        return None;
    };
    pos.row += 1;
    if parent != find_path(snapshot, module, path) {
        return None;
    }
    snapshot
        .node(id)
        .filter(|n| n.parent == parent && source.node(id).is_some_and(|old| old.uuid == n.uuid))
}

fn step(
    source: &ProcessTreeSnapshot,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    pos: &mut Position,
    wanted: &mut [std::collections::HashSet<NodeUuid>; 4],
) -> Result<Option<Repair>, String> {
    if matches!(
        pos.stage,
        Stage::InputRoot | Stage::OutputRoot | Stage::SpectralParameters
    ) {
        let namespace = snapshot.node(module).ok_or("Sound Card retired")?.uuid;
        let key = match pos.stage {
            Stage::InputRoot => b"input-values".as_slice(),
            Stage::OutputRoot => b"output-values".as_slice(),
            _ => SPECTRAL_PARAMETERS_UUID_KEY,
        };
        let uuid = derived_uuid(namespace, key);
        if let Some(state) = snapshot.node_id_by_uuid(uuid).and_then(|id| snapshot.node(id)) {
            let allowed = if pos.stage == Stage::SpectralParameters {
                state.parent == find_path(snapshot, module, "parameters/processing")
            } else {
                state.parent == find_path(snapshot, module, "values/levels")
                    || state.parent == find_path(snapshot, module, "values")
            };
            if !allowed {
                return Err("Sound Card derived identity belongs to another folder".into());
            }
        }
    }
    Ok(match pos.stage {
        Stage::InputRoot => migration::direction_root(source, snapshot, module, "input", pos),
        Stage::OutputRoot => migration::direction_root(source, snapshot, module, "output", pos),
        Stage::InputGains => return channels::desired(snapshot, module, "input", pos, false, &mut wanted[0]),
        Stage::OutputGains => return channels::desired(snapshot, module, "output", pos, false, &mut wanted[1]),
        Stage::StaleInputGains => channels::stale(source, snapshot, module, "input", pos, false, &wanted[0]),
        Stage::StaleOutputGains => channels::stale(source, snapshot, module, "output", pos, false, &wanted[1]),
        Stage::InputLevels => return channels::desired(snapshot, module, "input", pos, true, &mut wanted[2]),
        Stage::OutputLevels => return channels::desired(snapshot, module, "output", pos, true, &mut wanted[3]),
        Stage::StaleInputLevels => channels::stale(source, snapshot, module, "input", pos, true, &wanted[2]),
        Stage::StaleOutputLevels => channels::stale(source, snapshot, module, "output", pos, true, &wanted[3]),
        Stage::SpectralParameters => migration::spectral(source, snapshot, module, pos),
        Stage::PitchValues => migration::optional(snapshot, module, pos, false),
        Stage::SpectralValues => migration::optional(snapshot, module, pos, true),
        Stage::ClearSpectralValues => migration::cleanup(source, snapshot, module, pos, 0),
        Stage::ObsoleteValues => migration::cleanup(source, snapshot, module, pos, 1),
        Stage::EmptyParameters => migration::cleanup(source, snapshot, module, pos, 2),
        Stage::DirectionOrder => migration::order(snapshot, module, pos),
        Stage::Done => None,
    })
}

impl SoundCardModule {
    pub(super) fn synchronize_derived_structure(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &std::sync::Arc<ProcessTreeSnapshot>,
    ) -> bool {
        let module = self.id();
        let queued = ctx.edits.pending.len();
        let pending = match self.derived.service(ctx, snapshot, module) {
            Ok(pending) => pending,
            Err(error) => {
                self.set_runtime_error(ctx, module, &error);
                if let Some(wake) = &self.runtime_wake {
                    wake.wake_after(std::time::Duration::from_millis(250));
                }
                return true;
            }
        };
        if pending {
            if !self.derived.waiting() || ctx.edits.pending.len() != queued {
                self.wake_route_work();
            }
            return true;
        }
        let mut pending = false;
        for (input, direction) in [(true, "input"), (false, "output")] {
            let resetting = if input {
                self.input_route_reset_pending || self.routes.seeding_input()
            } else {
                self.output_route_reset_pending || self.routes.seeding_output()
            };
            if resetting {
                continue;
            }
            let count = if direction_used(snapshot, module, direction) {
                direction_channel_count(
                    snapshot,
                    module,
                    &format!("connection/{direction}_routing/channel_count"),
                )
            } else {
                0
            };
            let namespace = snapshot.node(module).expect("live Sound Card").uuid;
            let desired = (0..count)
                .map(|index| {
                    NodeReference::new(derived_uuid(
                        namespace,
                        format!("{direction}-channel-{}", index + 1).as_bytes(),
                    ))
                })
                .collect::<Vec<_>>();
            pending |= self.prune_routes(ctx, snapshot, input, &desired);
        }
        pending
    }
}

fn channel_gain_parameter(uuid: NodeUuid, name: &str, decl_id: &str, value: f64) -> Parameter {
    let label = channel_gain_label(name);
    let mut parameter = Parameter::new(
        label.as_str(),
        ParamValue::Float(value),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.range = RangeConstraint::uniform(Some(-120.0), Some(24.0));
    let data = parameter.node_data_mut();
    data.meta.uuid = uuid;
    data.meta.decl_id = DeclId(decl_id.to_owned());
    data.meta.short_name = name.to_owned();
    data.meta.user_permissions = NodeUserPermissions::none();
    data.meta.can_be_disabled = false;
    parameter
}

fn channel_level_parameter(uuid: NodeUuid, name: &str, decl_id: &str, value: f64) -> Parameter {
    let mut parameter = Parameter::new(name, ParamValue::Float(value), ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    parameter.constraints.range = RangeConstraint::uniform(Some(0.0), Some(1.0));
    let data = parameter.node_data_mut();
    data.meta.uuid = uuid;
    data.meta.decl_id = DeclId(decl_id.to_owned());
    data.meta.short_name = decl_id.to_owned();
    data.meta.user_permissions = NodeUserPermissions::none();
    data.meta.can_be_disabled = false;
    parameter
}

fn channel_gain_label(name: &str) -> String {
    format!("{name} Gain")
}

pub(super) fn channel_name(state: &ProcessTreeNodeSnapshot) -> String {
    state
        .label
        .strip_suffix(" Gain")
        .unwrap_or(state.label.as_str())
        .to_owned()
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

pub(super) fn spectral_analysis_enabled(snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
    let Some(namespace) = snapshot.node(module).map(|node| node.uuid) else {
        return false;
    };
    let uuid = derived_uuid(namespace, SPECTRAL_PARAMETERS_UUID_KEY);
    snapshot
        .node_id_by_uuid(uuid)
        .and_then(|node| snapshot.node(node))
        .is_some_and(|node| node.node_type == "folder" && node.enabled)
}

pub(super) fn direction_channel_count(snapshot: &ProcessTreeSnapshot, module: NodeId, path: &str) -> usize {
    find_path(snapshot, module, path)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_int)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(2)
        .clamp(1, 256)
}

fn set_derived_identity(node: &mut dyn Node, namespace: NodeUuid, uuid_key: &str, label: &str, decl_id: &str) {
    let data = node.node_data_mut();
    data.meta.uuid = derived_uuid(namespace, uuid_key.as_bytes());
    data.meta.label = label.to_owned();
    data.meta.decl_id = DeclId(decl_id.to_owned());
    data.meta.short_name = decl_id.to_owned();
}

pub(super) fn set_route_identity(
    route: &mut dyn Node,
    module_uuid: NodeUuid,
    direction: &str,
    physical_channel: &str,
    channel_uuid: NodeUuid,
) {
    let key = format!("{direction}-route-{physical_channel}-{}", channel_uuid.0.simple());
    let data = route.node_data_mut();
    data.meta.uuid = route_uuid(module_uuid, direction, physical_channel, channel_uuid);
    data.meta.label = format!(
        "{} -> {}",
        physical_channel,
        if direction == "input" { "Input" } else { "Output" }
    );
    data.meta.decl_id = DeclId(key.clone());
    data.meta.short_name = key;
}

pub(super) fn route_uuid(module: NodeUuid, direction: &str, physical: &str, channel: NodeUuid) -> NodeUuid {
    derived_uuid(
        module,
        format!("{direction}-route-{physical}-{}", channel.0.simple()).as_bytes(),
    )
}

pub(super) fn patch_channel_label(ctx: &mut ProcessCtx, channel: NodeId, name: String) {
    ctx.patch_node_meta(
        channel,
        NodeMetaPatch {
            short_name: Some(name.clone()),
            label: Some(channel_gain_label(name.as_str())),
            ..NodeMetaPatch::default()
        },
    );
}

pub(super) fn derived_uuid(namespace: NodeUuid, name: &[u8]) -> NodeUuid {
    NodeUuid(Uuid::new_v5(&namespace.0, name))
}
