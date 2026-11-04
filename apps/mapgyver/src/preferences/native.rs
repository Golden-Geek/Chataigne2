//! Coarse native catalog observation, only on desktop/headless host workers.
use crate::app::AppNode;
use golden_core::application::ProductionRuntime;
use std::time::{Duration, Instant};

pub(crate) struct Catalog {
    due: Instant,
    pub value: Option<String>,
    pub error: Option<String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            value: None,
            error: None,
        }
    }
}
impl Catalog {
    pub fn service(&mut self, runtime: &ProductionRuntime<AppNode>) {
        if Instant::now() < self.due {
            return;
        }
        self.due = Instant::now() + Duration::from_secs(1);
        let result = (|| {
            let source = runtime
                .capture_app(super::Source::capture)
                .map_err(|error| error.to_string())??;
            self.value = Some(source.value.clone());
            let available = golden_video::native::adapters()
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter(|adapter| !adapter.software)
                .map(|adapter| (adapter.id.key(), adapter.description))
                .collect::<Vec<_>>();
            source.publish(&available)
        })();
        let error = result.err();
        if error != self.error
            && let Some(error) = &error
        {
            eprintln!("[mapgyver-gpu] {error}");
        }
        self.error = error;
    }
}
