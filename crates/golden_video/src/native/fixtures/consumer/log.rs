use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R32G32B32A32_UINT, DXGI_SAMPLE_DESC};
use windows::core::{Error, HRESULT, Result};

const INVALID_ARGUMENT: HRESULT = HRESULT(0x80070057_u32 as i32);
const WORDS: usize = 12;
pub(super) const ROW_BYTES: u64 = (WORDS * size_of::<u32>()) as u64;
const PAGE_HEIGHT: u32 = 16384;

#[derive(Clone, Copy)]
struct Layout {
    capacity: u32,
    width: u32,
    height: u32,
}

impl Layout {
    fn new(capacity: u32) -> Result<Self> {
        if capacity == 0 || capacity > 65536 {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        Ok(Self {
            capacity,
            width: 3 * capacity.div_ceil(PAGE_HEIGHT),
            height: capacity.min(PAGE_HEIGHT),
        })
    }
    fn origin(self, row: u32) -> Option<[u32; 2]> {
        (row < self.capacity).then_some([3 * (row / PAGE_HEIGHT), row % PAGE_HEIGHT])
    }
    fn bytes(self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * (ROW_BYTES / 3)
    }
    fn offset(self, row: u32, pitch: u32) -> Option<usize> {
        let [x, y] = self.origin(row)?;
        (pitch as usize).checked_mul(y as usize)?.checked_add(x as usize * 16)
    }
}

/// Append-only integer GPU rows and one equally bounded, initially unmapped final staging copy.
pub(in crate::native) struct GpuLog {
    pub target: ID3D11RenderTargetView,
    pub texture: ID3D11Texture2D,
    pub staging: ID3D11Texture2D,
    layout: Layout,
}

impl GpuLog {
    pub fn new(device: &ID3D11Device, capacity: u32) -> Result<Self> {
        let layout = Layout::new(capacity)?;
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: layout.width,
            Height: layout.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R32G32B32A32_UINT,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..Default::default()
        };
        let mut texture = None;
        let mut staging = None;
        let mut target = None;
        // SAFETY: fixed single-level integer log and CPU-read staging allocation. No native
        // commands are submitted during construction, and neither texture is CPU initialized.
        unsafe {
            device.CreateTexture2D(&descriptor, None, Some(&mut texture))?;
            device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Usage: D3D11_USAGE_STAGING,
                    BindFlags: 0,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    ..descriptor
                },
                None,
                Some(&mut staging),
            )?;
            device.CreateRenderTargetView(texture.as_ref().ok_or_else(missing)?, None, Some(&mut target))?;
        }
        Ok(Self {
            texture: texture.ok_or_else(missing)?,
            staging: staging.ok_or_else(missing)?,
            target: target.ok_or_else(missing)?,
            layout,
        })
    }

    pub fn origin(&self, row: u32) -> [u32; 2] {
        self.layout.origin(row).expect("admitted log row")
    }
    pub fn bytes(&self) -> u64 {
        self.layout.bytes()
    }

    /// Caller has fenced the one final CopyResource. Only written rows are inspected, using
    /// the driver RowPitch rather than assuming the staging texture is tightly packed.
    pub fn read(
        &self,
        context: &ID3D11DeviceContext,
        count: u32,
        mut row: impl FnMut(u32, [u32; WORDS]),
    ) -> Result<()> {
        if count > self.layout.capacity {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: the caller proves actual completion of the final copy before this only Map.
        unsafe {
            context.Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        }
        let _mapping = Mapping {
            context,
            texture: &self.staging,
        };
        if mapped.pData.is_null() || mapped.RowPitch < self.layout.width * 16 {
            return Err(Error::from_hresult(INVALID_ARGUMENT));
        }
        for index in 0..count {
            let offset = self
                .layout
                .offset(index, mapped.RowPitch)
                .ok_or_else(|| Error::from_hresult(INVALID_ARGUMENT))?;
            let words = std::array::from_fn(|word| {
                // SAFETY: each complete row contains twelve u32 words; count is bounded by
                // this allocation's height and the driver-provided RowPitch was validated.
                unsafe { std::ptr::read_unaligned(mapped.pData.cast::<u8>().add(offset + word * 4).cast::<u32>()) }
            });
            row(index, words);
        }
        Ok(())
    }
}

struct Mapping<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}
impl Drop for Mapping<'_> {
    fn drop(&mut self) {
        // SAFETY: exactly this successful Map owns the matching Unmap, including errors.
        unsafe {
            self.context.Unmap(self.texture, 0);
        }
    }
}

fn missing() -> Error {
    Error::from_hresult(HRESULT(0x80004005_u32 as i32))
}

#[cfg(test)]
mod tests;
