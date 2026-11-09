//! A presentation task pins its startup choice; edits request restart without moving devices.
use crate::{
    app::AppNode,
    preferences::{AUTOMATIC, native::Catalog},
};
use golden_core::application::ProductionRuntime;
use golden_video::native::{AdapterInfo, AdapterPreference, PhysicalAdapterId, select_adapter, select_from};

#[derive(Default)]
pub(super) struct Selection {
    catalog: Catalog,
    startup: Option<String>,
    adapter: Option<AdapterInfo>,
}
impl Selection {
    pub fn service(&mut self, runtime: &ProductionRuntime<AppNode>) {
        let refreshed = self.catalog.service(runtime);
        if self.startup.is_none() {
            self.startup = self.catalog.value.clone();
        }
        if refreshed
            && let Some(pinned) = self
                .adapter
                .as_ref()
                .and_then(|adapter| adapter.physical_id.as_ref().ok())
            && let Ok(current) = select_from(&self.catalog.adapters, AdapterPreference::Explicit(pinned.clone()))
        {
            self.adapter = Some(current.clone());
        }
    }
    pub fn adapter(&mut self) -> Result<&AdapterInfo, String> {
        if self.adapter.is_none() {
            let value = self.startup.as_deref().ok_or("GPU Preferences not ready")?;
            let preference = if value == AUTOMATIC {
                AdapterPreference::Automatic
            } else {
                AdapterPreference::Explicit(PhysicalAdapterId::from_key(value).ok_or("invalid selected GPU identity")?)
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
