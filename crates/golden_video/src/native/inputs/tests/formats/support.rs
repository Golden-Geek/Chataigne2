use crate::native::{AdapterId, completion::EventCompletion, device::Device, texture::Texture};
use crate::runtime::{CompletionToken, DeviceGeneration};
use spout2_sys as sdk;
use std::{
    ffi::CString,
    ptr::NonNull,
    time::{Duration, Instant},
};
use windows::{
    Win32::Graphics::{
        Direct3D11::*,
        Dxgi::{Common::*, DXGI_ERROR_WAS_STILL_DRAWING},
    },
    core::Interface,
};

pub(super) fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(6);
    while !ready() {
        assert!(Instant::now() < deadline, "{label}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A failed assertion never releases shared-reader ownership before submitted GPU uses finish.
pub(super) struct GpuUse<'a> {
    device: &'a Device,
    completion: EventCompletion,
    token: CompletionToken,
    submitted: bool,
}
impl<'a> GpuUse<'a> {
    pub fn new(device: &'a Device) -> Self {
        Self {
            device,
            completion: EventCompletion::new(device).unwrap(),
            token: CompletionToken {
                device: DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
                serial: 1,
            },
            submitted: false,
        }
    }
    pub fn submit(&mut self, work: impl FnOnce()) {
        assert!(!self.submitted, "one finite GPU submission per retained use");
        self.submitted = true;
        let _submission = self.completion.begin(self.device, self.token);
        work();
    }
    pub fn wait(&mut self) {
        assert!(self.submitted);
        wait("finite source GPU use must complete", || {
            self.completion
                .completed(self.device, self.token.device)
                .unwrap()
                .serial
                >= self.token.serial
        });
    }
}
impl Drop for GpuUse<'_> {
    fn drop(&mut self) {
        while self.submitted
            && !self.device.removed()
            && !self
                .completion
                .completed(self.device, self.token.device)
                .is_ok_and(|done| done.serial >= self.token.serial)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

struct Sdk(NonNull<sdk::spout_dx_t>);
impl Drop for Sdk {
    fn drop(&mut self) {
        unsafe { sdk::spout_dx_destroy(self.0.as_ptr()) };
    }
}

/// Independent D3D11 Clear + SDK SendTexture; no Golden render/format conversion generates the oracle.
pub(super) struct Sender {
    sdk: Sdk,
    texture: ID3D11Texture2D,
    device: Device,
    completion: EventCompletion,
    token: CompletionToken,
}
impl Sender {
    pub fn pixels(&self, pixel_bytes: usize) -> Vec<u8> {
        readback_native(&self.device, &self.texture, pixel_bytes)
    }

    pub fn new(adapter: AdapterId, name: &str, format: DXGI_FORMAT, color: [f32; 4]) -> Self {
        let device = Device::new(adapter).unwrap();
        let mut texture = None;
        let mut rtv = None;
        unsafe {
            device
                .device
                .CreateTexture2D(
                    &D3D11_TEXTURE2D_DESC {
                        Width: 2,
                        Height: 2,
                        MipLevels: 1,
                        ArraySize: 1,
                        Format: format,
                        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                        Usage: D3D11_USAGE_DEFAULT,
                        BindFlags: (D3D11_BIND_SHADER_RESOURCE | D3D11_BIND_RENDER_TARGET).0 as u32,
                        ..Default::default()
                    },
                    None,
                    Some(&mut texture),
                )
                .unwrap();
        }
        let texture = texture.unwrap();
        unsafe {
            device
                .device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))
                .unwrap()
        };
        let sdk = Sdk(NonNull::new(unsafe { sdk::spout_dx_create() }).unwrap());
        let name = CString::new(name).unwrap();
        unsafe {
            assert_ne!(
                sdk::spout_dx_set_frame_count_mode(sdk.0.as_ptr(), sdk::SPOUT_DX_FRAME_COUNT_SENDER_LOCAL),
                0
            );
            assert_ne!(sdk::spout_dx_open_directx11(sdk.0.as_ptr(), device.device.as_raw()), 0);
            assert_ne!(sdk::spout_dx_set_sender_name(sdk.0.as_ptr(), name.as_ptr()), 0);
        }
        let completion = EventCompletion::new(&device).unwrap();
        let token = CompletionToken {
            device: DeviceGeneration::new(uuid::Uuid::new_v4(), 0),
            serial: 1,
        };
        let mut sender = Self {
            sdk,
            texture,
            device,
            completion,
            token,
        };
        {
            let _submission = sender.completion.begin(&sender.device, token);
            unsafe {
                sender.device.context.ClearRenderTargetView(&rtv.unwrap(), &color);
                assert_eq!(
                    sdk::spout_dx_send_texture_status(sender.sdk.0.as_ptr(), sender.texture.as_raw(), 0),
                    sdk::SPOUT_DX_TRANSFER_COPIED
                );
            }
        }
        wait("independent SDK source publication must complete", || {
            sender
                .completion
                .completed(&sender.device, token.device)
                .unwrap()
                .serial
                >= 1
        });
        sender
    }
}
impl Drop for Sender {
    fn drop(&mut self) {
        while !self.device.removed()
            && !self
                .completion
                .completed(&self.device, self.token.device)
                .is_ok_and(|done| done.serial >= 1)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(super) fn readback(device: &Device, texture: &Texture, pixel_bytes: usize) -> Vec<u8> {
    readback_native(device, &texture.texture, pixel_bytes)
}

fn readback_native(device: &Device, texture: &ID3D11Texture2D, pixel_bytes: usize) -> Vec<u8> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.GetDesc(&mut desc) };
    assert!(desc.Width <= 8 && desc.Height <= 8);
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.MiscFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    unsafe { device.device.CreateTexture2D(&desc, None, Some(&mut staging)).unwrap() };
    let staging = staging.unwrap();
    let mut completion = GpuUse::new(device);
    completion.submit(|| unsafe { device.context.CopyResource(&staging, texture) });
    completion.wait();
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    wait("completed native readback Map must become ready", || {
        match unsafe {
            device.context.Map(
                &staging,
                0,
                D3D11_MAP_READ,
                D3D11_MAP_FLAG_DO_NOT_WAIT.0 as u32,
                Some(&mut mapped),
            )
        } {
            Ok(()) => true,
            Err(error) if error.code() == DXGI_ERROR_WAS_STILL_DRAWING => false,
            Err(error) => panic!("native readback Map failed: {error}"),
        }
    });
    struct Mapped<'a>(&'a ID3D11DeviceContext, &'a ID3D11Texture2D);
    impl Drop for Mapped<'_> {
        fn drop(&mut self) {
            unsafe { self.0.Unmap(self.1, 0) };
        }
    }
    let _map = Mapped(&device.context, &staging);
    let stride = desc.Width as usize * pixel_bytes;
    assert!(!mapped.pData.is_null() && mapped.RowPitch as usize >= stride);
    let mut bytes = Vec::with_capacity(stride * desc.Height as usize);
    for y in 0..desc.Height as usize {
        bytes.extend_from_slice(unsafe {
            std::slice::from_raw_parts(mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize), stride)
        });
    }
    bytes
}
