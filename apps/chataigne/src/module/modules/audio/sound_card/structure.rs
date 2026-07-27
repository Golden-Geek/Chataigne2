use std::collections::{HashMap, HashSet};

use golden_audio::PhysicalChannelKey;
use golden_core::{
    edit::NodeTree,
    node::{
        DeclId, Node, NodeHandle, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions, NodeUuid,
    },
    parameter::{ParamValue, Parameter, ParameterChangeCheck, RangeConstraint},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use uuid::Uuid;

use super::{DEFAULT_SPECTRUM_BANDS, NO_AUDIO_DEVICE, SoundCardModule, find_child_by_key, find_path};
use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardInputRoute, SoundCardInputValues, SoundCardOutputRoute, SoundCardOutputValues, SoundCardPitchValues,
    SoundCardSpectralValues, SoundCardSpectrumBand,
};

impl SoundCardModule {
    pub(super) fn synchronize_derived_structure(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let driver_used = self.audio_driver.get_ref().0 != super::NO_AUDIO_DRIVER;
        let (input_device, output_device) = self.selected_device_values();
        let input_used = driver_used && input_device != NO_AUDIO_DEVICE;
        let output_used = driver_used && output_device != NO_AUDIO_DEVICE;

        self.synchronize_input_structure(ctx, snapshot, input_used);
        self.synchronize_output_structure(ctx, snapshot, output_used);
        self.synchronize_processing_values(ctx, snapshot);
    }

    pub(super) fn reset_routes_for_device_selection(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) -> bool {
        let mut changed = false;
        if self.input_route_reset_pending {
            changed |= clear_routes(
                ctx,
                snapshot,
                self.id(),
                "connection/input_routing/routes",
            );
            self.input_route_reset_pending = false;
        }
        if self.output_route_reset_pending {
            changed |= clear_routes(
                ctx,
                snapshot,
                self.id(),
                "connection/output_routing/routes",
            );
            self.output_route_reset_pending = false;
        }
        changed
    }

    fn synchronize_input_structure(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, used: bool) {
        synchronize_direction_values_root(ctx, snapshot, self.id(), "input", used);

        let count = direction_channel_count(snapshot, self.id(), "connection/input_routing/channel_count");
        let channels = synchronize_input_channels(ctx, snapshot, self.id(), count);
        if used {
            synchronize_channel_values(ctx, snapshot, self.id(), "values/input/channels", channels.as_slice());
        }
        remove_stale_routes(
            ctx,
            snapshot,
            self.id(),
            "connection/input_routing/routes",
            channels.as_slice(),
        );
    }

    fn synchronize_output_structure(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, used: bool) {
        synchronize_direction_values_root(ctx, snapshot, self.id(), "output", used);

        let count = direction_channel_count(snapshot, self.id(), "connection/output_routing/channel_count");
        let channels = synchronize_output_channels(ctx, snapshot, self.id(), count);
        if used {
            synchronize_channel_values(ctx, snapshot, self.id(), "values/output/channels", channels.as_slice());
        }
        remove_stale_routes(
            ctx,
            snapshot,
            self.id(),
            "connection/output_routing/routes",
            channels.as_slice(),
        );
    }

    fn synchronize_processing_values(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        synchronize_optional_node(
            ctx,
            snapshot,
            self.id(),
            "values/pitch_detection",
            self.pitch_detection.get(),
            || {
                let mut node = SoundCardPitchValues::new();
                set_derived_identity(
                    &mut node,
                    self.node_data().meta.uuid,
                    "pitch-values",
                    "Pitch Detection",
                    "pitch_detection",
                );
                NodeTree::new(node)
            },
        );
        synchronize_optional_node(
            ctx,
            snapshot,
            self.id(),
            "values/spectral_analysis",
            self.spectral_analysis.get(),
            || {
                let mut node = SoundCardSpectralValues::new();
                set_derived_identity(
                    &mut node,
                    self.node_data().meta.uuid,
                    "spectral-values",
                    "Spectral Analysis",
                    "spectral_analysis",
                );
                NodeTree::new(node)
            },
        );

        if self.spectral_analysis.get() {
            synchronize_spectrum_bands(ctx, snapshot, self.id());
        }
    }

