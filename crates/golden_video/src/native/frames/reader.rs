use super::GpuImage;
use crate::native::{
    device::Device,
    inputs::{reader::Imported, surface::acquire},
    texture::Texture,
};
use std::{collections::BTreeMap, rc::Rc, sync::Arc};
use windows::core::{Error, HRESULT, Result};

#[derive(Default)]
pub(in crate::native) struct Imports(BTreeMap<usize, Rc<Imported>>);
pub(in crate::native) struct ReadLease {
    imported: Rc<Imported>,
    image: GpuImage,
    // Notify after both the COM import and strong slot lease have been released.
    _wake: Wake,
}
struct Wake(Arc<super::Fault>);
impl Drop for Wake {
    fn drop(&mut self) {
        self.0.wake();
    }
}
impl Imports {
    pub(in crate::native) fn prune(&mut self) {
        self.0.retain(|_, imported| imported.alive());
    }
    pub fn acquire(&mut self, device: &Device, target: &Texture, image: GpuImage) -> Result<Option<ReadLease>> {
        if device.adapter != image.adapter || target.descriptor != image.descriptor() {
            return Err(Error::from_hresult(HRESULT(0x8007_0057_u32 as i32)));
        }
        // Imported owns a weak surface, so unused generations cannot keep source slots busy.
        self.prune();
        let identity = Arc::as_ptr(&image.surface) as usize;
        let imported = match self.0.get(&identity) {
            Some(imported) if imported.same_surface(&image.surface) => imported.clone(),
            _ => {
                let imported = Rc::new(Imported::open(device, &image.surface)?);
                self.0.insert(identity, imported.clone());
                imported
            }
        };
        if !acquire(&imported.mutex, 1)? && !acquire(&imported.mutex, 0)? {
            return Ok(None);
        }
        let wake = Wake(image.fault.clone());
        Ok(Some(ReadLease {
            imported,
            image,
            _wake: wake,
        }))
    }
}
impl ReadLease {
    pub(in crate::native) fn copy_to(&self, device: &Device, target: &Texture) {
        self.imported.copy_to(device, target);
    }
}
impl Drop for ReadLease {
    fn drop(&mut self) {
        // Only matching renderer completion (or confirmed removal) releases this lease.
        if let Err(error) = unsafe { self.imported.mutex.ReleaseSync(0) } {
            self.image.fail(error.code().0);
        }
    }
}
