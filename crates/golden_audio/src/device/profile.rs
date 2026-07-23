use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::AudioDeviceProfileKey;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioDeviceProfile<T> {
    pub key: AudioDeviceProfileKey,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DeviceProfileStore<T> {
    profiles: BTreeMap<AudioDeviceProfileKey, T>,
}

impl<T> Default for DeviceProfileStore<T> {
    fn default() -> Self {
        Self {
            profiles: BTreeMap::new(),
        }
    }
}

impl<T> DeviceProfileStore<T> {
    #[must_use]
    pub fn get(&self, key: &AudioDeviceProfileKey) -> Option<&T> {
        self.profiles.get(key)
    }

    pub fn insert(&mut self, profile: AudioDeviceProfile<T>) -> Option<T> {
        self.profiles.insert(profile.key, profile.value)
    }

    pub fn remove(&mut self, key: &AudioDeviceProfileKey) -> Option<T> {
        self.profiles.remove(key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }
}
