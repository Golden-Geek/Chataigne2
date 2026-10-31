use serde::Serialize;

pub const CYCLES: u32 = 100;
pub const BASELINE_CYCLE: u32 = 10;
pub const SPACING_MS: u64 = 2000;
pub const EDITS_PER_CYCLE: u32 = 10;
pub const EDIT_INTERVAL_MS: u64 = 10;
pub const DRAIN_SECONDS: u64 = 10;

/// Delay follows completed lifecycle work; slow native operations never consume this pause.
pub fn post_cycle_settle(cycle: u32, final_cycle: u32, spacing_ms: u64) -> std::time::Duration {
    let spacing = std::time::Duration::from_millis(spacing_ms);
    if cycle == BASELINE_CYCLE || cycle == final_cycle {
        spacing.max(std::time::Duration::from_secs(DRAIN_SECONDS))
    } else {
        spacing
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct StableCounters {
    pub handles: u32,
    pub private_bytes: u64,
    pub working_set_bytes: u64,
    pub lifetime_peak_working_set_bytes: u64,
    pub reserved_texture_bytes: u64,
    pub active_plan_bytes: u64,
    pub retiring_generations: usize,
    pub prepared_generations: usize,
    pub input_bindings: usize,
}

#[derive(Debug, Serialize)]
pub struct Verdict {
    pub handle_delta: i64,
    pub private_growth_bytes: i128,
    pub logical_live_set_restored: bool,
    pub observed_within_target: bool,
    pub eligible: bool,
}

pub fn evaluate(
    baseline: StableCounters,
    final_state: StableCounters,
    maximum_private_bytes: u64,
    cycles: u32,
    spacing_ms: u64,
    edit_count: u32,
    superseded: u64,
) -> Verdict {
    let handle_delta = i64::from(final_state.handles) - i64::from(baseline.handles);
    let private_growth_bytes =
        i128::from(maximum_private_bytes.max(final_state.private_bytes)) - i128::from(baseline.private_bytes);
    let logical_live_set_restored = baseline.reserved_texture_bytes == final_state.reserved_texture_bytes
        && baseline.active_plan_bytes == final_state.active_plan_bytes
        && baseline.active_plan_bytes > 0
        && baseline.reserved_texture_bytes >= baseline.active_plan_bytes
        && baseline.retiring_generations == 0
        && baseline.prepared_generations == 0
        && final_state.retiring_generations == 0
        && final_state.prepared_generations == 0
        && baseline.input_bindings == 2
        && final_state.input_bindings == 2;
    Verdict {
        handle_delta,
        private_growth_bytes,
        logical_live_set_restored,
        observed_within_target: handle_delta <= 10
            && private_growth_bytes <= 32 * 1024 * 1024
            && logical_live_set_restored,
        eligible: cycles == CYCLES
            && spacing_ms >= SPACING_MS
            && edit_count == CYCLES * EDITS_PER_CYCLE
            && superseded > 0,
    }
}
