//! Frozen outgoing intent; traversal, payload construction, encoding and DNS run on the worker.
use super::{
    osc_message::{encode_packet, OscValuePayload},
    osc_runtime::admission::payload_weight,
};
use crate::app::module_command::CapturedInvocation;
use golden_core::{
    engine::NodeWaker, node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot, script::ScriptArguments,
};
use golden_io::RetainedPayload;
use std::sync::Arc;

#[cfg(test)]
mod tests;

pub(crate) const CAPTURE_WEIGHT: usize = 1024 * 1024;
const MAX_TARGETS: usize = 256;
const MAX_EDGES: usize = 16_384;
const MAX_PLAN_BYTES: usize = 256 * 1024;
const PREPARATION_TURN: usize = 64;

#[derive(Debug)]
pub(crate) enum QueueError {
    Full,
    Closed,
    Invalid(String),
}
impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_str("OSC outgoing capacity exhausted; command was not admitted"),
            Self::Closed => f.write_str("OSC transport is not accepting commands"),
            Self::Invalid(error) => error.fmt(f),
        }
    }
}

#[derive(Debug)]
enum Source {
    Command(CapturedInvocation),
    Script {
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
        args: ScriptArguments,
        module: NodeId,
    },
    Values {
        snapshot: RetainedPayload<ProcessTreeSnapshot>,
        module: NodeId,
        root: NodeId,
        target: NodeId,
    },
}

#[derive(Debug)]
pub(crate) struct OutboundRequest {
    source: Source,
    producer: NodeWaker,
    log: bool,
}
impl OutboundRequest {
    pub(crate) fn command(invocation: CapturedInvocation, producer: NodeWaker) -> Self {
        let log = log_enabled(&invocation.snapshot, invocation.module);
        Self {
            source: Source::Command(invocation),
            producer,
            log,
        }
    }
    pub(crate) fn script(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        args: ScriptArguments,
        producer: NodeWaker,
    ) -> Result<Self, QueueError> {
        if args.len() > 1025 || args.retained_bytes() > 128 * 1024 {
            return Err(QueueError::Invalid(
                "OSC script source exceeds 1025 arguments / 128 KiB".into(),
            ));
        }
        let Some(ParamValue::Str(address) | ParamValue::File(address) | ParamValue::Enum(address)) = args.first()
        else {
            return Err(QueueError::Invalid("OSC send expects an address string".into()));
        };
        validate_address(address).map_err(QueueError::Invalid)?;
        Ok(Self {
            log: log_enabled(snapshot, module),
            source: Source::Script {
                snapshot: snapshot.try_lease().map_err(|_| QueueError::Full)?,
                args,
                module,
            },
            producer,
        })
    }
    pub(crate) fn values(
        snapshot: &Arc<ProcessTreeSnapshot>,
        module: NodeId,
        root: NodeId,
        target: NodeId,
        producer: NodeWaker,
        log: bool,
    ) -> Result<Self, QueueError> {
        Ok(Self {
            source: Source::Values {
                snapshot: snapshot.try_lease().map_err(|_| QueueError::Full)?,
                module,
                root,
                target,
            },
            producer,
            log,
        })
    }
    pub(crate) fn description(&self) -> String {
        match &self.source {
            Source::Command(invocation) => format!("command {:?}", invocation.command),
            Source::Script { .. } => "script send".into(),
            Source::Values { target, .. } => format!("value {target:?}"),
        }
    }
    fn view(&self) -> (&ProcessTreeSnapshot, NodeId) {
        match &self.source {
            Source::Command(invocation) => (&invocation.snapshot, invocation.module),
            Source::Script { snapshot, module, .. } | Source::Values { snapshot, module, .. } => (snapshot, *module),
        }
    }
    pub(crate) fn is_live(&self) -> bool {
        !self.producer.is_closed()
    }
    fn packet(&self) -> Result<(String, OscValuePayload), String> {
        match &self.source {
            Source::Command(invocation) => {
                // Only parameter values are needed. A snapshot overlay would
                // clone unrelated node metadata outside this request's quota.
                let execution = invocation
                    .execution
                    .as_ref()
                    .map(|payload| {
                        serde_json::from_value::<crate::app::module_command::ModuleCommandExecuteEvent>(
                            (**payload).clone(),
                        )
                        .map_err(|error| format!("Invalid OSC invocation: {error}"))
                    })
                    .transpose()?;
                if execution
                    .as_ref()
                    .is_some_and(|event| event.command_id != invocation.command)
                {
                    return Err("OSC invocation targets another command".into());
                }
                let view = &invocation.snapshot;
                let value = |id| {
                    let original = view
                        .node(id)
                        .and_then(|node| node.param_value.as_ref())
                        .ok_or_else(|| "OSC argument is not a parameter".to_string())?;
                    Ok::<_, String>(
                        execution
                            .as_ref()
                            .and_then(|event| event.param_overrides.iter().rev().find(|value| value.param_id == id))
                            .map(|value| &value.value)
                            .unwrap_or(original),
                    )
                };
                let address_id =
                    crate::app::module_command::resolve_module_command_child(view, invocation.command, "address")
                        .ok_or("missing OSC command address")?;
                let address = match value(address_id)? {
                    ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => text,
                    _ => return Err("OSC address must be text".into()),
                };
                validate_address(address)?;
                let root =
                    crate::app::module_command::resolve_module_command_child(view, invocation.command, "arguments")
                        .ok_or("missing OSC arguments folder")?;
                let ids = view.child_ids_slice(root);
                if ids.len() > 1024 {
                    return Err("OSC command exceeds 1024 arguments".into());
                }
                let values = ids.iter().map(|id| value(*id)).collect::<Result<Vec<_>, String>>()?;
                preflight_values(address, &values)?;
                Ok((
                    address.into(),
                    OscValuePayload::Arguments(values.into_iter().cloned().collect()),
                ))
            }
            Source::Script { args, .. } => {
                let address = match args.first() {
                    Some(ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value)) => value,
                    _ => return Err("OSC address is unavailable".into()),
                };
                let values = args.iter().skip(1).collect::<Vec<_>>();
                preflight_values(address, &values)?;
                Ok((
                    address.clone(),
                    OscValuePayload::Arguments(values.into_iter().cloned().collect()),
                ))
            }
            Source::Values {
                snapshot, root, target, ..
            } => value_packet(snapshot, *root, *target),
        }
    }
}

