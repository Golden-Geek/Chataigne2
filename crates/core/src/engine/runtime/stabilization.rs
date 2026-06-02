use super::*;

impl<T: Node> Engine<T> {
    pub(super) fn run_stabilization_rounds(&mut self) -> Result<(), EngineRuntimeError> {
        let mut pass = 0usize;
        let mut control_ms_total = 0u128;
        let mut dispatch_ms_total = 0u128;
        let mut apply_edits_ms_total = 0u128;

        loop {
            self.absorb_external_edits()?;
            let control_started = Instant::now();
            self.run_control_pass()?;
            control_ms_total =
                control_ms_total.saturating_add(control_started.elapsed().as_millis());

            if self.inbox.events.is_empty() && self.edits.pending.is_empty() {
                break;
            }

            #[cfg(debug_assertions)]
            self.log_verbose_stabilization_trace(pass, "post-control");

            if pass == STABILIZATION_WARN_PASSES {
                eprintln!(
                    "[engine] stabilization warning: {} passes at tick {} — possible feedback cycle",
                    pass, self.time.tick
                );
            }

            if pass >= self.runtime_limits.max_stabilization_passes_per_tick {
                #[cfg(debug_assertions)]
                self.log_verbose_stabilization_trace(pass, "pass-limit-reached");
                return Err(EngineRuntimeError::InfiniteEventEditCycle {
                    tick: self.time.tick,
                    passes: pass,
                });
            }

            self.time.micro = (pass as u32).saturating_add(1);
            self.time.seq = 0;

            if !self.inbox.events.is_empty() {
                let dispatch_started = Instant::now();
                self.dispatch_inbox(ExecutionPhase::EndOfTickStabilization)?;
                dispatch_ms_total =
                    dispatch_ms_total.saturating_add(dispatch_started.elapsed().as_millis());
            }

            if !self.edits.pending.is_empty() {
                let apply_started = Instant::now();
                self.apply_edits_without_history()?;
                self.resolve_if_needed()?;
                apply_edits_ms_total =
                    apply_edits_ms_total.saturating_add(apply_started.elapsed().as_millis());
            }

            pass += 1;
        }

        let stabilization_total_ms = control_ms_total + dispatch_ms_total + apply_edits_ms_total;
        if stabilization_total_ms >= PERF_LOG_TICK_THRESHOLD_MS {
            eprintln!(
                "[engine] stabilization passes={} control_ms={} dispatch_ms={} apply_edits_ms={}",
                pass, control_ms_total, dispatch_ms_total, apply_edits_ms_total
            );
        }

        Ok(())
    }

    pub(super) fn run_control_pass(&mut self) -> Result<(), EngineRuntimeError> {
        self.evaluate_parameter_controls();
        if !self.edits.pending.is_empty() {
            self.apply_edits_without_history()?;
            self.resolve_if_needed()?;
        }
        Ok(())
    }
}
