//! The channel node owns routing values. Workers consume these typed, replaceable snapshots.
use golden_audio::{AudioChannelId, GainDb};
use golden_core::{engine::NodeWaker, parameter::ParameterEnumOptions};
use std::sync::Mutex;

pub(crate) const LEFT: &str = "8fe83692-442b-4bc0-92b8-87256dfe0001";
pub(crate) const RIGHT: &str = "8fe83692-442b-4bc0-92b8-87256dfe0002";
pub(crate) const UNASSIGNED: &str = preferences::UNASSIGNED;
use super::preferences;

pub(crate) fn default_options() -> ParameterEnumOptions {
    [
        preferences::option(UNASSIGNED, UNASSIGNED.into()),
        preferences::option(LEFT, "Left".into()),
        preferences::option(RIGHT, "Right".into()),
    ]
    .to_vec()
    .into()
}

pub(crate) fn option_set(options: &ParameterEnumOptions, authored: &str) -> ParameterEnumOptions {
    if options.contains_variant(authored) {
        return options.clone();
    }
    let mut choices = options.as_slice().to_vec();
    choices.push(preferences::option(authored, format!("{authored} (unavailable)")));
    choices.into()
}

pub(crate) fn parse_route(choice: &str) -> Result<Option<AudioChannelId>, String> {
    if choice == UNASSIGNED {
        Ok(None)
    } else {
        choice
            .parse()
            .map(Some)
            .map_err(|_| "Invalid virtual audio output".into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Routing {
    pub enabled: bool,
    pub destinations: [Option<AudioChannelId>; 2],
    pub gain: GainDb,
}

#[derive(Default)]
struct State {
    routing: Option<Routing>,
    options: Option<ParameterEnumOptions>,
    waker: Option<NodeWaker>,
}

#[derive(Default)]
pub(crate) struct Binding(Mutex<State>);

impl Binding {
    pub fn watch_options(&self, waker: Option<NodeWaker>) {
        self.0.lock().unwrap().waker = waker;
    }

    pub fn update_routing(&self, routing: Routing) {
        self.0.lock().unwrap().routing = Some(routing);
    }

    #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
    pub fn retire(&self) {
        self.0.lock().unwrap().routing = None;
    }

    pub fn options(&self) -> Option<ParameterEnumOptions> {
        self.0.lock().unwrap().options.clone()
    }

    #[cfg(feature = "video-files")]
    pub fn routing(&self) -> Option<Routing> {
        self.0.lock().unwrap().routing
    }

    #[cfg(feature = "video-files")]
    pub fn update_options(&self, options: &ParameterEnumOptions) {
        let mut state = self.0.lock().unwrap();
        if state.options.as_ref() == Some(options) {
            return;
        }
        state.options = Some(options.clone());
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            let _ = waker.wake();
        }
    }
}
