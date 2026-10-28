use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::core::Interface;

use crate::ProbeResult;
use crate::texture::Texture;
use mapgyver_native_probe::allocation::{Ledger, Owner, Work};
use mapgyver_native_probe::resource_log::MemoryInfo;

pub struct Gpu {
    pub device: ID3D11Device,
    pub context: ID3D11DeviceContext,
    pub adapter: AdapterInfo,
    pub allocations: Ledger,
}

#[derive(Serialize)]
pub struct AdapterInfo {
    pub index: u32,
    pub description: String,
    pub luid: String,
    pub dedicated_video_bytes: usize,
    pub shared_system_bytes: usize,
    pub software: bool,
    pub local_memory: Option<MemoryInfo>,
    pub nonlocal_memory: Option<MemoryInfo>,
    pub outputs: Vec<OutputInfo>,
}

#[derive(Serialize)]
pub struct OutputInfo {
    device_name: String,
    attached: bool,
    desktop_rect: [i32; 4],
    coordinate_context: &'static str,
}

struct DpiScope(DPI_AWARENESS_CONTEXT);

impl DpiScope {
    fn enter() -> ProbeResult<Self> {
        // SAFETY: changes only the calling probe thread; restores its original context on drop.
        let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err(windows::core::Error::from_win32().into());
        }
        Ok(Self(previous))
    }
}

impl Drop for DpiScope {
    fn drop(&mut self) {
        // SAFETY: context was returned by the matching entry call on this thread.
        unsafe { SetThreadDpiAwarenessContext(self.0) };
    }
}

fn factory() -> windows::core::Result<IDXGIFactory1> {
    // SAFETY: COM output is constructed and owned by windows-rs.
    unsafe { CreateDXGIFactory1() }
}

pub fn inventory() -> ProbeResult<Vec<AdapterInfo>> {
    let factory = factory()?;
    let mut adapters = Vec::new();
    for index in 0.. {
        // SAFETY: enumerating the live factory, no borrowed raw output pointers.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(error.into()),
        };
        adapters.push(describe(index, &adapter, true)?);
    }
    Ok(adapters)
}

#[cfg(feature = "desktop")]
pub fn adapter_info(index: u32) -> ProbeResult<AdapterInfo> {
    // SAFETY: selected live factory/adapter; observations do not create a D3D device.
    let adapter = unsafe { factory()?.EnumAdapters1(index)? };
    describe(index, &adapter, true)
}

fn describe(index: u32, adapter: &IDXGIAdapter1, observe_memory: bool) -> ProbeResult<AdapterInfo> {
    let _dpi_scope = DpiScope::enter()?;
    // SAFETY: adapter is a live COM interface; returned structures are copied.
    let desc = unsafe { adapter.GetDesc1()? };
    let mut outputs = Vec::new();
    for output_index in 0.. {
        // SAFETY: enumeration does not alter the desktop configuration.
        let output = match unsafe { adapter.EnumOutputs(output_index) } {
            Ok(output) => output,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(error.into()),
        };
        // SAFETY: output owns its returned description.
        let desc = unsafe { output.GetDesc()? };
        let rect = desc.DesktopCoordinates;
        outputs.push(OutputInfo {
            device_name: utf16(&desc.DeviceName),
            attached: desc.AttachedToDesktop.as_bool(),
            desktop_rect: [rect.left, rect.top, rect.right, rect.bottom],
            coordinate_context: "physical_pixels_per_monitor_aware_v2_thread",
        });
    }
    let adapter3 = if observe_memory {
        adapter.cast::<IDXGIAdapter3>().ok()
    } else {
        None
    };
    Ok(AdapterInfo {
        index,
        description: utf16(&desc.Description),
        luid: format!(
            "{:08x}:{:08x}",
            desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
        ),
        dedicated_video_bytes: desc.DedicatedVideoMemory,
        shared_system_bytes: desc.SharedSystemMemory,
        software: desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0,
        local_memory: memory_info(adapter3.as_ref(), DXGI_MEMORY_SEGMENT_GROUP_LOCAL),
        nonlocal_memory: memory_info(adapter3.as_ref(), DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL),
        outputs,
    })
}

