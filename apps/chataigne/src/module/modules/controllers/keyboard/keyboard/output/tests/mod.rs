use super::*;
use golden_io::PayloadPool;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
};

mod capture;
mod coordination;
mod engine;
mod flow;
mod ownership;
mod qualification;

#[derive(Clone, Default)]
struct Control {
    calls: Arc<Mutex<Vec<Request>>>,
    gate: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
    cleanup: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
    panic: Arc<AtomicBool>,
    fail: Arc<AtomicBool>,
}
struct Fake(Control);
impl Provider for Fake {
    fn execute(&mut self, request: Request) -> Result<String, String> {
        self.0.calls.lock().unwrap().push(request);
        if let Some(gate) = self.0.gate.lock().unwrap().take() {
            gate.recv().unwrap();
        }
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Keyboard output failure"
        );
        if self.0.fail.swap(false, Ordering::AcqRel) {
            return Err("é".repeat(4096));
        }
        Ok(format!(
            "{} {} key",
            request.action.summary_label(),
            request.key.as_str()
        ))
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(gate) = self.0.cleanup.lock().unwrap().take() {
            let _ = gate.recv();
        }
    }
}
fn until(mut predicate: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < limit, "Keyboard output fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn slots(count: usize) -> RetirementPool {
    RetirementPool::new(NonZeroUsize::new(count).unwrap())
}
fn pool() -> PayloadPool {
    PayloadPool::new("keyboard-output-fixture", 128, 32 * 1024 * 1024).unwrap()
}
fn request(key: KeyboardKey) -> Request {
    Request {
        key,
        action: KeyboardKeyAction::Tap,
    }
}
fn spawn(control: Control) -> (Runtime, RetirementPool, RetirementPool, PayloadPool) {
    let owner = slots(1);
    let global = slots(1);
    let pool = pool();
    let runtime = Runtime::spawn_using(&owner, &global, pool.clone(), WorkerNotifier::new(|| {}), move || {
        Ok(Fake(control))
    })
    .unwrap();
    (runtime, owner, global, pool)
}
fn source(
    runtime: &Runtime,
    admission: &RetirementPool,
    ingress: &RetirementPool,
    key: KeyboardKey,
) -> RetainedPayload<Source> {
    runtime
        .capture_fixture(
            Input::Prepared(request(key)),
            golden_core::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            admission.try_reserve().unwrap(),
            ingress.try_reserve().unwrap(),
        )
        .unwrap()
}
fn submit(runtime: &Runtime, source: RetainedPayload<Source>) -> Receipt {
    source.arrive();
    runtime.submit(source).unwrap_or_else(|_| panic!("fixture admission"))
}
