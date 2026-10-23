use super::*;
use golden_io::RetainedPayload;
use runtime::{Input, Source};
const OUTPUT_TOPIC: &str = "chataigne.joycon.captured-output";

impl JoyConModule {
    pub(super) fn queue(
        &mut self,
        ctx: &mut ProcessCtx,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
    ) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("JoyCon module is disabled".into());
        }
        self.ensure_runtime(ctx);
        let runtime = self.runtime.as_ref().ok_or("JoyCon runtime is unavailable")?;
        let permit = self
            .service
            .admission
            .try_reserve()
            .map_err(|_| "JoyCon module's 64 source lifetimes are occupied")?;
        let ingress = self
            .service
            .ingress
            .try_reserve()
            .map_err(|_| "Joy-Con local ingress is full")?;
        let source = runtime.capture(input, producer, time, permit, ingress)?;
        ctx.emit_custom_event(CustomEvent::local(OUTPUT_TOPIC, Some(self.id()), source));
        Ok(())
    }
    pub(super) fn capture_script(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &'static str,
        args: &[ParamValue],
    ) -> Result<(), String> {
        let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
        if args.len() > 5 || args.retained_bytes() > 64 * 1024 {
            return Err("JoyCon scripts accept five arguments / 64 KiB".into());
        }
        let producer = ctx.node_waker().map_err(|error| error.to_string())?;
        self.queue(ctx, Input::Script { method, args }, producer, ctx.time)
    }
    pub(super) fn accept_output(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic != OUTPUT_TOPIC || event.origin != Some(self.id()) {
            return;
        }
        let Some(source) = event.local_payload::<RetainedPayload<Source>>() else {
            return;
        };
        source.arrive();
        if !self.node_data().effective_enabled || !source.live() {
            self.set_runtime_warning(ctx, "JoyCon captured source retired; request was not sent");
            return;
        }
        let at = self
            .service
            .outgoing
            .partition_point(|pending| pending.time <= source.time);
        self.service.outgoing.insert(at, source.clone());
        self.request_service();
    }
    pub(super) fn submit_outputs(&mut self, ctx: &mut ProcessCtx) {
        // A deferred command mutation may still own an older source than this queue's prefix.
        if self.service.ingress.metrics().active != 0 {
            return;
        }
        if self
            .service
            .output_retry_at
            .is_some_and(|at| std::time::Instant::now() < at)
        {
            return;
        }
        self.service.output_retry_at = None;
        for _ in 0..8 {
            let Some(source) = self.service.outgoing.pop_front() else {
                break;
            };
            if !source.live() {
                self.set_runtime_warning(ctx, "JoyCon source retired; request was not sent");
                continue;
            }
            let Some(runtime) = &self.runtime else {
                self.service.outgoing.push_front(source);
                return;
            };
            match runtime.submit(source) {
                Ok(receipt) => self.service.pending.push_back(receipt),
                Err(source) => {
                    self.service.outgoing.push_front(source);
                    self.service.output_retry_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(250));
                    return;
                }
            }
        }
        if !self.service.outgoing.is_empty() {
            self.request_service();
        }
    }
}
pub(crate) fn dispatch(
    ctx: &mut ProcessCtx,
    invocation: crate::app::module_command::CapturedInvocation,
) -> Result<(), String> {
    let producer = ctx.node_waker().map_err(|error| error.to_string())?;
    let time = ctx.current_event().map_or(ctx.time, |event| event.time);
    ctx.call_node_mutation_without_snapshot(invocation.module, move |node, ctx| {
        let module = node
            .as_any_mut()
            .downcast_mut::<JoyConModule>()
            .ok_or("JoyCon command target is unavailable")?;
        if let Err(error) = module.queue(ctx, Input::Command(invocation), producer, time) {
            module.set_runtime_warning(ctx, &error);
            golden_core::logerror!(error);
        }
        Ok(())
    });
    Ok(())
}
