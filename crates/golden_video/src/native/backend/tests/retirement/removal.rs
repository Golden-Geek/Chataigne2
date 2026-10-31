//! Prepares for an explicitly approved external GPU reset; this test never triggers one itself.
use super::super::inputs::{Producer, input_limits, runtime_config as sources_config, scene as sources_scene};
use super::*;
use std::process::Child;
use std::sync::{Arc, Mutex, Weak};

#[test]
#[ignore = "requires an explicitly approved external device-removal/TDR window affecting other GPU apps"]
fn hardware_genuine_device_removal_rebuilds_owned_plans_and_retires_unobserved_old_work() {
    assert_eq!(
        std::env::var("GOLDEN_VIDEO_EXTERNAL_DEVICE_REMOVAL").as_deref(),
        Ok("1"),
        "requires an explicitly approved external GPU-reset window"
    );
    run(true);
}

#[test]
#[ignore = "explicit owned Spout source/recovery fixture preflight; does not trigger or qualify removal"]
fn hardware_source_recovery_fixture_preflight_without_device_removal() {
    run(false);
}

fn run(genuine: bool) {
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watched = children.clone();
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            finished.recv_timeout(Duration::from_secs(90)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            for child in watched.lock().unwrap().iter().filter_map(Weak::upgrade) {
                if let Ok(mut child) = child.try_lock() {
                    let _ = child.kill();
                }
            }
            eprintln!("external native device-removal/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .enumerate()
        .filter(|(_, adapter)| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    let mut owners = Vec::new();
    let mut producers = Vec::new();
    for (index, adapter) in hardware {
        let identity = uuid::Uuid::new_v4();
        let names = [
            format!("GoldenRemovalA-{identity}"),
            format!("GoldenRemovalB-{identity}"),
        ];
        for name in &names {
            let mut producer = Producer::new(index, name, &children);
            producer.command("start 2 2 0 1 0");
            producers.push(producer);
        }
        let mut owner = Owner::configured(
            adapter.id,
            false,
            sources_config(),
            sources_scene(&names, 2, 2),
            input_limits(),
        );
        owner.submit();
        wait("both actual source workers publish/copy before reset", || {
            let sources = owner.control.sources();
            sources.len() == 2
                && sources.iter().all(|source| {
                    source.state == crate::native::SourceState::Streaming
                        && source.publications > 0
                        && source.renderer_copies > 0
                })
        });
        owner.gate.hold();
        wait("two unobserved native uses before external removal", || {
            owner.runtime().snapshot().resources.unwrap().in_flight == 2
        });
        let original = owner.runtime().snapshot().resources.unwrap();
        assert!(!original.device_lost);
        assert!(original.bytes_used > 0);
        owners.push((adapter, owner, original));
    }
    if genuine {
        eprintln!(
            "READY_EXTERNAL_DEVICE_REMOVAL: owned D3D11 plans active, completions withheld; external reset must be explicitly approved"
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        while !owners
            .iter()
            .any(|(_, owner, _)| owner.gate.snapshot().removal.is_some())
        {
            assert!(
                Instant::now() < deadline,
                "no genuine GetDeviceRemovedReason error observed; no removal pass"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    let mut removals = 0;
    for (adapter, owner, original) in &mut owners {
        if let Some(removal) = owner.gate.snapshot().removal {
            removals += 1;
            assert_eq!(removal.device, original.last_submitted.device);
            assert!(removal.reason < 0);
            wait("real removal rebuilds original plan on a new native device", || {
                owner.runtime().snapshot().resources.is_some_and(|resources| {
                    resources.active.is_some_and(|active| {
                        active.device != removal.device && active.identity == original.active.unwrap().identity
                    })
                })
            });
            assert!(removal.observed_at.elapsed() < WAIT, "recreation exceeded five seconds");
            let resources = owner.runtime().snapshot().resources.unwrap();
            assert_eq!(resources.bytes_used, original.bytes_used);
            assert!(resources.peak_bytes <= resources.byte_limit);
            assert_eq!(
                resources.retiring_generations, 0,
                "confirmed removal must retire old-device uses despite the held observation gate"
            );
            assert_eq!(
                owner.runtime().snapshot().failure.unwrap().code(),
                crate::runtime::FailureCode::Backend
            );
            eprintln!(
                "GENUINE_NATIVE_REMOVAL adapter={:?} description={} reason=0x{:08x} old={:?} rebuilt={:?} recreate_ms={}",
                adapter.id,
                adapter.description,
                removal.reason as u32,
                removal.device,
                resources.active,
                removal.observed_at.elapsed().as_millis()
            );
        }
        owner.gate.release();
        let before = owner.control.snapshot().accepted_outputs;
        wait("actual output resumes after external reset", || {
            owner.control.snapshot().accepted_outputs >= before + 3
        });
        wait(
            "current-device input workers are streaming after release/recovery",
            || {
                let current = owner.runtime().snapshot().resources.unwrap().last_submitted.device;
                let sources = owner.control.sources();
                sources.len() == 2
                    && sources.iter().all(|source| {
                        source.device == current
                            && source.state == crate::native::SourceState::Streaming
                            && source.publications > 0
                            && source.renderer_copies > 0
                    })
            },
        );
        let resources = owner.runtime().snapshot().resources.unwrap();
        assert!(
            resources.in_flight <= 2
                && resources.retiring_generations <= 2
                && resources.bytes_used <= resources.byte_limit
        );
        let budget = PayloadPool::new("recovered-native-pixels", 3, 65536).unwrap();
        let mut probe = SpoutProbe::new(&owner.name, adapter.id, budget.clone()).unwrap();
        let mut pixels = None;
        wait("recovered output pixels", || {
            pixels = probe.capture_pixels().unwrap();
            pixels.is_some()
        });
        let pixels = pixels.unwrap();
        assert!(pixels.pixels().iter().all(|pixel| *pixel == [0, 255, 0, 255]));
        drop(pixels);
        drop(probe);
        assert!(budget.wait_for_idle(WAIT));
    }
    if genuine {
        assert!(removals > 0);
    } else {
        assert_eq!(removals, 0, "preflight must not claim removal");
    }
    let close = Instant::now();
    for (_, owner, _) in &owners {
        owner.runtime().close();
    }
    for (_, owner, _) in &owners {
        assert!(owner.runtime().retirement().wait(WAIT.saturating_sub(close.elapsed())));
        assert_eq!(owner.runtime().snapshot().state, RuntimeState::Stopped);
        assert_eq!(owner.runtime().snapshot().resources.unwrap().bytes_used, 0);
        assert!(!owner.control.snapshot().initialized);
    }
    assert!(close.elapsed() < WAIT);
    eprintln!(
        "external device-removal qualification removals={removals} shutdown_ms={}",
        close.elapsed().as_millis()
    );
    drop(owners);
    for mut producer in producers {
        producer.finish();
    }
    eprintln!("source fixture preflight/recovery genuine={genuine}; all owned producer processes exited normally");
    done.send(()).unwrap();
}
