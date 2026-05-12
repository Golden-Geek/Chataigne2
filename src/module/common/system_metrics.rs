use golden_core::{
    edit::Edit,
    node::NodeId,
    parameter::{ParameterConstraints, RangeConstraint},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use std::time::{SystemTime, UNIX_EPOCH};

const BYTES_PER_MEGABYTE: f64 = 1024.0 * 1024.0;

pub(crate) fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / BYTES_PER_MEGABYTE
}

pub(crate) fn bytes_f64_to_mb(bytes: f64) -> f64 {
    bytes / BYTES_PER_MEGABYTE
}

pub(crate) fn percent_to_ratio(percent: f64) -> f64 {
    (percent / 100.0).clamp(0.0, 1.0)
}

pub(crate) fn process_cpu_percent_to_ratio(percent: f64, cpu_count: usize) -> f64 {
    let total_capacity_percent = 100.0 * cpu_count.max(1) as f64;
    (percent / total_capacity_percent).clamp(0.0, 1.0)
}

pub(crate) fn uptime_seconds_from_unix_start(process_start_time_secs: u64) -> f64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    now.saturating_sub(process_start_time_secs) as f64
}

pub(crate) fn float_constraints(min: Option<f64>, max: Option<f64>) -> ParameterConstraints {
    let max = match (min, max) {
        (Some(min), Some(max)) if max < min => Some(min),
        _ => max,
    };

    ParameterConstraints {
        range: RangeConstraint::uniform(min, max),
        ..Default::default()
    }
}

pub(crate) fn sync_float_constraints(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    min: Option<f64>,
    max: Option<f64>,
) {
    let expected = float_constraints(min, max);
    if snapshot.node(node_id).and_then(|node| node.param_constraints.as_ref()) == Some(&expected) {
        return;
    }

    ctx.edits.push(Edit::SetParamConstraints {
        node: node_id,
        constraints: expected,
    });
}