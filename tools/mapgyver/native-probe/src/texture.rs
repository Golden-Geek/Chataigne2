//! The wrapper and every clone share one logical allocation lease.
use crate::ProbeResult;
use mapgyver_native_probe::allocation::{Lease, Ledger, Owner, bgra_bytes};
use std::ops::Deref;
use windows::Win32::Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;

#[derive(Clone)]
pub struct Texture {
    inner: ID3D11Texture2D,
    // COM reference drops before its ownership lease, including the last clone.
    _lease: Lease,
}

impl Texture {
    pub fn new(inner: ID3D11Texture2D, ledger: &Ledger, owner: Owner) -> ProbeResult<Self> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: owned interface and writable descriptor. The harness supports this exact format.
        unsafe {
            inner.GetDesc(&mut desc);
        }
        if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || desc.MipLevels != 1
            || desc.ArraySize != 1
            || desc.SampleDesc.Count != 1
        {
            return Err("logical accounting requires single-level, single-sample BGRA8 textures".into());
        }
        let lease = ledger.allocate(owner, bgra_bytes(desc.Width, desc.Height, 1)?, 1)?;
        Ok(Self { inner, _lease: lease })
    }
}

impl Deref for Texture {
    type Target = ID3D11Texture2D;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
