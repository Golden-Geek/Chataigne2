//! Coarse display IO on the existing headless host worker; actor reads occur only when dirty.
use super::super::protocol::DisplayInfo;
use crate::{
    app::AppNode,
    screens::catalog_runtime::{Source, display_options},
};
use golden_core::application::ProductionRuntime;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(crate) struct Catalog {
    due: Instant,
    displays: Arc<[DisplayInfo]>,
    source: Option<Source>,
    error: Option<String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            displays: Arc::from([]),
            source: None,
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
        let error = self
            .update(runtime, super::scan().map_err(|error| error.to_string()))
            .err();
        if error != self.error
            && let Some(error) = &error
        {
            eprintln!("[mapgyver-displays] {error}");
        }
        self.error = error;
    }

    fn update(
        &mut self,
        runtime: &ProductionRuntime<AppNode>,
        result: Result<Vec<DisplayInfo>, String>,
    ) -> Result<(), String> {
        let (displays, error) = match result {
            Ok(displays) => (displays, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let dirty = self
            .source
            .as_ref()
            .is_none_or(|source| !source.guard.is_current() || source.publisher.is_closed());
        if dirty || self.displays.as_ref() != displays.as_slice() {
            let source = runtime
                .capture_app(Source::capture)
                .map_err(|error| error.to_string())??;
            let displays: Arc<[DisplayInfo]> = displays.into();
            let update = source.prepare(displays.clone(), &display_options(&displays));
            source.publisher.publish(update)?;
            self.displays = displays;
            self.source = Some(source);
        }
        error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests;
