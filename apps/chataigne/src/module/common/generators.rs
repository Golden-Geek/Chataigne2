//! Admission policy shared by the app's timed generator workers.

use std::{num::NonZeroUsize, sync::OnceLock};

use golden_io::{PendingSendError, RetirementPool, WorkerLimits};

#[cfg(test)]
mod tests;

pub(crate) const MAX_ITEMS: usize = 4096;
pub(crate) const MAX_LABEL_BYTES: usize = 1024;
pub(crate) const MAX_CURVE_KEYS: usize = 16384;
pub(crate) const MAX_CONFIG_WEIGHT: usize = 32 * 1024 * 1024;
pub(crate) const EVENT_ROWS: usize = 64;

// A live or cancelling worker keeps its slot through final cleanup. Repeated
// replacement therefore cannot create unbounded detached generator threads.
pub(crate) fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(64).unwrap()))
}

pub(crate) fn limits() -> WorkerLimits {
    WorkerLimits {
        command_items: 16,
        command_weight: MAX_CONFIG_WEIGHT,
        event_items: 8,
        event_weight: 1024 * 1024,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    Full,
    Closed,
    Invalid(&'static str),
}

impl std::fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => f.write_str("generator command capacity exhausted"),
            Self::Closed => f.write_str("generator worker is no longer running"),
            Self::Invalid(reason) => f.write_str(reason),
        }
    }
}

pub(crate) fn admission_error<T>(error: PendingSendError<T>) -> AdmissionError {
    match error {
        PendingSendError::Full(_) => AdmissionError::Full,
        PendingSendError::Disconnected(_) => AdmissionError::Closed,
    }
}

pub(crate) fn check_items(count: usize) -> Result<(), AdmissionError> {
    if count > MAX_ITEMS {
        return Err(AdmissionError::Invalid("generator item limit is 4096"));
    }
    Ok(())
}

pub(crate) fn check_text(text: &str) -> Result<(), AdmissionError> {
    if text.len() > MAX_LABEL_BYTES {
        return Err(AdmissionError::Invalid(
            "generator label/declaration limit is 1024 bytes",
        ));
    }
    Ok(())
}

/// Checks borrowed metadata before constructing a configuration or cloning curve
/// script strings. Curve structure has its own aggregate bound, independent of the
/// number of generator items. The runtime repeats validation at its public boundary.
pub(crate) fn preflight(
    snapshot: &golden_core::process_ctx::ProcessTreeSnapshot,
    list: golden_core::node::NodeId,
    curves: bool,
) -> Result<(), AdmissionError> {
    use golden_core::parameter::ParamValue;

    let items = snapshot.child_ids_slice(list);
    check_items(items.len())?;
    let mut keys = 0usize;
    let mut weight = items.len().saturating_mul(1024);
    for &id in items {
        let Some(item) = snapshot.node(id) else { continue };
        check_text(&item.label)?;
        weight = weight.saturating_add(item.label.len());
        if !curves {
            continue;
        }
        let Some(curve) = snapshot.find_child(id, "curve") else {
            continue;
        };
        let children = snapshot.child_ids_slice(curve);
        keys = keys.saturating_add(children.len());
        if keys > MAX_CURVE_KEYS {
            return Err(AdmissionError::Invalid("generator curve child limit is 16384"));
        }
        weight = weight.saturating_add(children.len().saturating_mul(1024));
        for &key in children {
            let Some(easing) = snapshot.find_child(key, "easing") else {
                continue;
            };
            let Some(source) = snapshot.find_child(easing, "script_source") else {
                continue;
            };
            if let Some(ParamValue::Str(source)) = snapshot.node(source).and_then(|node| node.param_value.as_ref()) {
                weight = weight.saturating_add(source.len().saturating_mul(2));
            }
        }
    }
    if weight > MAX_CONFIG_WEIGHT {
        return Err(AdmissionError::Invalid(
            "generator configuration exceeds 32 MiB retained-weight limit",
        ));
    }
    Ok(())
}

pub(crate) trait EventSink<E> {
    /// False means publication was cancelled; the worker must stop servicing time.
    fn publish(&self, event: E) -> bool;
}

impl<E, F: Fn(E) -> bool> EventSink<E> for F {
    fn publish(&self, event: E) -> bool {
        self(event)
    }
}
