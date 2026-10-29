use std::marker::PhantomData;
use std::rc::Rc;

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, DXGI_ERROR_UNSUPPORTED, IDXGIAdapter1,
    IDXGIFactory1,
};
use windows::core::{Error, HRESULT, Result};

use super::AdapterId;

/// Observed adapter metadata. Enumeration creates no rendering device or output window.
#[derive(Clone, Debug)]
pub struct AdapterInfo {
    /// Adapter identity for explicit device creation and recreation.
    pub id: AdapterId,
    /// Human-readable driver description.
    pub description: String,
    /// Software adapters are listed for observation but rejected by the native backend.
    pub software: bool,
}

/// Enumerates current DXGI adapters without changing display or driver settings.
pub fn adapters() -> Result<Vec<AdapterInfo>> {
    // SAFETY: COM outputs are owned by windows-rs; this is read-only adapter enumeration.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
    let mut found = Vec::new();
    for index in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(error),
        };
        found.push(describe(&adapter)?);
    }
    Ok(found)
}

fn describe(adapter: &IDXGIAdapter1) -> Result<AdapterInfo> {
    // SAFETY: the adapter owns its copied descriptor.
    let desc = unsafe { adapter.GetDesc1()? };
    Ok(AdapterInfo {
        id: AdapterId {
            low: desc.AdapterLuid.LowPart,
            high: desc.AdapterLuid.HighPart,
        },
        description: String::from_utf16_lossy(
            &desc.Description[..desc
                .Description
                .iter()
                .position(|&value| value == 0)
                .unwrap_or(desc.Description.len())],
        ),
        software: desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
    })
}

pub(super) struct Device {
    pub(super) device: ID3D11Device,
    pub(super) context: ID3D11DeviceContext,
    pub(super) adapter: AdapterId,
    _local: PhantomData<Rc<()>>,
}

impl Device {
    pub(super) fn new(id: AdapterId) -> Result<Self> {
        // SAFETY: exact LUID selection on a live factory, with no WARP/adapter fallback.
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1()? };
        let mut selected = None;
        for index in 0.. {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(error),
            };
            let info = describe(&adapter)?;
            if info.id == id {
                if info.software {
                    return Err(Error::from_hresult(DXGI_ERROR_UNSUPPORTED));
                }
                selected = Some(adapter);
                break;
            }
        }
        let adapter = selected.ok_or_else(|| Error::from_hresult(DXGI_ERROR_NOT_FOUND))?;
        let mut device = None;
        let mut context = None;
        // SAFETY: explicit retained hardware adapter and initialized owned output slots.
        unsafe {
            D3D11CreateDevice(
                &adapter,
                D3D_DRIVER_TYPE_UNKNOWN,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )?;
        }
        Ok(Self {
            device: device.ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?,
            context: context.ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?,
            adapter: id,
            _local: PhantomData,
        })
    }

    pub(super) fn removed(&self) -> bool {
        // SAFETY: a failure here reports device removal; ordinary output errors do not.
        unsafe { self.device.GetDeviceRemovedReason().is_err() }
    }
}