fn log_enabled(snapshot: &ProcessTreeSnapshot, module: NodeId) -> bool {
    snapshot
        .find_child(module, "connection")
        .and_then(|root| snapshot.find_child(root, "log_outgoing"))
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .is_some_and(|value| matches!(value, ParamValue::Bool(true)))
}

fn validate_address(address: &str) -> Result<(), String> {
    if !address.starts_with('/') || address.len() > 4096 || address.contains('\0') {
        return Err("OSC address must start with '/' and fit 4096 bytes".into());
    }
    Ok(())
}
fn string(snapshot: &ProcessTreeSnapshot, id: NodeId) -> Result<&str, String> {
    match snapshot.node(id).and_then(|node| node.param_value.as_ref()) {
        Some(ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text)) => Ok(text),
        _ => Err("OSC text parameter is unavailable".into()),
    }
}
fn preflight_values(address: &str, values: &[&ParamValue]) -> Result<(), String> {
    let mut weight = address
        .len()
        .saturating_add(values.len() * std::mem::size_of::<ParamValue>());
    if values.len() > 1024 {
        return Err("OSC values exceed 1024 items".into());
    }
    for value in values {
        if let ParamValue::Reference(reference) = value {
            if reference.relative_path_from_root.len() > 64 {
                return Err("OSC reference path exceeds 64 segments".into());
            }
        }
        weight = weight.saturating_add(value.retained_bytes());
        if weight > 128 * 1024 {
            return Err("OSC source values exceed 128 KiB; no output was sent".into());
        }
    }
    Ok(())
}

