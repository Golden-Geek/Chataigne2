use crate::{AudioDeviceProfile, DeviceProfileStore};

use super::support::{device, fingerprint};

#[test]
fn matching_device_profile_restores_its_route_set() {
    let device = device("device", "Device", true, fingerprint("Device", 2, 2), 2, 2, false, true);
    let mut profiles = DeviceProfileStore::default();
    profiles.insert(AudioDeviceProfile {
        key: device.profile_key.clone(),
        value: vec!["input:0 -> virtual:0", "virtual:0 -> output:0"],
    });

    assert_eq!(
        profiles.get(&device.profile_key),
        Some(&vec!["input:0 -> virtual:0", "virtual:0 -> output:0"])
    );
}
