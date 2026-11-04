//! A presentation task pins its startup choice; edits request restart without moving devices.
use crate::{
    app::AppNode,
    preferences::{AUTOMATIC, native::Catalog},
};
use golden_core::application::ProductionRuntime;
use golden_video::native::{AdapterId, AdapterInfo, AdapterPreference, select_adapter};

#[derive(Default)]
pub(super) struct Selection {
    catalog: Catalog,
    startup: Option<String>,
    adapter: Option<AdapterInfo>,
}
impl Selection {
    pub fn service(&mut self, runtime: &ProductionRuntime<AppNode>) {
        self.catalog.service(runtime);
        if self.startup.is_none() {
            self.startup = self.catalog.value.clone();
        }
    }
    pub fn adapter(&mut self) -> Result<&AdapterInfo, String> {
        if self.adapter.is_none() {
            let value = self.startup.as_deref().ok_or("GPU Preferences not ready")?;
            let preference = if value == AUTOMATIC {
                AdapterPreference::Automatic
            } else {
                AdapterPreference::Explicit(AdapterId::from_key(value).ok_or("invalid selected GPU identity")?)
            };
            self.adapter = Some(select_adapter(preference).map_err(|error| error.to_string())?);
        }
        Ok(self.adapter.as_ref().unwrap())
    }
    pub fn decorate(&self, status: &mut crate::preview::protocol::VideoRuntimeStatus) {
        status.gpu_preference = self.catalog.value.clone();
        status.gpu_restart_required = self.startup.is_some() && self.catalog.value != self.startup;
        if status.error.is_none() {
            status.error = self.catalog.error.clone();
        }
    }
}