    /// Seeds fresh-project defaults only after the selected device has exposed
    /// its real channel inventory. Persisted projects never enter this path.
    pub(super) fn seed_default_routes_from_inventory(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        input_channels: &[PhysicalChannelKey],
        output_channels: &[PhysicalChannelKey],
    ) -> bool {
        let mut changed = false;
        if self.input_default_routes_pending && !input_channels.is_empty() {
            let channels = channel_references(
                snapshot,
                self.id(),
                "parameters/input/channels",
            );
            if !channels.is_empty() {
                add_default_input_routes(ctx, snapshot, self.id(), channels.as_slice(), input_channels);
                self.input_default_routes_pending = false;
                changed = true;
            }
        }
        if self.output_default_routes_pending && !output_channels.is_empty() {
            let channels = channel_references(
                snapshot,
                self.id(),
                "parameters/output/channels",
            );
            if !channels.is_empty() {
                add_default_output_routes(ctx, snapshot, self.id(), channels.as_slice(), output_channels);
                self.output_default_routes_pending = false;
                changed = true;
            }
        }
        changed
    }
}

fn channel_references(
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
) -> Vec<NodeReference> {
    let Some(parent) = find_path(snapshot, module, path) else {
        return Vec::new();
    };
    snapshot
        .child_ids_slice(parent)
        .iter()
        .filter_map(|channel| {
            let state = snapshot.node(*channel)?;
            if !matches!(state.param_value, Some(ParamValue::Float(_))) {
                return None;
            }
            let mut reference = NodeReference::with_cached_id(state.uuid, Some(*channel));
            reference.set_cached_name(Some(state.label.clone()));
            Some(reference)
        })
        .collect()
}

fn synchronize_direction_values_root(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    direction: &str,
    used: bool,
) {
    let Some(parent) = find_path(snapshot, module, "values") else {
        return;
    };
    let namespace = snapshot.node(module).expect("module exists").uuid;
    let uuid = derived_uuid(namespace, format!("{direction}-values").as_bytes());
    let expected_type = if direction == "input" {
        SoundCardInputValues::NODE_TYPE
    } else {
        SoundCardOutputValues::NODE_TYPE
    };
    let candidates = snapshot
        .child_ids_slice(parent)
        .iter()
        .copied()
        .filter(|child| {
            snapshot.node(*child).is_some_and(|node| {
                node.uuid == uuid || node.decl_id == direction
            })
        })
        .collect::<Vec<_>>();
    let retained = used
        .then(|| {
            candidates.iter().copied().find(|child| {
                snapshot
                    .node(*child)
                    .is_some_and(|node| node.node_type == expected_type)
            })
        })
        .flatten();
    for candidate in candidates {
        if Some(candidate) != retained {
            NodeHandle::new(candidate).remove(ctx);
        }
    }
    if !used || retained.is_some() {
        return;
    }
    let tree = if direction == "input" {
        let mut node = SoundCardInputValues::new();
        set_derived_identity(&mut node, namespace, "input-values", "Input", "input");
        NodeTree::new(node)
    } else {
        let mut node = SoundCardOutputValues::new();
        set_derived_identity(&mut node, namespace, "output-values", "Output", "output");
        NodeTree::new(node)
    };
    ctx.add_child_tree(parent, tree, None);
}

fn synchronize_input_channels(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    count: usize,
) -> Vec<NodeReference> {
    let Some(parent) = find_path(snapshot, module, "parameters/input/channels") else {
        return Vec::new();
    };
    let module_uuid = snapshot.node(module).expect("module exists").uuid;
    synchronize_channel_gains(ctx, snapshot, parent, module_uuid, count, "input")
}

fn synchronize_output_channels(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    count: usize,
) -> Vec<NodeReference> {
    let Some(parent) = find_path(snapshot, module, "parameters/output/channels") else {
        return Vec::new();
    };
    let module_uuid = snapshot.node(module).expect("module exists").uuid;
    synchronize_channel_gains(ctx, snapshot, parent, module_uuid, count, "output")
}

