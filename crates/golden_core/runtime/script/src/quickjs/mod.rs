use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use golden_model::{DeclId, NodeId};
use golden_parameters::{
    CssUnit, CssValue, FileConstraints, FileTypeGroup, ParamValue, ParameterConstraintPolicy, ParameterConstraints,
    ParameterEnumOption, ParameterUiHints, RangeConstraint,
};
use rquickjs::context::EvalOptions as QuickJsEvalOptions;
use rquickjs::function::{Args as QuickJsArgs, Func as QuickJsFunc, MutFn as QuickJsMutFn};
use rquickjs::{
    Context as QuickJsContext, Ctx as QuickJsCtx, Error as QuickJsError, Function as QuickJsFunction, IntoJs as _,
    Object as QuickJsObject, Runtime as QuickJsRuntimeHandle, Value as QuickJsValue,
};
use serde_json::Value as JsonValue;

use crate::*;

const SCRIPT_HOST_CALL_BUDGET_MESSAGE: &str = "script host-call budget exceeded in current callback";
const SCRIPT_MAX_STACK_BYTES: usize = 512 * 1024;
const SCRIPT_MAX_HOST_LABEL_BYTES: usize = 1_024;
const SCRIPT_MAX_HOST_MESSAGE_BYTES: usize = 64 * 1_024;
const SCRIPT_MAX_HOST_JSON_BYTES: usize = 1_024 * 1_024;

enum ScriptHostOp {
    Log {
        level: ScriptLogLevel,
        message: String,
    },
    EmitCustom {
        topic: String,
        payload: JsonValue,
    },
    SetNodeScriptProperty {
        node: NodeId,
        property: String,
        value: ParamValue,
    },
    CallNodeScriptMethod {
        node: NodeId,
        method: String,
        args: Vec<ParamValue>,
    },
    SetEventListener {
        target: NodeId,
        level: u32,
    },
    RemoveEventListener {
        target: NodeId,
    },
    ClearEventListeners,
}

#[derive(Default)]
struct QuickJsTreeBridgeState {
    snapshot: Option<Arc<dyn ScriptTreeView>>,
    host: Option<NodeId>,
    script: Option<NodeId>,
    time_seconds: f64,
    delta_seconds: f64,
}

#[derive(Default)]
struct QuickJsEntrypoints {
    init: Option<String>,
    update: Option<String>,
    event: Option<String>,
    param_changed: Option<String>,
    destroy: Option<String>,
    exports: Vec<String>,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScriptInterruptReason {
    None = 0,
    Cancelled = 1,
    Deadline = 2,
    InstructionBudget = 3,
}

impl ScriptInterruptReason {
    fn from_raw(value: u8) -> Self {
        match value {
            1 => Self::Cancelled,
            2 => Self::Deadline,
            3 => Self::InstructionBudget,
            _ => Self::None,
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Cancelled => "was cancelled",
            Self::Deadline => "exceeded its monotonic wall-time deadline",
            Self::InstructionBudget => "exceeded its VM instruction target",
        }
    }
}

struct ScriptInterruptState {
    clock_origin: Instant,
    active_depth: AtomicU32,
    deadline_ns: AtomicU64,
    instruction_budget: AtomicU64,
    interrupt_checks: AtomicU64,
    cancelled: AtomicBool,
    reason: AtomicU8,
    poisoned: AtomicBool,
}

impl ScriptInterruptState {
    fn new() -> Self {
        Self {
            clock_origin: Instant::now(),
            active_depth: AtomicU32::new(0),
            deadline_ns: AtomicU64::new(0),
            instruction_budget: AtomicU64::new(0),
            interrupt_checks: AtomicU64::new(0),
            cancelled: AtomicBool::new(false),
            reason: AtomicU8::new(ScriptInterruptReason::None as u8),
            poisoned: AtomicBool::new(false),
        }
    }

    fn now_ns(&self) -> u64 {
        u64::try_from(self.clock_origin.elapsed().as_nanos()).unwrap_or(u64::MAX)
    }

