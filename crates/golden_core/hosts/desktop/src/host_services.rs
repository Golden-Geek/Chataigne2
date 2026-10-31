//! App-owned services selected by host mode, separate from engine/project activation.
use crate::NativePresentation;
use golden_engine::{app::ProjectLifecycle, application::ProductionRuntime};
use std::io;
use tauri::Runtime;

pub(crate) type PresentationFactory<T, R> =
    Box<dyn FnOnce(ProductionRuntime<T>) -> io::Result<NativePresentation<R>> + Send>;
pub(crate) type HeadlessFactory<T> = Box<dyn FnOnce(ProductionRuntime<T>) -> io::Result<Box<dyn Send>> + Send>;

/// Mode-specific services started only for the authoritative host runtime.
/// Factories run once after listener binding, never during project candidate validation.
pub struct HostServices<T: ProjectLifecycle, R: Runtime> {
    pub(crate) presentation: Option<PresentationFactory<T, R>>,
    pub(crate) headless: Option<HeadlessFactory<T>>,
}
impl<T: ProjectLifecycle, R: Runtime> Default for HostServices<T, R> {
    fn default() -> Self {
        Self {
            presentation: None,
            headless: None,
        }
    }
}
impl<T: ProjectLifecycle, R: Runtime> HostServices<T, R> {
    /// Attach native rendering to the desktop window; headless mode never invokes this factory.
    pub fn with_native_presentation(
        mut self,
        factory: impl FnOnce(ProductionRuntime<T>) -> io::Result<NativePresentation<R>> + Send + 'static,
    ) -> Self {
        self.presentation = Some(Box::new(factory));
        self
    }
    /// Start an app service in headless mode, retaining its guard for the server's lifetime.
    /// The factory and guard's Drop must return promptly; the guard owns cancellation and background retirement.
    pub fn with_headless<S: Send + 'static>(
        mut self,
        factory: impl FnOnce(ProductionRuntime<T>) -> io::Result<S> + Send + 'static,
    ) -> Self {
        self.headless = Some(Box::new(move |runtime| {
            factory(runtime).map(|service| Box::new(service) as Box<dyn Send>)
        }));
        self
    }
}