fn synchronize_channel_gains(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    namespace: NodeUuid,
    count: usize,
    direction: &str,
) -> Vec<NodeReference> {
    let existing = snapshot
        .child_ids(parent)
        .into_iter()
        .filter_map(|id| snapshot.node(id).map(|state| (id, state.uuid)))
        .collect::<Vec<_>>();
    let by_uuid = existing
        .iter()
        .map(|(id, uuid)| (*uuid, *id))
        .collect::<HashMap<_, _>>();
    let desired = (0..count)
        .map(|index| derived_uuid(namespace, format!("{direction}-channel-{}", index + 1).as_bytes()))
        .collect::<Vec<_>>();
    for (index, uuid) in desired.iter().enumerate() {
        let number = index + 1;
        let label = format!("{} {number}", title_case(direction));
        let decl_id = format!("{direction}_{number}");
        if let Some(existing) = by_uuid.get(uuid).copied() {
            if snapshot
                .node(existing)
                .is_some_and(|node| !matches!(node.param_value, Some(ParamValue::Float(_))))
            {
                let gain = find_child_by_key(snapshot, existing, "volume_db")
                    .and_then(|node| snapshot.node(node))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_float)
                    .unwrap_or(0.0);
                for child in snapshot.child_ids(existing) {
                    NodeHandle::new(child).remove(ctx);
                }
                let existing_label = snapshot
                    .node(existing)
                    .map(|node| node.label.as_str())
                    .unwrap_or(label.as_str());
                ctx.replace_node(
                    existing,
                    channel_parameter(*uuid, existing_label, decl_id.as_str(), gain, false),
                );
            }
        } else {
            ctx.add_child_tree(
                parent,
                NodeTree::new(channel_parameter(*uuid, label.as_str(), decl_id.as_str(), 0.0, false)),
                None,
            );
        }
    }
    let desired_set = desired.iter().copied().collect::<HashSet<_>>();
    for (id, uuid) in existing {
        if !desired_set.contains(&uuid) {
            NodeHandle::new(id).remove(ctx);
        }
    }
    desired
        .into_iter()
        .enumerate()
        .map(|(index, uuid)| {
            let id = by_uuid.get(&uuid).copied();
            let label = id
                .and_then(|id| snapshot.node(id))
                .map(|state| state.label.clone())
                .unwrap_or_else(|| format!("{} {}", title_case(direction), index + 1));
            let mut reference = NodeReference::with_cached_id(uuid, id);
            reference.set_cached_name(Some(label));
            reference
        })
        .collect()
}

fn synchronize_channel_values(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
    channels: &[NodeReference],
) {
    let Some(parent) = find_path(snapshot, module, path) else {
        return;
    };
    let desired = channels
        .iter()
        .map(|channel| (derived_uuid(channel.uuid, b"sound-card-channel-value"), channel))
        .collect::<HashMap<_, _>>();
    let existing = snapshot.child_ids(parent);
    let existing_by_uuid = existing
        .iter()
        .filter_map(|id| snapshot.node(*id).map(|node| (node.uuid, *id)))
        .collect::<HashMap<_, _>>();

    for (uuid, channel) in &desired {
        let label = channel.cached_name.as_deref().unwrap_or("Channel");
        if let Some(existing) = existing_by_uuid.get(uuid).copied() {
            let is_direct_level = snapshot
                .node(existing)
                .is_some_and(|node| matches!(node.param_value, Some(ParamValue::Float(_))));
            if !is_direct_level {
                let level = find_child_by_key(snapshot, existing, "volume")
                    .and_then(|node| snapshot.node(node))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_float)
                    .unwrap_or(0.0);
                for child in snapshot.child_ids(existing) {
                    NodeHandle::new(child).remove(ctx);
                }
                ctx.replace_node(
                    existing,
                    channel_parameter(
                        *uuid,
                        label,
                        format!("channel_{}", channel.uuid.0.simple()).as_str(),
                        level,
                        true,
                    ),
                );
            } else if snapshot.node(existing).is_some_and(|node| node.label != label) {
                ctx.patch_node_meta(
                    existing,
                    NodeMetaPatch {
                        label: Some(label.to_owned()),
                        ..NodeMetaPatch::default()
                    },
                );
            }
            continue;
        }
        ctx.add_child_tree(
            parent,
            NodeTree::new(channel_parameter(
                *uuid,
                label,
                format!("channel_{}", channel.uuid.0.simple()).as_str(),
                0.0,
                true,
            )),
            None,
        );
    }
    for id in existing {
        let Some(state) = snapshot.node(id) else {
            continue;
        };
        if !desired.contains_key(&state.uuid) {
            NodeHandle::new(id).remove(ctx);
        }
    }
}