    fn begin(self: &Arc<Self>, wall_time: Duration, instruction_budget: u64) -> ScriptInvocationGuard {
        let previous_deadline = self.deadline_ns.load(Ordering::Acquire);
        let previous_instruction_budget = self.instruction_budget.load(Ordering::Acquire);
        let depth = self.active_depth.fetch_add(1, Ordering::AcqRel);
        let proposed_deadline = self
            .now_ns()
            .saturating_add(u64::try_from(wall_time.as_nanos()).unwrap_or(u64::MAX))
            .max(1);
        let proposed_instruction_budget = instruction_budget.max(1);

        if depth == 0 {
            self.cancelled.store(false, Ordering::Release);
            self.reason.store(ScriptInterruptReason::None as u8, Ordering::Release);
            self.interrupt_checks.store(0, Ordering::Release);
            self.deadline_ns.store(proposed_deadline, Ordering::Release);
            self.instruction_budget
                .store(proposed_instruction_budget, Ordering::Release);
        } else {
            self.deadline_ns
                .store(previous_deadline.min(proposed_deadline), Ordering::Release);
            self.instruction_budget.store(
                previous_instruction_budget.min(proposed_instruction_budget),
                Ordering::Release,
            );
        }

        ScriptInvocationGuard {
            state: Arc::clone(self),
            previous_deadline,
            previous_instruction_budget,
        }
    }

    fn request_cancel(&self) {
        if self.active_depth.load(Ordering::Acquire) > 0 {
            self.cancelled.store(true, Ordering::Release);
        }
    }

    fn record_reason(&self, reason: ScriptInterruptReason) -> bool {
        let _ = self.reason.compare_exchange(
            ScriptInterruptReason::None as u8,
            reason as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        true
    }

    fn should_interrupt(&self) -> bool {
        if self.active_depth.load(Ordering::Acquire) == 0 {
            return false;
        }
        if self.cancelled.load(Ordering::Acquire) {
            return self.record_reason(ScriptInterruptReason::Cancelled);
        }

        let deadline = self.deadline_ns.load(Ordering::Acquire);
        if deadline != 0 && self.now_ns() >= deadline {
            return self.record_reason(ScriptInterruptReason::Deadline);
        }

        let checks = self.interrupt_checks.fetch_add(1, Ordering::Relaxed) + 1;
        let budget = self.instruction_budget.load(Ordering::Acquire);
        if budget != 0 && checks > budget {
            return self.record_reason(ScriptInterruptReason::InstructionBudget);
        }
        false
    }

    fn interrupt_reason(&self) -> ScriptInterruptReason {
        ScriptInterruptReason::from_raw(self.reason.load(Ordering::Acquire))
    }
}

struct ScriptInvocationGuard {
    state: Arc<ScriptInterruptState>,
    previous_deadline: u64,
    previous_instruction_budget: u64,
}

impl Drop for ScriptInvocationGuard {
    fn drop(&mut self) {
        let previous_depth = self.state.active_depth.fetch_sub(1, Ordering::AcqRel);
        if previous_depth <= 1 {
            self.state.deadline_ns.store(0, Ordering::Release);
            self.state.instruction_budget.store(0, Ordering::Release);
            self.state.cancelled.store(false, Ordering::Release);
        } else {
            self.state.deadline_ns.store(self.previous_deadline, Ordering::Release);
            self.state
                .instruction_budget
                .store(self.previous_instruction_budget, Ordering::Release);
        }
    }
}

/// Thread-safe cancellation handle for the currently active JavaScript entry.
#[derive(Clone)]
pub struct ScriptCancellationHandle {
    state: Arc<ScriptInterruptState>,
}

impl ScriptCancellationHandle {
    /// Requests interruption of the currently active entry. Calling this while idle is a no-op.
    pub fn cancel(&self) {
        self.state.request_cancel();
    }
}

/// QuickJS-backed script runtime.
pub struct QuickJsRuntime {
    runtime: QuickJsRuntimeHandle,
    context: QuickJsContext,
    budgets: ScriptBudgets,
    entrypoints: QuickJsEntrypoints,
    manifest: Option<ScriptManifest>,
    host_ops: Arc<Mutex<Vec<ScriptHostOp>>>,
    host_call_counter: Arc<AtomicU32>,
    tree_bridge_state: Arc<Mutex<QuickJsTreeBridgeState>>,
    interrupt_state: Arc<ScriptInterruptState>,
}

impl From<QuickJsError> for ScriptRuntimeError {
    fn from(value: QuickJsError) -> Self {
        Self::QuickJs(value.to_string())
    }
}

mod core;
mod host_api;
mod invocation;
mod manifest;
mod runtime;

#[cfg(test)]
mod tests;

use manifest::{default_param_value, parameter_default_from_json_value, parse_manifest_from_json};
