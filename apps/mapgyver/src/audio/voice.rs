//! The channel node owns routing values. Workers consume these typed, replaceable snapshots.
use super::{ndi_route::RouteBinding, preferences};
use golden_audio::{AudioChannelId, GainDb};
use golden_core::{engine::NodeWaker, parameter::ParameterEnumOptions};
use std::sync::{Arc, Mutex, Weak};

pub(crate) const LEFT: &str = "8fe83692-442b-4bc0-92b8-87256dfe0001";
pub(crate) const RIGHT: &str = "8fe83692-442b-4bc0-92b8-87256dfe0002";
pub(crate) const UNASSIGNED: &str = preferences::UNASSIGNED;

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
    additional_routes: Vec<Weak<RouteBinding>>,
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

    pub fn retire(&self) {
        let mut state = self.0.lock().unwrap();
        state.routing = None;
        state.additional_routes.clear();
    }

    pub fn options(&self) -> Option<ParameterEnumOptions> {
        self.0.lock().unwrap().options.clone()
    }

    #[cfg(feature = "video-files")]
    pub fn routing(&self) -> Option<Routing> {
        self.0.lock().unwrap().routing
    }

    pub fn register_ndi_route(&self, route: &Arc<RouteBinding>) {
        let mut state = self.0.lock().unwrap();
        state.additional_routes.retain(|prior| prior.strong_count() != 0);
        state.additional_routes.push(Arc::downgrade(route));
        if let Some(options) = &state.options {
            route.update_options(options);
        }
    }

    #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
    pub fn visit_ndi_routes(&self, mut visit: impl FnMut(usize, Option<AudioChannelId>)) -> Option<GainDb> {
        let state = self.0.lock().unwrap();
        let routing = state.routing.filter(|routing| routing.enabled)?;
        for (source, destination) in routing.destinations.into_iter().enumerate() {
            visit(source, destination);
        }
        for route in state.additional_routes.iter().filter_map(Weak::upgrade) {
            if let Some((source, destination)) = route.route() {
                visit(source, destination);
            }
        }
        Some(routing.gain)
    }

    #[cfg(feature = "video-files")]
    pub fn update_options(&self, options: &ParameterEnumOptions) {
        let mut state = self.0.lock().unwrap();
        if state.options.as_ref() == Some(options) {
            return;
        }
        state.options = Some(options.clone());
        state.additional_routes.retain(|route| {
            if let Some(route) = route.upgrade() {
                route.update_options(options);
                true
            } else {
                false
            }
        });
        let waker = state.waker.clone();
        drop(state);
        if let Some(waker) = waker {
            let _ = waker.wake();
        }
    }
}
