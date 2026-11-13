use std::os::windows::io::AsRawHandle;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device1, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::IDXGIKeyedMutex;
use windows::core::{Interface, Result};

use super::super::{device::Device, texture::Texture};
use super::shared::Shared;
use super::surface::{Surface, acquire};

pub(in crate::native) struct Imported {
    pub texture: Texture,
    conversion: Option<super::conversion::Converter>,
    pub(in crate::native) mutex: IDXGIKeyedMutex,
    surface: Weak<Surface>,
    // Release native views/texture before returning admission, even after the NT handle closes.
    _lease: Option<Arc<dyn Send + Sync>>,
}

impl Imported {
    pub(in crate::native) fn alive(&self) -> bool {
        self.surface.strong_count() != 0
    }
    pub fn open(device: &Device, surface: &Arc<Surface>) -> Result<Self> {
        let device1: ID3D11Device1 = device.device.cast()?;
        // SAFETY: this strong surface keeps its owned NT handle alive through import.
        let texture: ID3D11Texture2D = unsafe { device1.OpenSharedResource1(HANDLE(surface.handle.as_raw_handle()))? };
        Ok(Self {
            mutex: texture.cast()?,
            texture: Texture::from_native(&device.device, texture, surface.storage_descriptor)?,
            conversion: (surface.storage_descriptor != surface.descriptor)
                .then(|| super::conversion::Converter::new(&device.device, surface.descriptor.format))
                .transpose()?,
            surface: Arc::downgrade(surface),
            _lease: surface._lease.clone(),
        })
    }

    pub fn same_surface(&self, surface: &Arc<Surface>) -> bool {
        self.surface.ptr_eq(&Arc::downgrade(surface))
    }

    pub fn copy_to(&self, device: &Device, target: &Texture) {
        if let Some(conversion) = &self.conversion {
            conversion.copy(&device.context, &self.texture, target);
        } else {
            // SAFETY: publication descriptor/target equality was checked before acquisition.
            unsafe { device.context.CopyResource(&target.texture, &self.texture.texture) };
        }
    }
}

pub(super) struct Reader {
    pub imported: Rc<Imported>,
    _surface: Arc<Surface>,
    shared: Arc<Shared>,
}

impl Reader {
    pub fn acquire(imported: Rc<Imported>, surface: Arc<Surface>, shared: Arc<Shared>) -> Result<Option<Self>> {
        // A strong publication lease prevents writer reuse. A previously consumed static
        // publication may already be at key zero; new plan generations must still copy it.
        if !acquire(&imported.mutex, 1)? && !acquire(&imported.mutex, 0)? {
            return Ok(None);
        }
        Ok(Some(Self {
            imported,
            _surface: surface,
            shared,
        }))
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        // SAFETY: only a successful AcquireSync constructs a Reader; core completion precedes
        // normal destruction. The surface lease is released after this native handoff.
        if let Err(error) = unsafe { self.imported.mutex.ReleaseSync(0) } {
            self.shared.record_fault(error.code().0);
        }
    }
}
