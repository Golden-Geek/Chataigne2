//! Finite external producer for actual-app checks; uses the independently qualified SDK sender.
use super::support::Sender;
use crate::Extent2D;
use crate::native::adapters;
use serde::Deserialize;
use std::io::{self, BufRead, Write};
use std::time::Duration;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    format: u32,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
    #[serde(default)]
    continuous: bool,
}

#[test]
#[ignore = "owned stdio native producer; requires explicit app-fixture adapter/name environment"]
fn owned_native_source_for_app_format_checks() {
    let index: usize = std::env::var("GOLDEN_VIDEO_APP_SOURCE_ADAPTER")
        .expect("explicit adapter required")
        .parse()
        .unwrap();
    let name = std::env::var("GOLDEN_VIDEO_APP_SOURCE_NAME").expect("explicit owned sender name required");
    assert!(name.starts_with("MapGyverFormat-") && name.len() < 128);
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .nth(index)
        .filter(|adapter| !adapter.software)
        .expect("hardware adapter absent");
    let (done, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            receiver.recv_timeout(Duration::from_secs(360)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned native source exceeded six minutes; shutdown unverified");
            std::process::exit(1);
        }
    });
    let mut live: Option<Sender> = None;
    let mut session = uuid::Uuid::nil();
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
                assert!((1..=8).contains(&frame.width) && (1..=8).contains(&frame.height));
                // The independent producer supports the canonical classes selected by this app check.
                // Exact pitches prevent malformed input from reaching UpdateSubresource's raw pointer.
                let pixel_bytes = match frame.format {
                    2..=4 => 16,
                    10 | 102 => 8,
                    28 | 29 | 87 | 88 | 91 | 93 | 100 | 101 => 4,
                    103 => 1,
                    104 | 105 => 2,
                    _ => panic!("unselected app-fixture format"),
                };
                let planar = matches!(frame.format, 103..=105);
                if planar {
                    assert!(frame.width.is_multiple_of(2) && frame.height.is_multiple_of(2));
                }
                let rows = frame.height + if planar { frame.height / 2 } else { 0 };
                let pitch = frame.width as usize * pixel_bytes;
                assert_eq!(frame.bytes.len(), pitch * rows as usize);
                let format = DXGI_FORMAT(frame.format as i32);
                let extent = Extent2D::new(frame.width, frame.height).unwrap();
                if frame.continuous
                    && let Some(sender) = live.as_mut()
                {
                    sender.update_rows(format, extent, &frame.bytes, pitch as u32);
                } else {
                    drop(live.take());
                    live = Some(Sender::from_rows(
                        adapter.id,
                        &name,
                        format,
                        extent,
                        &frame.bytes,
                        pitch as u32,
                    ));
                    session = uuid::Uuid::new_v4();
                }
                assert_eq!(
                    live.as_ref().unwrap().pixels(pixel_bytes),
                    frame.bytes,
                    "independent native source bits"
                );
                println!("OK started {} {} {} {session}", frame.format, frame.width, frame.height);
            }
        }
        io::stdout().flush().unwrap();
    }
    drop(live);
    done.send(()).unwrap();
}
