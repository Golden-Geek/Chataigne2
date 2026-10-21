use super::*;

impl MidiModule {
    pub(super) fn apply_time_code_quarter_frame(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
        raw_value: u8,
        received_at: Duration,
    ) -> MidiApplyResult {
        self.mtc_state.apply(raw_value);
        let was_active = self.mtc_stream_active;
        self.last_mtc_quarter_frame_at = Some(received_at);
        self.mtc_stream_active = true;

        if !self.rate.is_bound() || !self.mtc_playing.is_bound() || !self.time.is_bound() || !self.timecode.is_bound() {
            return MidiApplyResult::Retry;
        }

        if !was_active {
            self.set_internal_param(ctx, self.mtc_playing.id(), ParamValue::Bool(true));
        }
        self.set_internal_param(
            ctx,
            self.rate.id(),
            ParamValue::Str(mtc_rate_label(self.mtc_state.rate_code()).to_string()),
        );
        self.set_internal_param(ctx, self.time.id(), ParamValue::Float(self.mtc_state.time_seconds()));
        self.set_internal_param(
            ctx,
            self.timecode.id(),
            ParamValue::Str(self.mtc_state.timecode_string()),
        );
        MidiApplyResult::Applied
    }

    pub(super) fn apply_midi_clock_tick(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        received_at: Duration,
        clock_timing: Option<MidiClockTiming>,
    ) -> MidiApplyResult {
        let (beat_triggered, values_changed) = if let Some(clock_timing) = clock_timing {
            let mut values_changed = false;
            if !self.midi_clock_state.running {
                self.midi_clock_state.running = true;
                values_changed = true;
            }

            self.midi_clock_state.last_tick_at = Some(received_at);

            if let Some(next_bpm) = clock_timing.bpm {
                let previous_units = self.quantized_midi_clock_bpm_units(self.midi_clock_state.bpm);
                let next_units = self.quantized_midi_clock_bpm_units(next_bpm);
                self.midi_clock_state.bpm = next_bpm;
                if next_units != previous_units {
                    values_changed = true;
                }
            }

            (clock_timing.beat_triggered, values_changed)
        } else {
            self.record_midi_clock_tick_fallback(received_at)
        };

        let mut result = MidiApplyResult::Applied;

        if beat_triggered {
            if !self.beat.is_bound() {
                return MidiApplyResult::Retry;
            }
            self.set_internal_param(ctx, self.beat.id(), ParamValue::Trigger());
        }

        if values_changed {
            result = merge_apply_results(result, self.apply_midi_clock_values(ctx, snapshot));
        }

        result
    }

    pub(super) fn apply_midi_clock_transport_event(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        decl_id: &str,
        running: bool,
        reset_ticks: bool,
    ) -> MidiApplyResult {
        self.midi_clock_state.tick_in_beat = 0;
        self.midi_clock_state.last_tick_at = None;
        self.midi_clock_state.last_beat_at = None;
        self.midi_clock_state.recent_beat_durations.clear();
        self.midi_clock_state.running = running;

        if reset_ticks || matches!(decl_id, START_DECL_ID) {
            self.midi_clock_state.bpm = 0.0;
        }

        let trigger_id = match decl_id {
            START_DECL_ID if self.start.is_bound() => Some(self.start.id()),
            CONTINUE_DECL_ID if self.r#continue.is_bound() => Some(self.r#continue.id()),
            STOP_DECL_ID if self.stop.is_bound() => Some(self.stop.id()),
            RESET_DECL_ID if self.reset.is_bound() => Some(self.reset.id()),
            _ => None,
        };
        let Some(trigger_id) = trigger_id else {
            return MidiApplyResult::Retry;
        };

        merge_apply_results(
            {
                self.set_internal_param(ctx, trigger_id, ParamValue::Trigger());
                MidiApplyResult::Applied
            },
            self.apply_midi_clock_values(ctx, snapshot),
        )
    }

    pub(super) fn apply_midi_clock_values(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
    ) -> MidiApplyResult {
        if !self.playing.is_bound() || !self.bpm.is_bound() {
            return MidiApplyResult::Retry;
        }

        self.set_internal_param(ctx, self.playing.id(), ParamValue::Bool(self.midi_clock_state.running));
        self.set_internal_param(
            ctx,
            self.bpm.id(),
            ParamValue::Float(self.quantized_midi_clock_bpm(self.midi_clock_state.bpm)),
        );
        MidiApplyResult::Applied
    }

