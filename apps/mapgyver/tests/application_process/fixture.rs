use std::time::Duration;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--lifecycle-exit") {
        std::thread::sleep(Duration::from_millis(40));
        std::process::exit(23);
    }
    #[cfg(all(windows, feature = "native-qualification"))]
    if let Some(sender) = std::env::args().nth(1) {
        spout(sender);
        return;
    }
    // Finite, headless owned target. The parent test must stop it before this fallback deadline.
    std::thread::sleep(Duration::from_secs(20));
}

#[cfg(all(windows, feature = "native-qualification"))]
fn spout(sender: String) {
    use golden_video::native::{
        DiagnosticsConfig, adapters,
        fixtures::{CardStamp, FixtureSource, FixtureSourceConfig},
    };
    use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};
    use std::{num::NonZeroUsize, time::Instant};

    // Deliberate startup gap proves a resumed process alone cannot satisfy sender readiness.
    std::thread::sleep(Duration::from_millis(500));
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap()
        .id;
    let instance = uuid::Uuid::new_v4();
    let mut source = FixtureSource::new(FixtureSourceConfig {
        adapter,
        descriptor: TextureDescriptor {
            extent: Extent2D::new(128, 128).unwrap(),
            format: PixelFormat::Bgra8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        },
        source_id: 1,
        instance,
        sender_name: sender,
        send_access_timeout_ms: 0,
        diagnostics: DiagnosticsConfig {
            capacity: NonZeroUsize::new(64).unwrap(),
        },
    })
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut frame = 1;
    while Instant::now() < deadline {
        let mut qpc = 0;
        unsafe { windows::Win32::System::Performance::QueryPerformanceCounter(&mut qpc).unwrap() };
        source
            .submit(CardStamp {
                source: 1,
                frame,
                qpc: u64::try_from(qpc).unwrap(),
            })
            .unwrap();
        frame += 1;
        std::thread::sleep(Duration::from_millis(16));
    }
}
