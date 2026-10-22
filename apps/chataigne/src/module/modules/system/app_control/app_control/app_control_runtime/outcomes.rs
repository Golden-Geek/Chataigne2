use super::*;
struct State {
    started: AtomicBool,
    complete: AtomicBool,
    result: Mutex<Option<Result<RetainedPayload<Outcome>, String>>>,
    wake: WorkerNotifier,
}
pub(in super::super) struct Receipt(Arc<State>);
impl Receipt {
    pub(in super::super) fn complete(&self) -> bool {
        self.0.complete.load(Ordering::Acquire)
    }
    pub(in super::super) fn take(&self) -> Option<Result<RetainedPayload<Outcome>, String>> {
        self.0.result.lock().unwrap_or_else(|error| error.into_inner()).take()
    }
}
pub(super) struct Work {
    source: RetainedPayload<Source>,
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
    fn finish(&self, result: Result<RetainedPayload<Outcome>, String>) {
        *self.state.result.lock().unwrap_or_else(|error| error.into_inner()) = Some(result);
        self.state.complete.store(true, Ordering::Release);
        self.state.wake.notify();
    }
    pub(super) fn run<P: Provider>(self, provider: &mut P, pool: &PayloadPool) {
        let result = (|| {
            let reservation = pool
                .try_reserve(COMMAND_WEIGHT)
                .map_err(|_| "App Control outcome capacity exhausted; request was not sent")?;
            let request = self.source.prepare();
            let result = match request {
                Ok(request) if self.source.live() => {
                    self.state.started.store(true, Ordering::Release);
                    provider.execute(&request)
                }
                Ok(_) => Err("App Control source retired; request was not sent".into()),
                Err(error) => Err(error),
            };
            let attempted = result
                .as_ref()
                .map_or(self.state.started.load(Ordering::Acquire), |value| value.attempted);
            let error = result.as_ref().err().cloned().map(diagnostic);
            let method = result.as_ref().map_or(self.source.method(), |value| value.method);
            let mut callback_error = None;
            let mut prepare = |name, args| match NativeCallbackPayload::prepare(name, args) {
                Ok(value) => Some(value),
                Err(error) => {
                    callback_error = Some(diagnostic(error));
                    None
                }
            };
            let requested = result.as_ref().ok().filter(|value| value.attempted).and_then(|value| {
                prepare(
                    super::super::APP_CONTROL_COMMAND_REQUESTED_CALLBACK,
                    vec![serde_json::json!(method), value.details.clone()],
                )
            });
            let failed = error.as_ref().and_then(|error| {
                prepare(
                    super::super::APP_CONTROL_COMMAND_FAILED_CALLBACK,
                    vec![serde_json::json!(method), serde_json::json!(error)],
                )
            });
            reservation
                .retain(
                    Outcome {
                        source: self.source.clone(),
                        attempted,
                        error,
                        callback_error,
                        requested,
                        failed,
                    },
                    COMMAND_WEIGHT,
                )
                .map_err(|_| "App Control outcome reservation failed".into())
        })();
        self.finish(result);
    }
}
impl Drop for Work {
    fn drop(&mut self) {
        if self.admitted && !self.state.complete.load(Ordering::Acquire) {
            self.finish(Err(if self.state.started.load(Ordering::Acquire) {
                "App Control execution interrupted; outcome is unconfirmed"
            } else {
                "App Control request cancelled; request was not sent"
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