fn memory_info(adapter: Option<&IDXGIAdapter3>, segment: DXGI_MEMORY_SEGMENT_GROUP) -> Option<MemoryInfo> {
    let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
    // SAFETY: writable stack output, queried on node zero of this adapter.
    unsafe { adapter?.QueryVideoMemoryInfo(0, segment, &mut info).ok()? };
    Some(MemoryInfo {
        budget_bytes: info.Budget,
        current_usage_bytes: info.CurrentUsage,
    })
}

fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|c| *c == 0).unwrap_or(value.len())])
}

impl Gpu {
    pub fn retire(&self) -> ProbeResult<f64> {
        // SAFETY: called only after presenters/SDK/textures leave scope on this worker.
        unsafe {
            self.context.ClearState();
        }
        self.wait_submitted()
    }

    pub fn wait_submitted(&self) -> ProbeResult<f64> {
        let serial = self.allocations.submitted();
        let start = Instant::now();
        let mut completion = crate::completion::Completion::new(self)?;
        let value = completion.submit()?;
        completion.wait(value)?;
        self.allocations.complete(serial);
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    /// Explicit event-query isolation control; runtime completion uses a timeline fence.
    #[cfg(feature = "desktop")]
    pub fn wait_submitted_event(&self) -> ProbeResult<f64> {
        let serial = self.allocations.submitted();
        let start = Instant::now();
        let mut done = None;
        // SAFETY: fence this worker's submitted commands without mapping any image pixels.
        unsafe {
            self.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut done),
            )?;
            self.context.End(done.as_ref().ok_or("missing retirement fence")?);
            self.context.Flush();
        }
        while !crate::query_data::event(&self.context, done.as_ref().ok_or("missing retirement fence")?)? {
            if start.elapsed() >= Duration::from_secs(2) {
                return Err("submitted GPU work exceeded two-second worker deadline".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.allocations.complete(serial);
        Ok(start.elapsed().as_secs_f64() * 1000.0)
    }

    pub fn memory(&self) -> ProbeResult<[Option<MemoryInfo>; 2]> {
        let device = self.device.cast::<IDXGIDevice>()?;
        // SAFETY: this live device retains its adapter.
        let adapter = unsafe { device.GetAdapter()? }.cast::<IDXGIAdapter3>().ok();
        Ok([
            memory_info(adapter.as_ref(), DXGI_MEMORY_SEGMENT_GROUP_LOCAL),
            memory_info(adapter.as_ref(), DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL),
        ])
    }

    pub fn new(index: u32) -> ProbeResult<Self> {
        Self::with_memory_observation(index, true)
    }

    pub fn with_memory_observation(index: u32, observe_memory: bool) -> ProbeResult<Self> {
        // SAFETY: factory is live; an explicit adapter forbids silent WARP fallback.
        let adapter = unsafe { factory()?.EnumAdapters1(index)? };
        let info = describe(index, &adapter, observe_memory)?;
        if info.software {
            return Err("software adapter cannot qualify hardware texture sharing".into());
        }
        let mut device = None;
        let mut context = None;
        // SAFETY: output slots are valid; SDK version/flags match D3D11, adapter is retained.
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
            device: device.ok_or("D3D11 returned no device")?,
            context: context.ok_or("D3D11 returned no context")?,
            adapter: info,
            allocations: Ledger::default(),
        })
    }

