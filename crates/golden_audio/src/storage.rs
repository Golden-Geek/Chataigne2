use std::mem::size_of;

use crate::{AudioDeviceFingerprint, AudioDeviceTargetId};

pub(crate) struct Budget {
    pub(crate) bytes: usize,
    pub(crate) maximum: usize,
    pub(crate) items: usize,
}

impl Budget {
    pub(crate) fn add(&mut self, bytes: usize) -> Option<()> {
        self.bytes = self.bytes.checked_add(bytes)?;
        (self.bytes <= self.maximum).then_some(())
    }
    pub(crate) fn items(&mut self, count: usize) -> Option<()> {
        self.items = self.items.checked_sub(count)?;
        Some(())
    }
    pub(crate) fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.items(values.len())?;
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }
    pub(crate) fn optional(&mut self, value: &Option<String>) -> Option<()> {
        self.add(value.as_ref().map_or(0, String::capacity))
    }
    pub(crate) fn target(&mut self, target: &AudioDeviceTargetId) -> Option<()> {
        match target {
            AudioDeviceTargetId::SystemDefault { backend } => self.add(backend.retained_heap_bytes()),
            AudioDeviceTargetId::Device { backend, device } => {
                self.add(backend.retained_heap_bytes())?;
                self.add(device.retained_heap_bytes())
            }
        }
    }
    pub(crate) fn fingerprint(&mut self, fingerprint: &AudioDeviceFingerprint) -> Option<()> {
        let AudioDeviceFingerprint {
            vendor,
            product,
            serial,
            transport,
            backend_path,
            input_channels: _,
            output_channels: _,
            properties,
        } = fingerprint;
        for value in [vendor, product, serial, transport, backend_path] {
            self.optional(value)?;
        }
        self.items(properties.len())?;
        self.add(properties.len().checked_add(1)?.checked_mul(1024)?)?;
        for (key, value) in properties {
            self.add(key.capacity())?;
            self.add(value.capacity())?;
        }
        Some(())
    }
}
