//! Actual D3D9Ex-owned texture producer, using only the SDK binding's public fixture API.
use super::source_fixture::{Frame, environment};
use crate::native::device::Device;
use spout2_sys as sdk;
use std::{
    ffi::CString,
    io::{self, BufRead, Write},
    ptr::NonNull,
};
use windows::Win32::{
    Foundation::HANDLE,
    Graphics::Direct3D11::{D3D11_TEXTURE2D_DESC, ID3D11Texture2D},
};

struct Source(NonNull<sdk::spout_dx9_fixture_t>);
impl Drop for Source {
    fn drop(&mut self) {
        unsafe { sdk::spout_dx9_fixture_destroy(self.0.as_ptr()) };
    }
}

#[test]
#[ignore = "explicit actual D3D9Ex producer; requires owned app-fixture adapter/name environment"]
fn owned_d3d9_source_for_app_format_checks() {
    let (adapter, name, done) = environment();
    let name = CString::new(name).unwrap();
    let mut live = None;
    println!("OK ready");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        match line.as_str() {
            "exit" => {
                drop(live.take());
                println!("OK exit");
                break;
            }
            "stop" => {
                drop(live.take());
                println!("OK stopped");
            }
            _ => {
                assert!(line.len() <= 8192);
                let frame: Frame = serde_json::from_str(&line).unwrap();
                assert!(!frame.continuous, "D3D9 fixture explicitly replaces producers");
                assert!((1..=8).contains(&frame.width) && (1..=8).contains(&frame.height));
                let bytes_per_pixel = match frame.format {
                    21 | 22 | 31 => 4,
                    113 => 8,
                    _ => panic!("unselected D3D9 fixture format"),
                };
                let pitch = frame.width * bytes_per_pixel;
                assert_eq!(frame.bytes.len(), (pitch * frame.height) as usize);
                drop(live.take());
                let mut error = 0;
                let source = unsafe {
                    sdk::spout_dx9_fixture_create(
                        adapter.id.low,
                        adapter.id.high,
                        name.as_ptr(),
                        frame.format,
                        frame.width,
                        frame.height,
                        frame.bytes.as_ptr(),
                        frame.bytes.len(),
                        pitch,
                        &mut error,
                    )
                };
                live = Some(Source(NonNull::new(source).unwrap_or_else(|| {
                    panic!(
                        "D3D9 source format={} adapter={:?} failed HRESULT=0x{:08x}",
                        frame.format, adapter.id, error as u32
                    )
                })));
                assert_eq!(error, 0);
                // Read-only cross-API alias observation; source allocation/upload stays D3D9.
                let observer = Device::new(adapter.id).unwrap();
                let mut alias: Option<ID3D11Texture2D> = None;
                let mut desc = D3D11_TEXTURE2D_DESC::default();
                unsafe {
                    observer
                        .device
                        .OpenSharedResource(
                            HANDLE(sdk::spout_dx9_fixture_share_handle(live.as_ref().unwrap().0.as_ptr())),
                            &mut alias,
                        )
                        .unwrap();
                    alias.as_ref().unwrap().GetDesc(&mut desc);
                }
                assert_eq!((desc.Width, desc.Height), (frame.width, frame.height));
                eprintln!(
                    "DX9_SOURCE_NATIVE {}",
                    serde_json::json!({
                        "d3d9_format": frame.format, "native_dxgi_format": desc.Format.0,
                        "width": desc.Width, "height": desc.Height, "source_bits_verified": true,
                    })
                );
                println!(
                    "OK started {} {} {} {}",
                    frame.format,
                    frame.width,
                    frame.height,
                    uuid::Uuid::new_v4()
                );
            }
        }
        io::stdout().flush().unwrap();
    }
    drop(live);
    done.send(()).unwrap();
}
