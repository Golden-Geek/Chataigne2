use std::marker::PhantomData;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::rc::Rc;

use windows::Win32::Foundation::{DXGI_STATUS_OCCLUDED, HANDLE, HWND, RECT, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, IsIconic, IsWindowVisible};
use windows::core::{Error, HRESULT, Interface, Result};

use crate::TextureDescriptor;

use super::super::config::WindowTarget;
use super::super::texture::Texture;
use super::super::timing::{CpuInterval, Recorder};
use super::Delivery;

/// Fixed admitted backing size; DXGI stretches it when the host's physical client size changes.
pub(super) struct WindowOutput {
    // DXGI transfers ownership of this handle. Close it before releasing the chain.
    readiness: Readiness,
    chain: IDXGISwapChain2,
    window: HWND,
    descriptor: TextureDescriptor,
    device: ID3D11Device,
    addresses: Option<[u64; 3]>,
    _local: PhantomData<Rc<()>>,
}

impl WindowOutput {
    pub(super) fn new(
        device: &ID3D11Device,
        window: WindowTarget,
        descriptor: TextureDescriptor,
        diagnostics: bool,
    ) -> Result<Self> {
        let window = HWND(window.raw() as *mut _);
        let dxgi: IDXGIDevice = device.cast()?;
        // SAFETY: interfaces belong to the retained caller-selected D3D11 device.
        let factory: IDXGIFactory2 = unsafe { dxgi.GetAdapter()?.GetParent()? };
        // SAFETY: the host owns this HWND and leases it through actual runtime retirement.
        // The fixed descriptor was budgeted before this pool is initialized.
        let chain = unsafe {
            factory.CreateSwapChainForHwnd(
                device,
                window,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: descriptor.extent.width(),
                    Height: descriptor.extent.height(),
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                    AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                    Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
                    ..Default::default()
                },
                None,
                None,
            )?
        }
        .cast::<IDXGISwapChain2>()?;
        // SAFETY: only this owned swapchain is configured. The returned wait handle belongs
        // to the caller and is kept worker-local rather than sent into control metadata.
        let readiness = unsafe {
            chain.SetMaximumFrameLatency(1)?;
            if chain.GetMaximumFrameLatency()? != 1 {
                return Err(Error::new(
                    HRESULT(0x8000_4005_u32 as i32),
                    "DXGI did not retain maximum frame latency one",
                ));
            }
            let handle = chain.GetFrameLatencyWaitableObject();
            if handle.is_invalid() {
                return Err(Error::new(
                    HRESULT(0x8000_4005_u32 as i32),
                    "DXGI did not provide a frame-latency handle",
                ));
            }
            OwnedHandle::from_raw_handle(handle.0)
        };
        let addresses = if diagnostics {
            Some([
                chain.cast::<IDXGISwapChain>()?.as_raw() as usize as u64,
                chain.cast::<IDXGISwapChain1>()?.as_raw() as usize as u64,
                chain.as_raw() as usize as u64,
            ])
        } else {
            None
        };
        Ok(Self {
            readiness: Readiness::new(readiness),
            chain,
            window,
            descriptor,
            device: device.clone(),
            addresses,
            _local: PhantomData,
        })
    }

    pub(super) fn present_identity(&self, generation: u64) -> Option<super::super::timing::WindowPresentIdentity> {
        self.addresses
            .map(|addresses| super::super::timing::WindowPresentIdentity { generation, addresses })
    }

    pub(super) fn admit(&mut self) -> std::result::Result<(), Delivery> {
        let available = self.available().map_err(|error| Delivery::Failed(error.code()))?;
        self.readiness.admit(available)
    }

    fn available(&self) -> Result<bool> {
        let mut rect = RECT::default();
        // SAFETY: this lease's host-owned HWND is retained until the renderer has retired.
        unsafe {
            GetClientRect(self.window, &mut rect)?;
            Ok(!IsIconic(self.window).as_bool()
                && IsWindowVisible(self.window).as_bool()
                && rect.right > rect.left
                && rect.bottom > rect.top)
        }
    }

    pub(super) fn deliver(
        &mut self,
        context: &ID3D11DeviceContext,
        texture: &Texture,
        recorder: Option<&Recorder>,
        consumer_api: &mut Option<CpuInterval>,
    ) -> Delivery {
        match self.try_deliver(context, texture, recorder, consumer_api) {
            Ok(delivery) => delivery,
            Err(error) => Delivery::Failed(error.code()),
        }
    }

    fn try_deliver(
        &mut self,
        context: &ID3D11DeviceContext,
        texture: &Texture,
        recorder: Option<&Recorder>,
        consumer_api: &mut Option<CpuInterval>,
    ) -> Result<Delivery> {
        debug_assert!(self.readiness.permit, "delivery requires an admitted frame permit");
        // SAFETY: all interfaces remain on this worker. The native texture wrapper validates
        // storage, while explicit pointer checks reject accidental cross-device submission.
        unsafe {
            if texture.descriptor != self.descriptor
                || texture.texture.GetDevice()?.as_raw() != self.device.as_raw()
                || context.GetDevice()?.as_raw() != self.device.as_raw()
            {
                return Err(Error::new(
                    HRESULT(0x8007_0057_u32 as i32),
                    "window output texture/context does not match its admitted descriptor/device",
                ));
            }
            let target: ID3D11Texture2D = self.chain.GetBuffer(0)?;
            context.CopyResource(&target, &texture.texture);
            let begin = recorder.and_then(Recorder::stamp);
            let result = self.chain.Present(0, DXGI_PRESENT_DO_NOT_WAIT);
            *consumer_api = recorder.and_then(|recorder| recorder.interval(begin));
            Ok(self.readiness.presented(result))
        }
    }
}

/// One acquired frame-latency signal, retained until an actual presentation consumes it.
pub(super) struct Readiness {
    handle: OwnedHandle,
    permit: bool,
}

impl Readiness {
    pub(super) fn new(handle: OwnedHandle) -> Self {
        Self { handle, permit: false }
    }

    pub(super) fn admit(&mut self, available: bool) -> std::result::Result<(), Delivery> {
        if !available {
            return Err(Delivery::Unavailable);
        }
        if self.permit {
            return Ok(());
        }
        // SAFETY: the owned waitable handle remains retained. Zero timeout never stalls
        // another output, and the successful signal is consumed only once for this permit.
        match unsafe { WaitForSingleObject(HANDLE(self.handle.as_raw_handle()), 0) } {
            WAIT_OBJECT_0 => {
                self.permit = true;
                Ok(())
            }
            WAIT_TIMEOUT => Err(Delivery::Busy),
            WAIT_FAILED => Err(Delivery::Failed(Error::from_win32().code())),
            _ => Err(Delivery::Failed(HRESULT(0x8000_4005_u32 as i32))),
        }
    }

    pub(super) fn presented(&mut self, result: HRESULT) -> Delivery {
        if result == DXGI_ERROR_WAS_STILL_DRAWING {
            return Delivery::Busy;
        }
        self.permit = false;
        if result == DXGI_STATUS_OCCLUDED {
            return Delivery::Unavailable;
        }
        match result.ok() {
            Ok(()) => Delivery::Accepted,
            Err(error) => Delivery::Failed(error.code()),
        }
    }
}
