use super::{
    scripts::http_request_from_script,
    transport::{weights, HttpQueuedRequest, HttpRequestOrigin},
};
use crate::app::{module::common::http::HttpCommandKind, module_command::CapturedInvocation};
use golden_core::engine::NodeWaker;
use golden_core::{events::CustomEvent, node::NodeId, process_ctx::ProcessTreeSnapshot};
use golden_core::{parameter::ParamValue, process_ctx::ProcessCtx, script::ScriptArguments};
use std::sync::Arc;

pub(super) const HTTP_INPUT_TOPIC: &str = "chataigne.http.request_input";

/// Original inputs survive native rejection and retirement without actor-side
/// destruction. Only the HTTP worker materializes a network request.
#[derive(Clone, Debug)]
pub(super) enum HttpInput {
    Script {
        method: &'static str,
        args: ScriptArguments,
    },
    Command {
        invocation: CapturedInvocation,
        kind: HttpCommandKind,
        source_weight: usize,
        source: NodeWaker,
        target: NodeWaker,
    },
    #[cfg(test)]
    Prepared {
        request: Arc<HttpQueuedRequest>,
        weight: usize,
    },
}

impl HttpInput {
    pub(super) fn capture_command(
        snapshot: &Arc<ProcessTreeSnapshot>,
        command: NodeId,
        kind: HttpCommandKind,
        execution: Option<CustomEvent>,
        waker: impl Fn(NodeId) -> Result<NodeWaker, String>,
    ) -> Result<Self, String> {
        if snapshot
            .node(command)
            .is_none_or(|node| node.node_type != kind.node_type())
        {
            return Err("HTTP command source type does not match its request kind".into());
        }
        let invocation = CapturedInvocation::capture(snapshot, command, execution, &["http_module"])?;
        let source_weight = command_weight(&invocation.snapshot, command, kind)?;
        let source = waker(command)?;
        let target = waker(invocation.module)?;
        Ok(Self::Command {
            invocation,
            kind,
            source_weight,
            source,
            target,
        })
    }

    pub(super) fn eligible_for_admission(&self) -> bool {
        match self {
            Self::Command { source, target, .. } => !source.is_closed() && !target.is_closed(),
            _ => true,
        }
    }

    pub(super) fn command_target(&self) -> Option<(NodeId, NodeId)> {
        match self {
            Self::Command { invocation, .. } => Some((invocation.module, invocation.command)),
            _ => None,
        }
    }
    pub(super) fn capture(ctx: &ProcessCtx, method: &str, args: &[ParamValue]) -> Option<Result<Self, String>> {
        let method = match method {
            "request" => "request",
            "requestJson" => "requestJson",
            "get" => "get",
            "head" => "head",
            "options" => "options",
            "delete" => "delete",
            "post" => "post",
            "put" => "put",
            "patch" => "patch",
            "postJson" => "postJson",
            "putJson" => "putJson",
            "patchJson" => "patchJson",
            "uploadFile" => "uploadFile",
            _ => return None,
        };
        Some(
            ctx.capture_script_args(args)
                .map(|args| Self::Script { method, args })
                .map_err(str::to_owned),
        )
    }

    #[cfg(test)]
    pub(super) fn prepare(request: HttpQueuedRequest) -> Result<Self, String> {
        let weight = weights::request_weight(&request)?;
        if weight > super::transport::HTTP_REQUEST_BYTES {
            return Err("HTTP request capacity is limited to 8 MiB retained storage".into());
        }
        // Native executor fixtures construct owned requests directly. Production
        // admits only script and snapshot leases and cannot enter this path.
        Ok(Self::Prepared {
            request: Arc::new(request),
            weight,
        })
    }

    pub(super) fn queue_weight(&self) -> usize {
        match self {
            Self::Script { args, .. } => std::mem::size_of::<Self>() + args.retained_bytes(),
            Self::Command {
                source_weight,
                invocation,
                ..
            } => std::mem::size_of::<Self>() + source_weight + invocation.override_weight,
            #[cfg(test)]
            Self::Prepared { weight, .. } => *weight,
        }
    }

    pub(super) fn description(&self) -> String {
        match self {
            Self::Script { method, .. } => format!("script {method}"),
            Self::Command { kind, .. } => match kind {
                HttpCommandKind::Request => "request command".into(),
                HttpCommandKind::UploadFile => "file upload command".into(),
            },
            #[cfg(test)]
            Self::Prepared { request, .. } => format!(
                "{} {}",
                request.payload.method.label(),
                super::diagnostic_url(request.payload.path.trim())
            ),
        }
    }

    pub(super) fn materialize(&self) -> Result<HttpQueuedRequest, String> {
        let request = match self {
            Self::Script { method, args } => HttpQueuedRequest {
                origin: HttpRequestOrigin::Script {
                    method: (*method).into(),
                },
                payload: http_request_from_script(method, args).expect("recognized HTTP method")?,
            },
            Self::Command { invocation, kind, .. } => {
                let view = invocation.view()?;
                command_weight(&view, invocation.command, *kind)?;
                HttpQueuedRequest {
                    origin: HttpRequestOrigin::Command {
                        command_id: invocation.command,
                        command_type: kind.node_type().into(),
                    },
                    payload: match kind {
                        HttpCommandKind::Request => {
                            crate::app::HttpRequestCommand::request_payload(&view, invocation.command)?
                        }
                        HttpCommandKind::UploadFile => {
                            crate::app::HttpUploadFileCommand::request_payload(&view, invocation.command)?
                        }
                    },
                }
            }
            #[cfg(test)]
            Self::Prepared { request, .. } => (**request).clone(),
        };
        let weight = weights::request_weight(&request)?;
        if weight > super::transport::HTTP_REQUEST_BYTES {
            return Err("HTTP converted request exceeds 8 MiB retained storage".into());
        }
        Ok(request)
    }
}

fn command_weight(snapshot: &ProcessTreeSnapshot, command: NodeId, kind: HttpCommandKind) -> Result<usize, String> {
    let mut weight = 0_usize;
    for path in kind.paths() {
        let value = crate::app::module_command::resolve_module_command_child(snapshot, command, path)
            .and_then(|id| snapshot.node(id))
            .and_then(|node| node.param_value.as_ref());
        if let Some(value) = value {
            if matches!(value, ParamValue::Reference(reference) if reference.relative_path_from_root().len() > 4096) {
                return Err("HTTP command reference exceeds 4096 path segments".into());
            }
            weight = weight.saturating_add(value.retained_bytes());
            if weight > 4 * 1024 * 1024 {
                return Err("HTTP command source exceeds 4 MiB retained storage".into());
            }
        }
    }
    Ok(weight)
}

#[cfg(test)]
mod tests;
