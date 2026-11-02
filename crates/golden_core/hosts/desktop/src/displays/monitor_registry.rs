use super::metadata::canonical_edid;
use windows::{
    Win32::{
        Devices::DeviceAndDriverInstallation::{
            DICS_FLAG_GLOBAL, DIREG_DEV, HDEVINFO, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
            SP_DEVINFO_DATA, SetupDiCreateDeviceInfoList, SetupDiDestroyDeviceInfoList,
            SetupDiGetDeviceInterfaceDetailW, SetupDiOpenDevRegKey, SetupDiOpenDeviceInterfaceW,
        },
        Foundation::ERROR_SUCCESS,
        System::Registry::{HKEY, KEY_QUERY_VALUE, REG_BINARY, REG_VALUE_TYPE, RegCloseKey, RegQueryValueExW},
    },
    core::{PCWSTR, w},
};

struct DeviceInfo(HDEVINFO);
impl Drop for DeviceInfo {
    fn drop(&mut self) {
        // SAFETY: this owner closes the sole information-set handle returned by SetupAPI.
        unsafe {
            let _ = SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}
struct HardwareKey(HKEY);
impl Drop for HardwareKey {
    fn drop(&mut self) {
        // SAFETY: this owner closes the read-only device hardware key exactly once.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

pub(super) fn read_edid(interface_path: &[u16]) -> Option<String> {
    let end = interface_path.iter().position(|value| *value == 0)?;
    if end == 0 {
        return None;
    }
    // SAFETY: every buffer is initialized and bounded; the path is checked NUL-terminated.
    // No handles or OS-owned pointers escape this synchronous preparation-worker query.
    unsafe {
        let devices = DeviceInfo(SetupDiCreateDeviceInfoList(None, None).ok()?);
        let mut interface = SP_DEVICE_INTERFACE_DATA {
            cbSize: size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        SetupDiOpenDeviceInterfaceW(devices.0, PCWSTR(interface_path.as_ptr()), 0, Some(&mut interface)).ok()?;
        let mut required = 0;
        let _ = SetupDiGetDeviceInterfaceDetailW(devices.0, &interface, None, 0, Some(&mut required), None);
        if !(size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32..=4096).contains(&required) {
            return None;
        }
        // u64 storage guarantees alignment for the platform detail structure, including x64.
        let mut storage = vec![0_u64; (required as usize).div_ceil(size_of::<u64>())];
        let detail = storage.as_mut_ptr().cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        (*detail).cbSize = size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        let mut device = SP_DEVINFO_DATA {
            cbSize: size_of::<SP_DEVINFO_DATA>() as u32,
            ..Default::default()
        };
        SetupDiGetDeviceInterfaceDetailW(devices.0, &interface, Some(detail), required, None, Some(&mut device))
            .ok()?;
        let key = HardwareKey(
            SetupDiOpenDevRegKey(devices.0, &device, DICS_FLAG_GLOBAL.0, 0, DIREG_DEV, KEY_QUERY_VALUE.0).ok()?,
        );
        let mut kind = REG_VALUE_TYPE::default();
        let mut length = 0;
        if RegQueryValueExW(key.0, w!("EDID"), None, Some(&mut kind), None, Some(&mut length)) != ERROR_SUCCESS
            || kind != REG_BINARY
            || !(128..=4096).contains(&length)
        {
            return None;
        }
        let mut data = vec![0_u8; length as usize];
        if RegQueryValueExW(
            key.0,
            w!("EDID"),
            None,
            Some(&mut kind),
            Some(data.as_mut_ptr()),
            Some(&mut length),
        ) != ERROR_SUCCESS
            || kind != REG_BINARY
            || length as usize > data.len()
        {
            return None;
        }
        data.truncate(length as usize);
        canonical_edid(&data)
    }
}
