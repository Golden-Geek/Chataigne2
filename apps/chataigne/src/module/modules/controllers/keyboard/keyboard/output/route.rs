use super::*;
use crate::app::module_command::CapturedInvocation;
const TOPIC: &str = "chataigne.keyboard.captured-output";

impl KeyboardModule {
    fn queue_keyboard(
        &mut self,
        ctx: &mut ProcessCtx,
        input: Input,
        producer: golden_core::engine::NodeWaker,
        time: golden_core::engine::EngineTime,
    ) -> Result<(), String> {
        self.ensure_output(ctx)?;
        let permit = self
            .output
            .admission
            .try_reserve()
            .map_err(|_| "Keyboard module's 64 source lifetimes are occupied")?;
        let ingress = self
            .output
            .ingress
            .try_reserve()
            .map_err(|_| "Keyboard local ingress is full")?;
        let source = self
            .output
            .runtime
            .as_ref()
            .unwrap()
            .capture(input, producer, time, permit, ingress)?;
        ctx.emit_custom_event(CustomEvent::local(TOPIC, Some(self.id()), source));
        Ok(())
    }
    pub(in super::super) fn capture_keyboard_script(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &'static str,
        args: &[ParamValue],
    ) -> Result<(), String> {
        let args = ctx.capture_script_args(args).map_err(str::to_owned)?;
        if args.len() > 1 || args.retained_bytes() > 64 * 1024 {
            return Err("Keyboard scripts accept one argument / 64 KiB".into());
        }
        let producer = ctx.node_waker().map_err(|error| error.to_string())?;
        self.queue_keyboard(ctx, Input::Script { method, args }, producer, ctx.time)
    }
    pub(in super::super) fn accept_keyboard_output(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic != TOPIC || event.origin != Some(self.id()) {
            return;
        }
        let Some(source) = event.local_payload::<RetainedPayload<Source>>() else {
            return;
        };
        source.arrive();
        if !source.live() || !self.node_data().effective_enabled || !self.send_enabled.get() {
            self.set_backend_warning(
                ctx,
                KEYBOARD_OUTPUT_WARNING_ID,
                "Keyboard captured source retired; request was not sent",
            );
            return;
        }
        let at = self
            .output
            .outgoing
            .partition_point(|pending| pending.time <= source.time);
        self.output.outgoing.insert(at, source.clone());
        self.request_output_service();
    }
    pub(super) fn submit_keyboard_outputs(&mut self) {
        if self.output.ingress.metrics().active != 0 || self.output.submit_at.is_some_and(|at| Instant::now() < at) {
            return;
        }
        self.output.submit_at = None;
        for _ in 0..8 {
            let Some(source) = self.output.outgoing.pop_front() else {
                break;
            };
            if !source.live() {
                golden_core::logerror!("Keyboard source retired; request was not sent");
                continue;
            }
            let Some(runtime) = &self.output.runtime else {
                self.output.outgoing.push_front(source);
                return;
            };
            match runtime.submit(source) {
                Ok(receipt) => self.output.receipts.push_back(receipt),
                Err(source) => {
                    self.output.outgoing.push_front(source);
                    self.output.submit_at = Some(Instant::now() + Duration::from_millis(250));
                    break;
                }
            }
        }
        if !self.output.outgoing.is_empty() && self.output.submit_at.is_none() {
            self.request_output_service();
        }
    }
}
pub(crate) fn dispatch(ctx: &mut ProcessCtx, invocation: CapturedInvocation) -> Result<(), String> {
    let producer = ctx.node_waker().map_err(|error| error.to_string())?;
    let time = ctx.current_event().map_or(ctx.time, |event| event.time);
    ctx.call_node_mutation_without_snapshot(invocation.module, move |node, ctx| {
        let module = node
            .as_any_mut()
            .downcast_mut::<KeyboardModule>()
            .ok_or("Keyboard command target unavailable")?;
        if let Err(error) = module.queue_keyboard(ctx, Input::Command(invocation), producer, time) {
            module.set_backend_warning(ctx, KEYBOARD_OUTPUT_WARNING_ID, &error);
            golden_core::logerror!(error);
        }
        Ok(())
    });
    Ok(())
}
