use std::{any::Any, sync::Arc};

#[derive(Clone)]
pub(super) struct LocalPayload(Arc<dyn Any + Send + Sync>);
impl LocalPayload {
    pub fn new<T: Send + Sync + 'static>(value: T) -> Self {
        Self(Arc::new(value))
    }
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }
}
impl PartialEq for LocalPayload {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl std::fmt::Debug for LocalPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalPayload").finish_non_exhaustive()
    }
}
