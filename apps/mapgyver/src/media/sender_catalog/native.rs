//! Coarse SDK enumeration and catalog preparation on an existing host worker.
use super::Source;
use crate::app::AppNode;
use golden_core::application::ProductionRuntime;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

pub(crate) struct Catalog {
    due: Instant,
    discovery: Option<golden_video::native::SpoutDiscovery>,
    source: Option<Source>,
    names: Vec<String>,
    error: Option<String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            discovery: None,
            source: None,
            names: Vec::new(),
            error: None,
        }
    }
}
impl Catalog {
    pub fn service(&mut self, runtime: &ProductionRuntime<AppNode>) {
        if Instant::now() < self.due {
            return;
        }
        self.due = Instant::now() + Duration::from_millis(500);
        let result = self.update(runtime);
        let error = result.err();
        if error != self.error
            && let Some(error) = &error
        {
            eprintln!("[mapgyver-senders] {error}");
        }
        self.error = error;
    }
    fn update(&mut self, runtime: &ProductionRuntime<AppNode>) -> Result<(), String> {
        if self.discovery.is_none() {
            self.discovery = Some(golden_video::native::SpoutDiscovery::new().map_err(|e| e.to_string())?);
        }
        let discovery_error = match self
            .discovery
            .as_mut()
            .unwrap()
            .snapshot(NonZeroUsize::new(64).unwrap())
        {
            Ok(catalog) => {
                self.names = catalog.names;
                None
            }
            Err(error) => {
                self.discovery = None;
                self.names.clear();
                Some(error.to_string())
            }
        };
        let dirty = self.source.as_ref().is_none_or(Source::dirty);
        let mut previous = self.source.take();
        self.source = Some(
            runtime
                .capture_app(move |engine| {
                    if previous.as_ref().is_none_or(|source| {
                        source.membership != engine.nodes.membership_revision() || source.publisher.waker.is_closed()
                    }) {
                        Source::capture(engine)
                    } else {
                        let mut source = previous.take().unwrap();
                        if dirty {
                            source.refresh(engine)?;
                        }
                        Ok(source)
                    }
                })
                .map_err(|e| e.to_string())??,
        );
        self.source
            .as_mut()
            .ok_or("sender catalog source missing")?
            .publish(&self.names)?;
        discovery_error.map_or(Ok(()), Err)
    }
}
