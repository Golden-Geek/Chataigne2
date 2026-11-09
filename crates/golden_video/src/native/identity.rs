//! Persistent device identity, separate from a runtime's replaceable DXGI LUID.
use super::AdapterId;
use serde::{Deserialize, Serialize};
use windows::Win32::Devices::{
    DeviceAndDriverInstallation::{CM_Get_Device_Interface_PropertyW, CM_MapCrToWin32Err, CR_SUCCESS},
    Display::{DISPLAYCONFIG_ADAPTER_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME, DisplayConfigGetDeviceInfo},
    Properties::{DEVPKEY_Device_InstanceId, DEVPROP_TYPE_STRING, DEVPROPTYPE},
};
use windows::Win32::Foundation::{E_INVALIDARG, ERROR_GEN_FAILURE, LUID};
use windows::core::{Error, HRESULT, PCWSTR, Result};

/// Exact PnP device instance. Labels, enumeration order and PCI model IDs are not identity.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PhysicalAdapterId(String);

impl PhysicalAdapterId {
    /// Canonical persistent preference token. Historical LUID tokens are not physical keys.
    pub fn key(&self) -> String {
        format!("pnp:{}", self.0)
    }
    /// Parse a bounded exact device-instance token, rejecting labels and wildcard targets.
    pub fn from_key(key: &str) -> Option<Self> {
        Self::from_instance(key.strip_prefix("pnp:")?)
    }
    fn from_instance(instance: &str) -> Option<Self> {
        if instance.len() >= 200
            || !instance.is_ascii()
            || instance.bytes().any(|b| b.is_ascii_control() || b == b'*' || b == b'?')
        {
            return None;
        }
        let mut parts = instance.split('\\');
        ((0..3).all(|_| parts.next().is_some_and(|part| !part.is_empty())) && parts.next().is_none())
            .then(|| Self(instance.to_ascii_uppercase()))
    }
}
impl TryFrom<String> for PhysicalAdapterId {
    type Error = &'static str;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Self::from_key(&value).ok_or("invalid physical GPU identity")
    }
}
impl From<PhysicalAdapterId> for String {
    fn from(value: PhysicalAdapterId) -> Self {
        value.key()
    }
}

pub(super) fn physical_id(id: AdapterId) -> Result<PhysicalAdapterId> {
    let mut name = DISPLAYCONFIG_ADAPTER_NAME::default();
    name.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_ADAPTER_NAME;
    name.header.size = size_of::<DISPLAYCONFIG_ADAPTER_NAME>() as u32;
    name.header.adapterId = LUID {
        LowPart: id.low,
        HighPart: id.high,
    };
    // SAFETY: correctly sized adapter request; no display setting is changed.
    let status = unsafe { DisplayConfigGetDeviceInfo(&mut name.header) };
    if status != 0 {
        return Err(Error::from_hresult(HRESULT::from_win32(status as u32)));
    }
    if !name.adapterDevicePath.contains(&0) {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let mut instance = [0u16; 200];
    let mut bytes = size_of_val(&instance) as u32;
    let mut kind = DEVPROPTYPE::default();
    // SAFETY: the NUL-terminated OS interface path remains live; a fixed, aligned buffer
    // admits the documented maximum device-instance length, including its terminator.
    let status = unsafe {
        CM_Get_Device_Interface_PropertyW(
            PCWSTR(name.adapterDevicePath.as_ptr()),
            &DEVPKEY_Device_InstanceId,
            &mut kind,
            Some(instance.as_mut_ptr().cast()),
            &mut bytes,
            0,
        )
    };
    if status != CR_SUCCESS {
        return Err(Error::from_hresult(HRESULT::from_win32(unsafe {
            CM_MapCrToWin32Err(status, ERROR_GEN_FAILURE.0)
        })));
    }
    if kind != DEVPROP_TYPE_STRING || bytes < 2 || bytes as usize > size_of_val(&instance) || !bytes.is_multiple_of(2) {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let text = &instance[..bytes as usize / 2];
    if text.last() != Some(&0) || text[..text.len() - 1].contains(&0) {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    let text = String::from_utf16(&text[..text.len() - 1]).map_err(|_| Error::from_hresult(E_INVALIDARG))?;
    PhysicalAdapterId::from_instance(&text).ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

#[cfg(test)]
mod tests;