    pub(super) fn midi_clock_bpm_precision_digits(&self) -> u32 {
        if !self.clock_bpm_precision.is_bound() {
            return DEFAULT_MIDI_CLOCK_BPM_PRECISION as u32;
        }

        self.clock_bpm_precision.get().clamp(0, 6) as u32
    }

    pub(super) fn quantized_midi_clock_bpm(&self, bpm: f64) -> f64 {
        quantize_decimal(bpm, self.midi_clock_bpm_precision_digits())
    }

    pub(super) fn quantized_midi_clock_bpm_units(&self, bpm: f64) -> i64 {
        quantize_decimal_units(bpm, self.midi_clock_bpm_precision_digits())
    }

    pub(super) fn record_midi_clock_tick_fallback(&mut self, now: Duration) -> (bool, bool) {
        let mut values_changed = false;
        if !self.midi_clock_state.running {
            self.midi_clock_state.running = true;
            values_changed = true;
        }

        self.midi_clock_state.last_tick_at = Some(now);
        self.midi_clock_state.tick_in_beat = self.midi_clock_state.tick_in_beat.saturating_add(1);
        if self.midi_clock_state.tick_in_beat < MIDI_CLOCK_PULSES_PER_BEAT {
            return (false, values_changed);
        }

        self.midi_clock_state.tick_in_beat = 0;

        if let Some(last_beat_at) = self.midi_clock_state.last_beat_at {
            let beat_duration = now.saturating_sub(last_beat_at);
            if !beat_duration.is_zero() {
                self.midi_clock_state.recent_beat_durations.push_back(beat_duration);
                while self.midi_clock_state.recent_beat_durations.len() > MIDI_CLOCK_BPM_WINDOW_BEATS {
                    self.midi_clock_state.recent_beat_durations.pop_front();
                }

                if let Some(next_bpm) = stable_midi_clock_bpm(&self.midi_clock_state.recent_beat_durations) {
                    let previous_units = self.quantized_midi_clock_bpm_units(self.midi_clock_state.bpm);
                    let next_units = self.quantized_midi_clock_bpm_units(next_bpm);
                    self.midi_clock_state.bpm = next_bpm;
                    if next_units != previous_units {
                        values_changed = true;
                    }
                }
            }
        }

        self.midi_clock_state.last_beat_at = Some(now);

        (true, values_changed)
    }

    pub(super) fn refresh_transport_activity(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let now = self.midi_receive_now(ctx);

        if self.midi_clock_state.running
            && self.midi_clock_state.last_tick_at.is_some_and(|last_tick_at| {
                now.saturating_sub(last_tick_at).as_secs_f64() > MIDI_CLOCK_PLAYING_TIMEOUT_SECS
            })
        {
            self.midi_clock_state.running = false;
            let _ = self.apply_midi_clock_values(ctx, snapshot);
        }

        if self.mtc_stream_active
            && self
                .last_mtc_quarter_frame_at
                .is_some_and(|last_mtc_at| now.saturating_sub(last_mtc_at).as_secs_f64() > MTC_PLAYING_TIMEOUT_SECS)
        {
            self.mtc_stream_active = false;
            if self.mtc_playing.is_bound() {
                self.set_internal_param(ctx, self.mtc_playing.id(), ParamValue::Bool(false));
            }
        }
    }

    pub(super) fn transport_activity_timeout_due(&self, now: Duration) -> bool {
        (self.midi_clock_state.running
            && self.midi_clock_state.last_tick_at.is_some_and(|last_tick_at| {
                now.saturating_sub(last_tick_at).as_secs_f64() > MIDI_CLOCK_PLAYING_TIMEOUT_SECS
            }))
            || (self.mtc_stream_active
                && self.last_mtc_quarter_frame_at.is_some_and(|last_mtc_at| {
                    now.saturating_sub(last_mtc_at).as_secs_f64() > MTC_PLAYING_TIMEOUT_SECS
                }))
    }

    pub(super) fn midi_receive_now(&self, ctx: &ProcessCtx) -> Duration {
        ctx.runtime_elapsed
    }
}
