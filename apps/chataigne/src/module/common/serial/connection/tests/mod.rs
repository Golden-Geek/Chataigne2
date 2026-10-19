use super::*;
use std::{
    collections::VecDeque,
    io,
    sync::{mpsc, Mutex},
    thread,
    time::{Duration, Instant},
};

mod input;
mod lifecycle;
mod logging;
mod pressure;

enum Action {
    Count(usize),
    Fail(io::ErrorKind),
    Block(mpsc::Sender<()>, mpsc::Receiver<()>, usize),
}
#[derive(Default)]
struct State {
    actions: VecDeque<Action>,
    output: Vec<u8>,
    input: VecDeque<Vec<u8>>,
    read_error: Option<io::ErrorKind>,
}
struct Wire {
    state: Arc<Mutex<State>>,
    drop_gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
}
impl Read for Wire {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let mut state = self.state.lock().unwrap();
        if let Some(error) = state.read_error.take() {
            return Err(error.into());
        }
        let Some(mut input) = state.input.pop_front() else {
            return Err(io::ErrorKind::WouldBlock.into());
        };
        let count = bytes.len().min(input.len());
        bytes[..count].copy_from_slice(&input[..count]);
        if count < input.len() {
            input.drain(..count);
            state.input.push_front(input);
        }
        Ok(count)
    }
}
impl Write for Wire {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let action = self.state.lock().unwrap().actions.pop_front();
        let count = match action {
            Some(Action::Count(count)) => count,
            Some(Action::Fail(error)) => return Err(error.into()),
            Some(Action::Block(started, release, count)) => {
                started.send(()).unwrap();
                release.recv().unwrap();
                count
            }
            None => bytes.len(),
        }
        .min(bytes.len());
        self.state.lock().unwrap().output.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("unbounded flush")
    }
}
impl Drop for Wire {
    fn drop(&mut self) {
        if let Some((started, release)) = self.drop_gate.take() {
            started.send(()).unwrap();
            release.recv().unwrap();
        }
    }
}
fn config() -> SerialConnectionConfig {
    SerialConnectionConfig {
        port_name: "fixture".into(),
        baud_rate: 115200,
        dtr: false,
        rts: false,
        input: StreamInputConfig {
            module: golden_core::node::NodeId(1),
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    }
}
fn pool() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "serial fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn until(handle: &SerialConnectionHandle, mut condition: impl FnMut(&Event) -> bool) {
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        let found = events.iter().any(&mut condition);
        assert!(found || drain.state != golden_io::PendingDrainState::Disconnected);
        found
    });
}
fn spawn_using(
    config: SerialConnectionConfig,
    pool: &RetirementPool,
    opener: impl FnMut(&SerialConnectionConfig) -> Result<Wire, String> + Send + 'static,
) -> Result<SerialConnectionHandle, String> {
    SerialConnectionHandle::spawn_using(config, pool, opener, WorkerNotifier::new(|| {}))
}
fn input_bytes(input: &PreparedStreamInput) -> Vec<u8> {
    input.callbacks[0].payload()["args"][0]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u8)
        .collect()
}
fn wire() -> (Wire, Arc<Mutex<State>>) {
    let state = Arc::new(Mutex::new(State::default()));
    (
        Wire {
            state: state.clone(),
            drop_gate: None,
        },
        state,
    )
}
fn connected(pool: &RetirementPool) -> (SerialConnectionHandle, Arc<Mutex<State>>) {
    let (port, state) = wire();
    let mut port = Some(port);
    let handle = spawn_using(config(), pool, move |_| port.take().ok_or("no replacement".into())).unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    (handle, state)
}

#[test]
fn admitted_commands_finish_fifo_after_partial_writes_without_replaying_the_prefix() {
    let pool = pool();
    let (handle, state) = connected(&pool);
    state.lock().unwrap().actions = VecDeque::from([
        Action::Count(3),
        Action::Fail(io::ErrorKind::WouldBlock),
        Action::Count(5),
    ]);
    let payload = vec![7; 100_000];
    handle.send(&payload, false).unwrap();
    handle.send(b"tail", false).unwrap();
    wait(|| state.lock().unwrap().output.len() == payload.len() + 4);
    assert_eq!(state.lock().unwrap().output, [payload.as_slice(), b"tail"].concat());
}

#[test]
fn failed_open_recovers_with_the_original_authored_line_configuration() {
    let pool = pool();
    let (port, state) = wire();
    let mut port = Some(port);
    let mut attempts = 0;
    let mut selected = config();
    selected.dtr = true;
    selected.rts = true;
    let expected = selected.clone();
    let handle = spawn_using(selected, &pool, move |actual| {
        assert_eq!(actual, &expected);
        attempts += 1;
        if attempts == 1 {
            Err("injected DTR setup failure".into())
        } else {
            port.take().ok_or("no replacement".into())
        }
    })
    .unwrap();
    until(
        &handle,
        |event| matches!(event, Event::Status(SerialConnectionStatus::Recovering { message, .. }) if message.contains("DTR")),
    );
    assert!(handle.send(b"unadmitted", false).is_err());
    until(&handle, |event| {
        matches!(event, Event::Status(SerialConnectionStatus::Connected { .. }))
    });
    handle.send(b"recovered", false).unwrap();
    wait(|| state.lock().unwrap().output == b"recovered");
}
