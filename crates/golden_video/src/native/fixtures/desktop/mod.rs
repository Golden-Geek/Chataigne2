//! Read-only qualification capture. Pixels and completion stamps share one native QPC clock.
mod window;
pub use window::DesktopGeometry;

use crate::native::{AdapterId, device::Device};
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION_IDENTITY};
use windows::Win32::Graphics::Dxgi::{
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_ERROR_WAS_STILL_DRAWING, DXGI_OUTDUPL_FRAME_INFO, IDXGIDevice, IDXGIOutput1,
    IDXGIOutputDuplication,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::core::{Error, HRESULT, Interface, Result};

#[derive(Clone, Copy, Debug)]
/// Exact capture endpoint and bounded read-only pixel region for one qualification lifetime.
pub struct DesktopCaptureConfig {
    /// Explicit capture adapter, independent of the application's rendering choice.
    pub adapter: AdapterId,
    /// Attached desktop output index on that adapter; no fallback enumeration.
    pub output: u32,
    /// Borrowed observed native window identity, reauthenticated before exporting pixels.
    pub hwnd: usize,
    /// Expected owner process; stale or recycled window identities reject.
    pub pid: u32,
    /// Physical desktop [left, top, right, bottom], bounded to256x32 pixels.
    pub region: [i32; 4],
}

/// One acquired desktop image's bounded pixels and native clock metadata.
pub struct DesktopFrame {
    /// Increasing capture-local identity, independent of renderer generations.
    pub serial: u64,
    /// QPC before AcquireNextFrame; not a display timestamp.
    pub acquire_begin_qpc: u64,
    /// QPC after pixel copy/unmap and the final ownership/geometry guard.
    pub readback_end_qpc: u64,
    /// OS desktop-image-update stamp; freshness metadata, never scanout proof.
    pub desktop_update_qpc: u64,
    /// OS-reported desktop updates accumulated since the previous acquired image.
    pub accumulated_frames: u32,
    /// Desktop-wide notice: Windows already blacked out some protected content. This does
    /// not identify this owned region; callers must verify its actual pixels independently.
    pub protected_content_masked_out: bool,
    /// Tightly packed BGRA8 pixels copied before the readback completion timestamp.
    pub bgra: Vec<u8>,
}

/// One thread-owned adapter/output/window lifetime. Capture loss invalidates the cohort;
/// callers must reauthenticate a fresh lifetime, never silently switch adapters or reuse stamps.
pub struct DesktopCapture {
    duplication: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
    gpu: Device,
    config: DesktopCaptureConfig,
    geometry: DesktopGeometry,
    source_box: D3D11_BOX,
    frequency: u64,
    serial: u64,
    _dpi: window::DpiScope,
}

pub(super) fn invalid(message: &str) -> Error {
    Error::new(HRESULT(0x8007_0057_u32 as i32), message)
}

fn dimensions(config: DesktopCaptureConfig) -> Result<(u32, u32)> {
    let [left, top, right, bottom] = config.region;
    let width = i64::from(right) - i64::from(left);
    let height = i64::from(bottom) - i64::from(top);
    if config.hwnd == 0 || config.pid == 0 || !(1..=256).contains(&width) || !(1..=32).contains(&height) {
        return Err(invalid("invalid owned capture identity/region"));
    }
    Ok((width as u32, height as u32))
}

fn qpc() -> Result<u64> {
    let mut value = 0;
    // SAFETY: initialized local output only.
    unsafe { QueryPerformanceCounter(&mut value)? };
    u64::try_from(value)
        .ok()
        .filter(|v| *v > 0)
        .ok_or_else(|| invalid("invalid capture QPC"))
}

impl DesktopCapture {
    /// Authenticate physical coordinates, owned foreground window and exact adapter/output.
    pub fn new(config: DesktopCaptureConfig) -> Result<Self> {
        let (width, height) = dimensions(config)?;
        let dpi = window::DpiScope::enter()?;
        let geometry = window::geometry(config)?;
        let gpu = Device::new(config.adapter)?;
        let device: IDXGIDevice = gpu.device.cast()?;
        // SAFETY: exact device adapter and bounded caller-selected output, no fallback or display mutation.
        let output = unsafe { device.GetAdapter()?.EnumOutputs(config.output)? };
        let desc = unsafe { output.GetDesc()? };
        let bounds = desc.DesktopCoordinates;
        if !desc.AttachedToDesktop.as_bool()
            || desc.Rotation != DXGI_MODE_ROTATION_IDENTITY
            || !window::contains([bounds.left, bounds.top, bounds.right, bounds.bottom], config.region)
        {
            return Err(invalid(
                "capture region must fit the selected attached unrotated output",
            ));
        }
        let output: IDXGIOutput1 = output.cast()?;
        let duplication = unsafe { output.DuplicateOutput(&gpu.device)? };
        let mut staging = None;
        unsafe {
            gpu.device.CreateTexture2D(
                &D3D11_TEXTURE2D_DESC {
                    Width: width,
                    Height: height,
                    MipLevels: 1,
                    ArraySize: 1,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    Usage: D3D11_USAGE_STAGING,
                    CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut staging),
            )?;
        }
        let mut frequency = 0;
        unsafe { QueryPerformanceFrequency(&mut frequency)? };
        let frequency = u64::try_from(frequency)
            .ok()
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("invalid QPC frequency"))?;
        Ok(Self {
            duplication,
            staging: staging.ok_or_else(|| invalid("missing capture staging texture"))?,
            gpu,
            config,
            geometry,
            source_box: D3D11_BOX {
                left: (i64::from(config.region[0]) - i64::from(bounds.left)) as u32,
                top: (i64::from(config.region[1]) - i64::from(bounds.top)) as u32,
                right: (i64::from(config.region[2]) - i64::from(bounds.left)) as u32,
                bottom: (i64::from(config.region[3]) - i64::from(bounds.top)) as u32,
                front: 0,
                back: 1,
            },
            frequency,
            serial: 0,
            _dpi: dpi,
        })
    }

    /// Initial authenticated geometry, required to remain unchanged for this lifetime.
    pub fn geometry(&self) -> DesktopGeometry {
        self.geometry
    }
    /// Native QPC ticks per second for every timestamp emitted by this capture owner.
    pub fn frequency(&self) -> u64 {
        self.frequency
    }
    /// Read QPC after revalidating ownership/geometry, suitable for a pre-input acknowledgement.
    pub fn stamp(&self) -> Result<u64> {
        self.validate()?;
        qpc()
    }
    fn validate(&self) -> Result<()> {
        if window::geometry(self.config)? != self.geometry {
            return Err(invalid("owned capture geometry/DPI changed"));
        }
        Ok(())
    }

    /// Acquire/copy one image with guards before and after; a16ms acquisition timeout returns None.
    /// Device/access loss invalidates this lifetime instead of changing endpoint or fabricating frames.
    pub fn next_frame(&mut self) -> Result<Option<DesktopFrame>> {
        self.validate()?;
        let begin = qpc()?;
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource = None;
        // SAFETY: one capture worker owns the duplication lifetime and releases every successful acquire.
        if let Err(error) = unsafe { self.duplication.AcquireNextFrame(16, &mut info, &mut resource) } {
            return if error.code() == DXGI_ERROR_WAIT_TIMEOUT {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let _lease = FrameLease(&self.duplication);
        let texture: ID3D11Texture2D = resource.ok_or_else(|| invalid("missing desktop frame"))?.cast()?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut desc) };
        if desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || self.source_box.right > desc.Width
            || self.source_box.bottom > desc.Height
        {
            return Err(invalid("unexpected desktop pixel format"));
        }
        unsafe {
            self.gpu
                .context
                .CopySubresourceRegion(&self.staging, 0, 0, 0, 0, &texture, 0, Some(&self.source_box))
        };
        // Submit this independent copy before polling; only successful Map proves completion.
        unsafe { self.gpu.context.Flush() };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match unsafe {
                self.gpu.context.Map(
                    &self.staging,
                    0,
                    D3D11_MAP_READ,
                    D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                    Some(&mut mapped),
                )
            } {
                Ok(()) => break,
                Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => return Err(error),
            }
        }
        let map = Mapped(&self.gpu.context, &self.staging);
        let (width, height) = dimensions(self.config)?;
        let row_bytes = width as usize * 4;
        if mapped.pData.is_null() || (mapped.RowPitch as usize) < row_bytes {
            return Err(invalid("invalid desktop readback pitch"));
        }
        let mut bgra = Vec::with_capacity(row_bytes * height as usize);
        for row in 0..height as usize {
            // SAFETY: matching bounded staging extent and checked native row pitch; copy only initialized region bytes.
            bgra.extend_from_slice(unsafe {
                std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(row * mapped.RowPitch as usize), row_bytes)
            });
        }
        drop(map);
        self.validate()?;
        let end = qpc()?;
        let present = u64::try_from(info.LastPresentTime).map_err(|_| invalid("negative desktop update QPC"))?;
        if present > end {
            return Err(invalid("desktop update timestamp exceeds readback completion"));
        }
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or_else(|| invalid("desktop frame serial exhausted"))?;
        Ok(Some(DesktopFrame {
            serial: self.serial,
            acquire_begin_qpc: begin,
            readback_end_qpc: end,
            desktop_update_qpc: present,
            accumulated_frames: info.AccumulatedFrames,
            protected_content_masked_out: info.ProtectedContentMaskedOut.as_bool(),
            bgra,
        }))
    }
}

struct FrameLease<'a>(&'a IDXGIOutputDuplication);
impl Drop for FrameLease<'_> {
    fn drop(&mut self) {
        unsafe {
            let _ = self.0.ReleaseFrame();
        }
    }
}
struct Mapped<'a>(&'a ID3D11DeviceContext, &'a ID3D11Texture2D);
impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        unsafe {
            self.0.Unmap(self.1, 0);
        }
    }
}

#[cfg(test)]
mod tests;