fn value_packet(
    snapshot: &ProcessTreeSnapshot,
    root: NodeId,
    changed: NodeId,
) -> Result<(String, OscValuePayload), String> {
    let target = feedback_target(snapshot, changed)?;
    let mut parts = Vec::new();
    if target == root {
        return Err("OSC feedback has no address below values".into());
    }
    let mut cursor = target;
    let mut bytes = 1usize;
    while cursor != root {
        if parts.len() >= 64 {
            return Err("OSC feedback path exceeds 64 levels".into());
        }
        let node = snapshot.node(cursor).ok_or("OSC feedback path is unavailable")?;
        if node.label.len() > 4096 {
            return Err("OSC feedback label exceeds 4096 bytes".into());
        }
        let text = node.label.trim().trim_matches('/');
        bytes = bytes.saturating_add(text.len() + 1);
        if bytes > 4096 {
            return Err("OSC feedback address exceeds 4096 bytes".into());
        }
        parts.push(text);
        cursor = node.parent.ok_or("OSC feedback node is outside values")?;
    }
    parts.reverse();
    let address = format!(
        "/{}",
        parts
            .into_iter()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    );
    let node = snapshot.node(target).ok_or("OSC value was removed")?;
    if let Some(value) = &node.param_value {
        preflight_values(&address, &[value])?;
        return Ok((address, OscValuePayload::Single(value.clone())));
    }
    let children = snapshot.child_ids_slice(target);
    if children.len() > 1024 {
        return Err("OSC multi-value folder exceeds 1024 children".into());
    }
    let mut values = children
        .iter()
        .filter_map(|id| {
            let child = snapshot.node(*id)?;
            let index = child.label.strip_prefix("value ")?.parse::<usize>().ok()?;
            (index > 0).then_some((index, child.param_value.as_ref()?))
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|(index, _)| *index);
    let refs = values.iter().map(|(_, value)| *value).collect::<Vec<_>>();
    preflight_values(&address, &refs)?;
    if refs.is_empty() {
        return Err("OSC multi-value folder has no arguments".into());
    }
    Ok((address, OscValuePayload::Multi(refs.into_iter().cloned().collect())))
}

/// Canonicalize tuple feedback before latest-state coalescing. Only the changed
/// node and its direct parent are inspected; payload construction stays off-actor.
pub(super) fn feedback_target(snapshot: &ProcessTreeSnapshot, changed: NodeId) -> Result<NodeId, String> {
    let node = snapshot.node(changed).ok_or("OSC value was removed")?;
    if node.param_value.is_some()
        && node
            .label
            .strip_prefix("value ")
            .and_then(|suffix| suffix.parse::<usize>().ok())
            .is_some_and(|index| index > 0)
    {
        if let Some(parent) = node
            .parent
            .filter(|id| snapshot.node(*id).is_some_and(|node| node.node_type == "folder"))
        {
            return Ok(parent);
        }
    }
    Ok(changed)
}

#[derive(Debug)]
pub(crate) struct Destination {
    pub host: String,
    pub port: u16,
}
#[derive(Debug)]
enum Walk {
    Visit(NodeId),
    Advance(NodeId),
    Done,
}

pub(crate) struct Preparation {
    request: OutboundRequest,
    walk: Walk,
    root: NodeId,
    edges: usize,
    targets: Vec<Destination>,
    target_bytes: usize,
}
pub(crate) struct PreparedSend {
    pub request: OutboundRequest,
    pub bytes: Vec<u8>,
    pub targets: Vec<Destination>,
    pub next: usize,
    address: String,
}
impl Preparation {
    pub(crate) fn new(request: OutboundRequest) -> Result<Self, String> {
        let (snapshot, module) = request.view();
        let root = snapshot
            .find_child(module, "connection")
            .and_then(|id| snapshot.find_child(id, "outputs"))
            .ok_or("OSC outputs folder is unavailable")?;
        if !snapshot.node(root).is_some_and(|node| node.enabled) {
            return Err("OSC outputs are disabled".into());
        }
        let walk = snapshot
            .node(root)
            .and_then(|node| node.first_child)
            .map_or(Walk::Done, Walk::Visit);
        Ok(Self {
            request,
            walk,
            root,
            edges: 0,
            targets: Vec::with_capacity(MAX_TARGETS),
            target_bytes: MAX_TARGETS * std::mem::size_of::<Destination>(),
        })
    }
    pub(crate) fn service(&mut self) -> Result<bool, String> {
        if !self.request.is_live() {
            return Err("OSC source lifetime retired before send".into());
        }
        let (snapshot, _) = self.request.view();
        for _ in 0..PREPARATION_TURN {
            self.edges += 1;
            if self.edges > MAX_EDGES {
                return Err("OSC destinations exceed 16384 traversal edges".into());
            }
            match self.walk {
                Walk::Done => return Ok(true),
                Walk::Visit(id) => {
                    let node = snapshot.node(id).ok_or("OSC destination disappeared")?;
                    self.walk = if node.node_type == "folder" && node.enabled {
                        node.first_child.map_or(Walk::Advance(id), Walk::Visit)
                    } else {
                        Walk::Advance(id)
                    };
                    if node.node_type == "osc_output" && node.enabled {
                        let host_id = snapshot
                            .find_child(id, "remote_host")
                            .ok_or("OSC destination has no host")?;
                        let host = string(snapshot, host_id)?;
                        if host.len() > 1024 {
                            return Err("OSC destination host exceeds 1024 bytes".into());
                        }
                        if host.trim().is_empty() {
                            continue;
                        }
                        let port = snapshot
                            .find_child(id, "remote_port")
                            .and_then(|id| snapshot.node(id))
                            .and_then(|node| node.param_value.as_ref())
                            .and_then(ParamValue::as_int)
                            .and_then(|value| u16::try_from(value).ok())
                            .ok_or("OSC destination port is invalid")?;
                        self.target_bytes = self.target_bytes.saturating_add(host.len());
                        if self.targets.len() == MAX_TARGETS || self.target_bytes > MAX_PLAN_BYTES {
                            return Err("OSC destination plan exceeds 256 targets / 256 KiB".into());
                        }
                        self.targets.push(Destination {
                            host: host.into(),
                            port,
                        });
                    }
                }
                Walk::Advance(id) => {
                    let node = snapshot.node(id).ok_or("OSC destination traversal is unavailable")?;
                    self.walk = if let Some(next) = node.next_sibling {
                        Walk::Visit(next)
                    } else if let Some(parent) = node.parent.filter(|parent| *parent != self.root) {
                        Walk::Advance(parent)
                    } else {
                        Walk::Done
                    };
                }
            }
        }
        Ok(matches!(self.walk, Walk::Done))
    }
    pub(crate) fn finish(self) -> Result<PreparedSend, String> {
        if self.targets.is_empty() {
            return Err("no enabled OSC outputs are configured; no output was sent".into());
        }
        let (address, payload) = self.request.packet()?;
        validate_address(&address)?;
        payload_weight(&address, &payload)?;
        let bytes = rosc::encoder::encode(&encode_packet(&address, &payload)?)
            .map_err(|error| format!("OSC encoding failed: {error}"))?;
        if bytes.len() > 65_507 {
            return Err("OSC encoded output exceeds 65507 bytes; native send was not attempted".into());
        }
        Ok(PreparedSend {
            request: self.request,
            bytes,
            targets: self.targets,
            next: 0,
            address,
        })
    }
}
impl PreparedSend {
    pub(crate) fn log(&self, target: &Destination) -> Option<String> {
        self.request.log.then(|| {
            format!(
                "Sent OSC {} -> {}:{} ({} bytes)",
                self.address.chars().take(256).collect::<String>(),
                target.host,
                target.port,
                self.bytes.len()
            )
        })
    }
}

pub(crate) fn dispatch(
    ctx: &mut golden_core::process_ctx::ProcessCtx,
    invocation: CapturedInvocation,
) -> Result<(), String> {
    let module = invocation.module;
    let request = OutboundRequest::command(invocation, ctx.node_waker().map_err(|error| error.to_string())?);
    ctx.call_node_mutation(module, move |node, _| {
        let Some(module) = node.as_any_mut().downcast_mut::<crate::app::GenericOscModule>() else {
            return Err("OSC command target is unavailable".into());
        };
        if let Err(error) = module.queue_captured_osc(request) {
            golden_core::logerror!(format!("OSC command was not admitted: {error}"));
        }
        Ok(())
    });
    Ok(())
}
