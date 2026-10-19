use super::{
    bounds::MAX_SOURCE_BYTES, streaming_script_send_request, validate_values, value_weight, StreamingSendFrameKind,
    StreamingSendRequest,
};
use golden_core::{parameter::ParamValue, process_ctx::ProcessCtx};
use golden_io::{PayloadPool, RetainedPayload};
use std::{
    mem::size_of,
    sync::{Arc, LazyLock},
};

#[derive(Clone, Copy, Debug)]
enum Method {
    Text,
    Bytes,
    Hex,
}

/// Canonical input storage is accounted separately from each native queue's
/// conservative output reservation. No argument strings are cloned on the actor.
#[derive(Clone, Debug)]
pub(crate) struct ScriptSend {
    method: Method,
    args: RetainedPayload<Vec<ParamValue>>,
}
impl ScriptSend {
    pub fn capture(ctx: &ProcessCtx, method: &str, args: &[ParamValue]) -> Option<Result<Self, String>> {
        let method = match method {
            "sendText" | "sendString" => Method::Text,
            "sendBytes" | "sendData" => Method::Bytes,
            "sendHex" | "sendHexString" => Method::Hex,
            _ => return None,
        };
        Some((|| {
            let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
            Self::retain(method, args)
        })())
    }
    fn retain(method: Method, args: Arc<Vec<ParamValue>>) -> Result<Self, String> {
        validate_values(&args)?;
        let weight = (size_of::<Vec<ParamValue>>() + 64)
            .saturating_add(args.capacity().saturating_mul(size_of::<ParamValue>()))
            .saturating_add(
                args.iter()
                    .map(|value| value_weight(value).saturating_sub(size_of::<ParamValue>()))
                    .sum::<usize>(),
            );
        if weight > MAX_SOURCE_BYTES {
            return Err("Stream script arguments exceed 4 MiB retained storage".into());
        }
        static POOL: LazyLock<Result<PayloadPool, std::io::Error>> =
            LazyLock::new(|| PayloadPool::new("stream-script-input", 64, 16 * 1024 * 1024));
        let args = POOL
            .as_ref()
            .map_err(|error| format!("Stream script input pool unavailable: {error}"))?
            .try_retain_arc(args, weight)
            .map_err(|_| "Stream script input lifetime capacity exhausted; command was not admitted".to_string())?;
        Ok(Self { method, args })
    }
    pub fn frame_kind(&self) -> StreamingSendFrameKind {
        match self.method {
            Method::Text | Method::Hex => StreamingSendFrameKind::Text,
            Method::Bytes => StreamingSendFrameKind::Binary,
        }
    }
    pub fn materialize(&self) -> Result<StreamingSendRequest, String> {
        let method = match self.method {
            Method::Text => "sendText",
            Method::Bytes => "sendBytes",
            Method::Hex => "sendHex",
        };
        streaming_script_send_request(method, &self.args).expect("recognized native stream method")
    }
    /// Upper bound without conversion: escapes/hex only shrink source text;
    /// byte-list encoding cannot produce more bytes than its string source.
    pub fn output_bound(&self) -> usize {
        fn text_bound(value: &ParamValue) -> usize {
            match value {
                ParamValue::Str(text) | ParamValue::File(text) | ParamValue::Enum(text) => text.len(),
                ParamValue::Reference(reference) => reference.cached_name().map_or(36, str::len),
                ParamValue::Trigger() => 7,
                ParamValue::Int(_) => 12,
                ParamValue::Bool(_) => 5,
                // Even four f64 decimal expansions and CSS units fit in 4096 bytes.
                ParamValue::Float(_)
                | ParamValue::CssValue(_)
                | ParamValue::Vec2(..)
                | ParamValue::Vec3(..)
                | ParamValue::Color(..) => 4096,
            }
        }
        match self.method {
            Method::Text => self.args.first().map_or(0, text_bound).saturating_add(2),
            Method::Hex => self.args.first().map_or(0, text_bound) / 2,
            Method::Bytes => self.args.iter().map(text_bound).fold(0_usize, usize::saturating_add),
        }
    }
}

#[cfg(test)]
mod tests;
