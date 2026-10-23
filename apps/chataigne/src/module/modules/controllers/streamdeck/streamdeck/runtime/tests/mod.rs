use super::*;
use golden_core::node::Node;
mod ownership;
use std::{
    collections::VecDeque,
    thread::{self, ThreadId},
};
#[derive(Clone)]
pub(in super::super) struct SimulatedStreamDeck(Arc<Mutex<Simulated>>);
struct Simulated {
    available: bool,
    info: DeviceInfo,
    pressed: [bool; MAX_KEYS],
    input: VecDeque<Input>,
    rendered: Vec<KeyVisual>,
    brightness: u8,
    threads: Vec<ThreadId>,
}
impl SimulatedStreamDeck {
    pub(in super::super) fn new(serial: &str, count: usize) -> Self {
        assert!((1..=MAX_KEYS).contains(&count));
        Self(Arc::new(Mutex::new(Simulated {
            available: true,
            info: DeviceInfo {
                serial: serial.into(),
                product: "Simulated Stream Deck".into(),
                key_count: count,
                feedback: true,
            },
            pressed: [false; MAX_KEYS],
            input: VecDeque::with_capacity(MAX_KEYS),
            rendered: vec![KeyVisual::default(); count],
            brightness: 100,
            threads: Vec::new(),
        })))
    }
    pub(in super::super) fn press(&self, index: usize) {
        self.change(index, true);
    }
    pub(in super::super) fn release(&self, index: usize) {
        self.change(index, false);
    }
    fn change(&self, index: usize, pressed: bool) {
        let mut state = self.0.lock().unwrap();
        assert!(index < state.info.key_count);
        if state.pressed[index] != pressed {
            assert!(state.input.len() < MAX_KEYS, "simulator input is full");
            state.pressed[index] = pressed;
            state
                .input
                .push_back(if pressed { Input::Down(index) } else { Input::Up(index) });
        }
    }
    pub(in super::super) fn rendered(&self, index: usize) -> Option<KeyVisual> {
        self.0.lock().unwrap().rendered.get(index).cloned()
    }
    pub(in super::super) fn brightness(&self) -> u8 {
        self.0.lock().unwrap().brightness
    }
    pub(in super::super) fn set_available(&self, available: bool) {
        self.0.lock().unwrap().available = available;
    }
    fn record(&self) {
        let mut state = self.0.lock().unwrap();
        let id = thread::current().id();
        if !state.threads.contains(&id) {
            state.threads.push(id);
        }
    }
}
pub(super) struct Provider(SimulatedStreamDeck);
impl Driver for Provider {
    fn discover(&mut self) -> Result<Vec<DeviceInfo>, String> {
        self.0.record();
        let state = self.0 .0.lock().unwrap();
        Ok(if state.available {
            vec![state.info.clone()]
        } else {
            Vec::new()
        })
    }
    fn connect(&mut self, serial: &str) -> Result<DeviceInfo, String> {
        self.0.record();
        let state = self.0 .0.lock().unwrap();
        if !state.available {
            return Err("Simulated device is missing".into());
        }
        let info = state.info.clone();
        if info.serial != serial {
            return Err("Simulated device is missing".into());
        }
        Ok(info)
    }
    fn disconnect(&mut self) -> Result<(), String> {
        self.0.record();
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Input>, String> {
        self.0.record();
        let mut state = self.0 .0.lock().unwrap();
        if !state.available {
            return Err("Simulated device disconnected".into());
        }
        Ok(state.input.drain(..).collect())
    }
    fn render(&mut self, index: usize, visual: &KeyVisual) -> Result<(), RenderError> {
        self.0.record();
        self.0 .0.lock().unwrap().rendered[index] = visual.clone();
        Ok(())
    }
    fn brightness(&mut self, value: u8) -> Result<(), String> {
        self.0.record();
        self.0 .0.lock().unwrap().brightness = value;
        Ok(())
    }
}
impl super::super::StreamDeckModule {
    pub(in super::super) fn install_simulated_device(&mut self, device: SimulatedStreamDeck) {
        self.stop_runtime();
        self.service.simulator = Some(device.clone());
        self.service.factory = Some(Box::new(move |owner, notifier, frame| {
            let device = device.clone();
            Runtime::spawn_using(
                owner,
                lifetimes(),
                Resources::new(sources()?, domains()?)?,
                notifier,
                frame,
                move || Ok(Provider(device)),
            )
        }));
        self.request_service();
    }
    pub(in super::super) fn simulated(&self) -> &SimulatedStreamDeck {
        self.service.simulator.as_ref().unwrap()
    }
    pub(in super::super) fn simulated_mut(&mut self) -> &SimulatedStreamDeck {
        self.service.simulator.as_ref().unwrap()
    }
}

#[test]
fn empty_mailbox_pressure_is_reported_by_the_actor_and_clears_after_recovery() {
    use super::super::tests::{create_streamdeck_module, module_mut, run_tick};
    use std::time::{Duration, Instant};
    let (mut engine, id) = create_streamdeck_module();
    let module = module_mut(&mut engine, id);
    let device = module.simulated().clone();
    module.stop_runtime();
    assert!(module.service.owner.wait_for_idle(Duration::from_secs(1)));
    let domains = PayloadPool::new("streamdeck-actor-pressure", 4, 1024 * 1024).unwrap();
    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(domains.try_retain((), 1).unwrap());
    }
    module.service.factory = Some(Box::new(move |owner, notifier, frame| {
        let device = device.clone();
        Runtime::spawn_using(
            owner,
            lifetimes(),
            Resources::new(sources()?, domains.clone())?,
            notifier,
            frame,
            move || Ok(Provider(device)),
        )
    }));
    module.request_service();
    let until = Instant::now() + Duration::from_secs(3);
    loop {
        run_tick(&mut engine);
        let module = module_mut(&mut engine, id);
        if module.service.pressure {
            assert_eq!(module.service.runtime.as_ref().unwrap().worker.event_metrics().depth, 0);
            assert!(module
                .node_data()
                .meta
                .presentation
                .warnings
                .iter()
                .any(|warning| warning.id == "streamdeck_pressure"));
            break;
        }
        assert!(Instant::now() < until);
    }
    drop(held);
    loop {
        run_tick(&mut engine);
        let module = module_mut(&mut engine, id);
        if !module.service.pressure {
            assert!(!module
                .node_data()
                .meta
                .presentation
                .warnings
                .iter()
                .any(|warning| warning.id == "streamdeck_pressure"));
            break;
        }
        assert!(Instant::now() < until);
    }
}
