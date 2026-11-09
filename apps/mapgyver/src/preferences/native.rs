//! Coarse native catalog observation, only on desktop/headless host workers.
use crate::app::AppNode;
use golden_core::application::ProductionRuntime;
use std::time::{Duration, Instant};

pub(crate) struct Catalog {
    due: Instant,
    pub value: Option<String>,
    pub error: Option<String>,
    pub adapters: Vec<golden_video::native::AdapterInfo>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            value: None,
            error: None,
            adapters: Vec::new(),
        }
    }
}
impl Catalog {
    pub fn service(&mut self, runtime: &ProductionRuntime<AppNode>) -> bool {
        if Instant::now() < self.due {
            return false;
        }
        self.due = Instant::now() + Duration::from_secs(1);
        let result = (|| {
            let source = runtime
                .capture_app(super::Source::capture)
                .map_err(|error| error.to_string())??;
            self.value = Some(source.value.clone());
            let adapters = golden_video::native::adapters().map_err(|error| error.to_string())?;
            let available = adapters
                .iter()
                .filter(|adapter| !adapter.software)
                .map(|adapter| {
                    adapter
                        .physical_id
                        .as_ref()
                        .map(|key| (key.key(), adapter.description.clone()))
                        .map_err(|error| format!("{} GPU identity unavailable: {error:#x}", adapter.description))
                })
                .collect::<Result<Vec<_>, _>>()?;
            source.publish(&available)?;
            self.adapters = adapters;
            Ok(())
        })();
        let error = result.err();
        if error != self.error
            && let Some(error) = &error
        {
            eprintln!("[mapgyver-gpu] {error}");
        }
        self.error = error;
        true
    }
}
