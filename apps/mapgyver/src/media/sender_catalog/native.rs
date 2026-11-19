//! Coarse SDK enumeration and catalog preparation on an existing host worker.
use super::Source;
use crate::app::AppNode;
use crate::media::protocol::MediaKind;
use golden_core::application::ProductionRuntime;
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

pub(crate) struct Catalog {
    due: Instant,
    discovery: Option<golden_video::native::SpoutDiscovery>,
    source: Option<Source>,
    error: Option<String>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            discovery: None,
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
        let (names, internal_names, discovery_error) = match self
            .discovery
            .as_mut()
            .unwrap()
            .snapshot(NonZeroUsize::new(64).unwrap())
        {
            Ok(catalog) => (catalog.names, catalog.internal_names, None),
            Err(error) => {
                self.discovery = None;
                (Vec::new(), Vec::new(), Some(error.to_string()))
            }
        };
        self.source = Some(reconcile(runtime, self.source.take(), MediaKind::SharedTexture)?);
        self.source
            .as_mut()
            .ok_or("sender catalog source missing")?
            .publish(&names, &internal_names)?;
        discovery_error.map_or(Ok(()), Err)
    }
}

fn reconcile(
    runtime: &ProductionRuntime<AppNode>,
    previous: Option<Source>,
    kind: MediaKind,
) -> Result<Source, String> {
    let dirty = previous.as_ref().is_none_or(Source::dirty);
    runtime
        .capture_app(move |engine| match previous {
            Some(mut source)
                if source.membership == engine.nodes.membership_revision() && !source.publisher.waker.is_closed() =>
            {
                if dirty {
                    source.refresh(engine)?;
                }
                Ok(source)
            }
            _ if kind == MediaKind::SharedTexture => Source::capture(engine),
            _ => Source::capture_for(engine, kind),
        })
        .map_err(|error| error.to_string())?
}

#[cfg(target_arch = "x86_64")]
pub(crate) struct NdiCatalog {
    due: Instant,
    discovery: Option<golden_video::ndi::NdiDiscovery>,
    source: Option<Source>,
    publication_error: Option<String>,
}

#[cfg(target_arch = "x86_64")]
impl Default for NdiCatalog {
    fn default() -> Self {
        Self {
            due: Instant::now(),
            discovery: None,
            source: None,
            publication_error: None,
        }
    }
}

#[cfg(target_arch = "x86_64")]
impl NdiCatalog {
    pub(crate) fn service(&mut self, runtime: &ProductionRuntime<AppNode>) {
        let now = Instant::now();
        if now < self.due {
            return;
        }
        let runtime_error = if self.discovery.is_none() {
            match golden_video::ndi::NdiDiscovery::new() {
                Ok(discovery) => {
                    self.discovery = Some(discovery);
                    None
                }
                Err(error) => Some(std::sync::Arc::<str>::from(error.to_string())),
            }
        } else {
            None
        };
        self.due = now + Duration::from_millis(if runtime_error.is_some() { 2000 } else { 500 });
        let names = self
            .discovery
            .as_mut()
            .map(|discovery| {
                discovery
                    .sources()
                    .iter()
                    .map(|source| source.name().to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let result = reconcile(runtime, self.source.take(), MediaKind::Ndi).and_then(|mut source| {
            let internal = golden_video::ndi::active_sender_names();
            source.publish_discovery(&names, &internal, runtime_error)?;
            self.source = Some(source);
            Ok(())
        });
        let error = result.err();
        if error != self.publication_error
            && let Some(error) = &error
        {
            eprintln!("[mapgyver-ndi-discovery] {error}");
        }
        self.publication_error = error;
    }
}