fn channel_parameter(
    uuid: NodeUuid,
    label: &str,
    decl_id: &str,
    value: f64,
    read_only: bool,
) -> Parameter {
    let mut parameter = Parameter::new(
        label,
        ParamValue::Float(value),
        ParameterChangeCheck::ValueChange,
    );
    parameter.read_only = read_only;
    if !read_only {
        parameter.constraints.range = RangeConstraint::uniform(Some(-120.0), Some(24.0));
    }
    let data = parameter.node_data_mut();
    data.meta.uuid = uuid;
    data.meta.decl_id = DeclId(decl_id.to_owned());
    data.meta.short_name = decl_id.to_owned();
    data.meta.user_permissions = if read_only {
        NodeUserPermissions::none()
    } else {
        NodeUserPermissions {
            can_edit_name: true,
            ..NodeUserPermissions::none()
        }
    };
    data.meta.can_be_disabled = false;
    parameter
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

fn add_default_input_routes(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    channels: &[NodeReference],
    physical_channels: &[PhysicalChannelKey],
) {
    let Some(parent) = find_path(snapshot, module, "connection/input_routing/routes") else {
        return;
    };
    for (channel, physical_channel) in channels.iter().zip(physical_channels).take(2) {
        let physical_channel = physical_channel.as_str().to_owned();
        let mut route = SoundCardInputRoute::connected(physical_channel.clone(), channel.clone());
        set_route_identity(
            &mut route,
            snapshot.node(module).expect("module exists").uuid,
            "input",
            physical_channel.as_str(),
            channel.uuid,
        );
        ctx.add_child_tree(parent, NodeTree::new(route), None);
    }
}

fn add_default_output_routes(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    channels: &[NodeReference],
    physical_channels: &[PhysicalChannelKey],
) {
    let Some(parent) = find_path(snapshot, module, "connection/output_routing/routes") else {
        return;
    };
    for (channel, physical_channel) in channels.iter().zip(physical_channels).take(2) {
        let physical_channel = physical_channel.as_str().to_owned();
        let mut route = SoundCardOutputRoute::connected(channel.clone(), physical_channel.clone());
        set_route_identity(
            &mut route,
            snapshot.node(module).expect("module exists").uuid,
            "output",
            physical_channel.as_str(),
            channel.uuid,
        );
        ctx.add_child_tree(parent, NodeTree::new(route), None);
    }
}

fn remove_stale_routes(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
    channels: &[NodeReference],
) {
    let Some(parent) = find_path(snapshot, module, path) else {
        return;
    };
    let valid_channels = channels.iter().map(|channel| channel.uuid).collect::<HashSet<_>>();
    for route in snapshot.child_ids(parent) {
        let Some(channel_parameter) = find_child_by_key(snapshot, route, "channel") else {
            NodeHandle::new(route).remove(ctx);
            continue;
        };
        let Some(ParamValue::Reference(reference)) = snapshot
            .node(channel_parameter)
            .and_then(|node| node.param_value.as_ref())
        else {
            NodeHandle::new(route).remove(ctx);
            continue;
        };
        if !valid_channels.contains(&reference.uuid) {
            NodeHandle::new(route).remove(ctx);
        }
    }
}

fn clear_routes(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
) -> bool {
    let Some(parent) = find_path(snapshot, module, path) else {
        return false;
    };
    let routes = snapshot.child_ids(parent);
    for route in &routes {
        NodeHandle::new(*route).remove(ctx);
    }
    !routes.is_empty()
}

fn synchronize_optional_node(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    path: &str,
    enabled: bool,
    create: impl FnOnce() -> NodeTree,
) {
    let existing = find_path(snapshot, module, path);
    if enabled && existing.is_none() {
        if let Some(parent) = find_path(snapshot, module, "values") {
            ctx.add_child_tree(parent, create(), None);
        }
    } else if !enabled {
        if let Some(existing) = existing {
            NodeHandle::new(existing).remove(ctx);
        }
    }
}

fn synchronize_spectrum_bands(ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, module: NodeId) {
    let Some(parent) = find_path(snapshot, module, "values/spectral_analysis") else {
        return;
    };
    let namespace = snapshot.node(parent).expect("spectral values exist").uuid;
    let existing = snapshot
        .child_ids(parent)
        .into_iter()
        .filter_map(|id| snapshot.node(id).map(|node| node.uuid))
        .collect::<HashSet<_>>();
    for index in 0..DEFAULT_SPECTRUM_BANDS {
        let uuid = derived_uuid(namespace, format!("sound-card-spectrum-band-{index}").as_bytes());
        if existing.contains(&uuid) {
            continue;
        }
        let mut band = SoundCardSpectrumBand::for_index(index);
        let data = band.node_data_mut();
        data.meta.uuid = uuid;
        data.meta.label = format!("Band {}", index + 1);
        data.meta.decl_id = DeclId(format!("band_{}", index + 1));
        data.meta.short_name = data.meta.decl_id.0.clone();
        ctx.add_child_tree(parent, NodeTree::new(band), None);
    }
}

fn direction_channel_count(snapshot: &ProcessTreeSnapshot, module: NodeId, path: &str) -> usize {
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
    data.meta.uuid = derived_uuid(module_uuid, key.as_bytes());
    data.meta.label = format!(
        "{} -> {}",
        physical_channel,
        if direction == "input" { "Input" } else { "Output" }
    );
    data.meta.decl_id = DeclId(key.clone());
    data.meta.short_name = key;
}

pub(super) fn patch_channel_label(ctx: &mut ProcessCtx, channel: NodeId, label: String) {
    ctx.patch_node_meta(
        channel,
        NodeMetaPatch {
            label: Some(label),
            ..NodeMetaPatch::default()
        },
    );
}

pub(super) fn derived_uuid(namespace: NodeUuid, name: &[u8]) -> NodeUuid {
    NodeUuid(Uuid::new_v5(&namespace.0, name))
}
