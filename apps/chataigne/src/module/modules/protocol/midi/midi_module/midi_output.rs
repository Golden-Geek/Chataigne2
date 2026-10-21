use super::*;
use crate::app::module_command::CapturedInvocation;
use golden_core::{engine::NodeWaker, script::ScriptArguments};
use std::time::Instant;

mod script;

pub(super) const CAPTURE_WEIGHT: usize = 1024 * 1024;
enum Source {
    Command(CapturedInvocation),
    Script {
        method: script::Method,
        args: ScriptArguments,
    },
    Feedback(Vec<MidiMessage>),
    #[cfg(test)]
    Prepared(MidiSendRequest),
}
pub(crate) struct MidiOutputRequest {
    source: Source,
    producer: Option<NodeWaker>,
    pub(super) captured_at: Instant,
    pub(super) log: bool,
}
impl MidiOutputRequest {
    pub(super) fn command(invocation: CapturedInvocation, producer: NodeWaker, captured_at: Instant) -> Self {
        Self {
            source: Source::Command(invocation),
            producer: Some(producer),
            captured_at,
            log: false,
        }
    }
    pub(super) fn script(
        ctx: &ProcessCtx,
        method: &str,
        args: &[ParamValue],
        log: bool,
    ) -> Option<Result<Self, String>> {
        let method = script::Method::parse(method)?;
        Some((|| {
            let args = ctx.capture_script_args(args).map_err(str::to_string)?;
            if args.len() > 4096 || args.retained_bytes() > 256 * 1024 {
                return Err("MIDI script source exceeds 4096 arguments / 256 KiB".into());
            }
            Ok(Self {
                source: Source::Script { method, args },
                producer: Some(ctx.node_waker().map_err(|e| e.to_string())?),
                captured_at: Instant::now(),
                log,
            })
        })())
    }
    pub(super) fn feedback(messages: Vec<MidiMessage>, producer: NodeWaker, log: bool) -> Result<Self, String> {
        if messages.capacity() > 2
            || messages
                .iter()
                .any(|message| matches!(message, MidiMessage::System(MidiSystemMessage::Sysex { .. })))
        {
            return Err("MIDI feedback must contain at most two fixed-size messages".into());
        }
        Ok(Self {
            source: Source::Feedback(messages),
            producer: Some(producer),
            captured_at: Instant::now(),
            log,
        })
    }
    pub(super) fn reservation_weight(&self) -> usize {
        match &self.source {
            Source::Command(_) | Source::Script { .. } => CAPTURE_WEIGHT,
            Source::Feedback(_) => 8192,
            #[cfg(test)]
            Source::Prepared(request) => {
                4096 + request
                    .packets
                    .iter()
                    .map(|p| p.bytes.capacity() + p.description.capacity())
                    .sum::<usize>()
            }
        }
    }
    pub(super) fn is_live(&self) -> bool {
        self.producer.as_ref().is_none_or(|waker| !waker.is_closed())
    }
    pub(super) fn prepare(self) -> Result<PreparedOutput, String> {
        if !self.is_live() {
            return Err("MIDI source retired before preparation; request was not sent".into());
        }
        let request = match self.source {
            Source::Command(invocation) => crate::app::prepare_midi_command(&invocation)?,
            Source::Script { method, args } => script::prepare(method, &args)?,
            Source::Feedback(messages) => MidiSendRequest::immediate_many(messages, "auto feedback".into()),
            #[cfg(test)]
            Source::Prepared(request) => request,
        };
        Ok(PreparedOutput {
            request,
            producer: self.producer,
            captured_at: self.captured_at,
            log: self.log,
        })
    }
    #[cfg(test)]
    pub(super) fn for_test(request: MidiSendRequest) -> Self {
        Self {
            source: Source::Prepared(request),
            producer: None,
            captured_at: Instant::now(),
            log: false,
        }
    }
    #[cfg(test)]
    pub(super) fn for_test_source(request: MidiSendRequest, producer: NodeWaker) -> Self {
        let mut output = Self::for_test(request);
        output.producer = Some(producer);
        output
    }
}
pub(super) struct PreparedOutput {
    pub request: MidiSendRequest,
    pub producer: Option<NodeWaker>,
    pub captured_at: Instant,
    pub log: bool,
}