    pub fn texture(&self, width: u32, height: u32) -> ProbeResult<Texture> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            ..Default::default()
        };
        self.create_texture(&desc)
    }

    fn create_texture(&self, desc: &D3D11_TEXTURE2D_DESC) -> ProbeResult<Texture> {
        let mut texture = None;
        // SAFETY: validated D3D11 descriptor; no initial CPU data is uploaded.
        unsafe { self.device.CreateTexture2D(desc, None, Some(&mut texture))? };
        Texture::new(
            texture.ok_or("D3D11 returned no texture")?,
            &self.allocations,
            Owner::Application,
        )
    }

    pub fn clear(&self, texture: &ID3D11Texture2D, color: [f32; 4]) -> ProbeResult<()> {
        self.allocations.work(Work::Clear);
        let mut view = None;
        // SAFETY: texture was created on this device with render-target binding.
        unsafe {
            self.device.CreateRenderTargetView(texture, None, Some(&mut view))?;
            self.context
                .ClearRenderTargetView(view.as_ref().ok_or("missing render target")?, &color);
        }
        Ok(())
    }

    /// Diagnostic-only, one readback at each lifecycle phase; never the render path.
    pub fn read_marker(&self, texture: &ID3D11Texture2D) -> ProbeResult<[u8; 4]> {
        Ok(self.read_samples(texture, &[[0, 0]])?[0])
    }

    /// Copies at most sixteen isolated pixels to a single staging row, not a whole frame.
    pub fn read_samples(&self, texture: &ID3D11Texture2D, points: &[[u32; 2]]) -> ProbeResult<Vec<[u8; 4]>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: output descriptor is writable; borrowed texture remains live through this call.
        unsafe { texture.GetDesc(&mut desc) };
        if points.is_empty()
            || points.len() > 16
            || desc.Format != DXGI_FORMAT_B8G8R8A8_UNORM
            || points
                .iter()
                .any(|point| point[0] >= desc.Width || point[1] >= desc.Height)
        {
            return Err("invalid diagnostic BGRA sample request".into());
        }
        desc.Width = points.len() as u32;
        desc.Height = 1;
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.MiscFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        let staging = self.create_texture(&desc)?;
        self.allocations.work(Work::Readback(points.len() as u64));
        let mut query = None;
        // SAFETY: all resources belong to this device and are owned until completion below.
        unsafe {
            self.device.CreateQuery(
                &D3D11_QUERY_DESC {
                    Query: D3D11_QUERY_EVENT,
                    MiscFlags: 0,
                },
                Some(&mut query),
            )?;
            for (index, [x, y]) in points.iter().copied().enumerate() {
                self.allocations.work(Work::GpuCopy(4));
                self.context.CopySubresourceRegion(
                    &*staging,
                    0,
                    index as u32,
                    0,
                    0,
                    texture,
                    0,
                    Some(&D3D11_BOX {
                        left: x,
                        top: y,
                        front: 0,
                        right: x + 1,
                        bottom: y + 1,
                        back: 1,
                    }),
                );
            }
        }
        let query = query.ok_or("D3D11 returned no completion query")?;
        let serial = self.allocations.submitted();
        // SAFETY: query is a valid event query; flush submits the preceding copy.
        unsafe {
            self.context.End(&query);
            self.context.Flush();
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if crate::query_data::event(&self.context, &query)? {
                break;
            }
            if Instant::now() >= deadline {
                return Err("GPU diagnostic readback timed out".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        self.allocations.complete(serial);
        // SAFETY: query completed; samples occupy the first staging row, whose pitch is checked.
        let samples = unsafe {
            self.context.Map(&*staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
            if mapped.pData.is_null() || (mapped.RowPitch as usize) < points.len() * 4 {
                self.context.Unmap(&*staging, 0);
                return Err("invalid diagnostic staging map".into());
            }
            let mut samples = vec![[0_u8; 4]; points.len()];
            std::ptr::copy_nonoverlapping(
                mapped.pData.cast::<u8>(),
                samples.as_mut_ptr().cast::<u8>(),
                points.len() * 4,
            );
            self.context.Unmap(&*staging, 0);
            samples
        };
        Ok(samples)
    }
}
