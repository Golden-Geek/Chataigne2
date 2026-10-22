use super::*;
use std::sync::{atomic::AtomicBool, Mutex};

struct State {
    started: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<Outcome>, String>>>,
    wake: WorkerNotifier,
}
impl State {
    fn finish(&self, result: Result<RetainedPayload<Outcome>, String>) {
        *self.result.lock().unwrap_or_else(|poison| poison.into_inner()) = Some(result);
        self.complete.store(true, Ordering::Release);
        self.wake.notify();
    }
}
pub(in super::super) struct Receipt(Arc<State>);
impl Receipt {
    pub(in super::super) fn complete(&self) -> bool {
        self.0.complete.load(Ordering::Acquire)
    }
    pub(in super::super) fn take(&self) -> Option<Result<RetainedPayload<Outcome>, String>> {
        self.0.result.lock().unwrap_or_else(|poison| poison.into_inner()).take()
    }
}
pub(super) struct Work {
    pub(super) source: RetainedPayload<Source>,
    state: Arc<State>,
    admitted: bool,
}
impl Work {
    pub(super) fn new(source: RetainedPayload<Source>, wake: WorkerNotifier) -> (Self, Receipt) {
        let state = Arc::new(State {
            started: AtomicBool::new(false),
            complete: AtomicBool::new(false),
            result: Mutex::new(None),
            wake,
        });
        (
            Self {
                source,
                state: state.clone(),
                admitted: true,
            },
            Receipt(state),
        )
    }
    pub(super) fn unadmitted(mut self) {
        self.admitted = false;
    }
    pub(super) fn run<P: Provider>(self, provider: &mut P, pool: &PayloadPool) {
        let result = execute(provider, self.source.clone(), pool, &self.state.started);
        self.state.finish(result);
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        if self.admitted && !self.state.complete.load(Ordering::Acquire) {
            self.state.finish(Err(if self.state.started.load(Ordering::Acquire) {
                "OS native worker stopped during execution; execution is unconfirmed"
            } else {
                "OS native worker stopped or request cancelled; request was not sent"
            }
            .into()));
        }
    }
}
pub(in super::super) struct Outcome {
    pub(in super::super) source: RetainedPayload<Source>,
    pub(in super::super) attempted: bool,
    pub(in super::super) error: Option<String>,
    pub(in super::super) callback_error: Option<String>,
    pub(in super::super) requested: Option<NativeCallbackPayload>,
    pub(in super::super) failed: Option<NativeCallbackPayload>,
}
fn execute<P: Provider>(
    provider: &mut P,
    source: RetainedPayload<Source>,
    pool: &PayloadPool,
    started: &AtomicBool,
) -> Result<RetainedPayload<Outcome>, String> {
    execute_with(provider, source, pool, started, NativeCallbackPayload::prepare)
}
pub(super) fn execute_with<P: Provider>(
    provider: &mut P,
    source: RetainedPayload<Source>,
    pool: &PayloadPool,
    started: &AtomicBool,
    mut callback: impl FnMut(&str, Vec<serde_json::Value>) -> Result<NativeCallbackPayload, String>,
) -> Result<RetainedPayload<Outcome>, String> {
    let reservation = pool
        .try_reserve(WEIGHT)
        .map_err(|_| "OS outcome capacity exhausted; request was not sent")?;
    let request = source.prepare();
    let (attempted, result) = match &request {
        Ok(request) if source.live() => {
            started.store(true, Ordering::Release);
            (true, provider.execute(request))
        }
        Ok(_) => (
            false,
            Err("OS source retired before native dispatch; request was not sent".into()),
        ),
        Err(error) => (false, Err(error.clone())),
    };
    let error = result.err().map(diagnostic);
    let mut callback_error = None;
    // Callback admission is observational. It must never erase a known native
    // result or turn an already performed action into something retryable.
    let mut prepare_callback = |name: &str, args| match callback(name, args) {
        Ok(callback) => Some(callback),
        Err(error) => {
            callback_error = Some(diagnostic(format!("OS callback was not admitted: {error}")));
            None
        }
    };
    let requested = if attempted && (matches!(request, Ok(Request::Control(_))) || error.is_none()) {
        prepare_callback(
            super::super::SYSTEM_COMMAND_REQUESTED_CALLBACK,
            vec![
                serde_json::json!(source.method.name()),
                request.as_ref().unwrap().details(),
            ],
        )
    } else {
        None
    };
    let failed = error.as_ref().and_then(|error| {
        prepare_callback(
            super::super::SYSTEM_COMMAND_FAILED_CALLBACK,
            vec![serde_json::json!(source.method.name()), serde_json::json!(error)],
        )
    });
    reservation
        .retain(
            Outcome {
                source,
                attempted,
                error,
                callback_error,
                requested,
                failed,
            },
            WEIGHT,
        )
        .map_err(|_| "OS outcome reservation failed".into())
}
