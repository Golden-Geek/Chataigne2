use asio_sys::Asio;

use crate::{AudioDeviceCatalogEntry, AudioDeviceId, AudioDeviceInventory, AudioDeviceTargetId, BackendId};

pub(super) fn lightweight_inventory(backend: &BackendId) -> AudioDeviceInventory {
    let catalog = Asio::new()
        .driver_names()
        .into_iter()
        .map(|name| AudioDeviceCatalogEntry {
            target: target_for_driver(backend, &name),
            label: name,
        })
        .collect();
    AudioDeviceInventory {
        catalog,
        devices: Vec::new(),
    }
}

pub(super) fn has_installed_driver() -> bool {
    !Asio::new().driver_names().is_empty()
}

fn target_for_driver(backend: &BackendId, name: &str) -> AudioDeviceTargetId {
    AudioDeviceTargetId::Device {
        backend: backend.clone(),
        device: AudioDeviceId::new(format!("asio:{name}")).expect("ASIO driver names are non-empty"),
    }
}
