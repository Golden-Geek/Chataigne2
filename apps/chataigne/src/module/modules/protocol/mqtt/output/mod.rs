use crate::app::{module::common::mqtt::MqttPublishRequest, module_command::CapturedInvocation};
use golden_core::{engine::NodeWaker, parameter::ParamValue, process_ctx::ProcessCtx, script::ScriptArguments};

mod command;
pub(super) mod script;
#[cfg(test)]
mod tests;

pub(super) const MAX_PACKET: usize = 1024 * 1024;
pub(super) const MAX_TOPIC: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub(super) enum Method {
    Publish,
    Text,
    Json,
}
impl Method {
    fn parse(method: &str) -> Option<Self> {
        Some(match method {
            "publish" => Self::Publish,
            "publishText" => Self::Text,
            "publishJson" => Self::Json,
            _ => return None,
        })
    }
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Text => "publishText",
            Self::Json => "publishJson",
        }
    }
}

enum Source {
    Command(CapturedInvocation),
    Script { method: Method, args: ScriptArguments },
}

pub(super) struct Request {
    source: Source,
    producer: NodeWaker,
    payload_bound: usize,
    topic_bound: usize,
}
impl Request {
    pub(super) fn command(invocation: CapturedInvocation, producer: NodeWaker) -> Self {
        let bound = |name, limit| {
            invocation
                .snapshot
                .find_child_by_decl_id(invocation.command, name)
                .and_then(|id| invocation.snapshot.node(id))
                .and_then(|node| node.param_value.as_ref())
                .map_or(0, script::text_bound)
                .max(invocation.override_weight)
                .min(limit)
        };
        let (payload_bound, topic_bound) = (bound("payload", MAX_PACKET), bound("topic", MAX_TOPIC));
        Self {
            source: Source::Command(invocation),
            producer,
            payload_bound,
            topic_bound,
        }
    }
    pub(super) fn script(ctx: &ProcessCtx, method: &str, args: &[ParamValue]) -> Option<Result<Self, String>> {
        let method = Method::parse(method)?;
        Some((|| {
            let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
            Self::script_arguments(method, args, ctx.node_waker().map_err(|error| error.to_string())?)
        })())
    }
    pub(super) fn script_arguments(method: Method, args: ScriptArguments, producer: NodeWaker) -> Result<Self, String> {
        if args.is_empty() || args.len() > 4 || args.retained_bytes() > 4 * 1024 * 1024 {
            return Err("MQTT script input requires 1-4 arguments / at most 4 MiB retained storage".into());
        }
        let topic_bound = script::text_bound(&args[0]).min(MAX_TOPIC);
        let payload_bound = match method {
            Method::Json => args.retained_bytes().saturating_mul(6).saturating_add(4096),
            _ => args.get(1).map_or(0, script::text_bound),
        }
        .min(MAX_PACKET);
        Ok(Self {
            source: Source::Script { method, args },
            producer,
            payload_bound,
            topic_bound,
        })
    }
    pub(super) fn description(&self) -> &'static str {
        match self.source {
            Source::Command(_) => "publish",
            Source::Script { method, .. } => method.name(),
        }
    }
    pub(super) fn reservation_weight(&self) -> usize {
        // Input arguments/snapshot allocations keep their canonical Core leases.
        // This reserves prepared bytes plus the library copy through disposition.
        let scratch = match &self.source {
            Source::Script {
                method: Method::Json,
                args,
            } => 2 * args.retained_bytes(),
            Source::Command(invocation) => 3 * invocation.override_weight,
            _ => 0,
        };
        size_of::<Self>()
            + 2 * (size_of::<MqttPublishRequest>() + self.payload_bound + self.topic_bound + 256)
            + scratch
            + 4096
    }
    pub(super) fn is_live(&self) -> bool {
        !self.producer.is_closed()
    }
    pub(super) fn prepare(&self) -> Result<MqttPublishRequest, String> {
        if !self.is_live() {
            return Err("MQTT source retired before dispatch; request was not sent".into());
        }
        let request = match &self.source {
            Source::Command(invocation) => command::prepare(invocation, self.payload_bound)?,
            Source::Script { method, args } => script::prepare(*method, args, self.payload_bound)?,
        };
        if request.payload.capacity() > self.payload_bound || request.topic.capacity() > self.topic_bound {
            return Err("MQTT prepared output exceeds its admitted bound; request was not sent".into());
        }
        Ok(request)
    }
}

pub(crate) fn dispatch(ctx: &mut ProcessCtx, invocation: CapturedInvocation) -> Result<(), String> {
    let module = invocation.module;
    let request = Request::command(invocation, ctx.node_waker().map_err(|error| error.to_string())?);
    ctx.call_node_mutation_without_snapshot(module, move |node, ctx| {
        let module = node
            .as_any_mut()
            .downcast_mut::<super::MqttModule>()
            .ok_or("MQTT command target is unavailable")?;
        if let Err(error) = module.queue_output_request(ctx, request) {
            golden_core::logerror!(format!("MQTT command was not admitted: {error}"));
        }
        Ok(())
    });
    Ok(())
}
