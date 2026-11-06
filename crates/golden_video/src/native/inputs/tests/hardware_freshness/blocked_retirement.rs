//! Real SDK access contention must not hold source shutdown or another input hostage.
use super::*;

#[test]
#[ignore = "requires owned hardware Spout senders and their actual access mutex"]
fn hardware_blocked_sdk_access_retires_without_waiting_for_the_peer() {
    let (finished, watchdog) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            watchdog.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned SDK-contention retirement watchdog expired");
            std::process::exit(1);
        }
    });
    for adapter in adapters().unwrap().into_iter().filter(|a| !a.software) {
        let names = [
            format!("GoldenBlocked-{}", Uuid::new_v4()),
            format!("GoldenHealthy-{}", Uuid::new_v4()),
        ];
        let _senders = names
            .iter()
            .map(|name| Sender::new(adapter.id, name.clone(), descriptor(8)))
            .collect::<Vec<_>>();
        let (ready, started) = mpsc::channel();
        let (stop, stopping) = mpsc::channel();
        let (retired, retirement) = mpsc::channel();
        let worker_names = names.clone();
        let worker = std::thread::spawn(move || {
            let ids = [TextureId::new(1).unwrap(), TextureId::new(2).unwrap()];
            let plan = RenderPlan {
                external_textures: ids
                    .into_iter()
                    .zip(worker_names)
                    .map(|(id, sender)| ExternalTexture {
                        id,
                        descriptor: descriptor(8),
                        source: Some(
                            SpoutSource {
                                metadata: crate::plan::SpoutMetadata::Declared,
                                sender,
                                descriptor: descriptor(8),
                                poll_interval: Duration::from_millis(5),
                                frame_count: SpoutFrameCount::Unversioned,
                            }
                            .into(),
                        ),
                    })
                    .collect(),
                ..Default::default()
            }
            .compile_with_source_demands(
                &PlanLimits {
                    max_passes: 0,
                    max_external_textures: 2,
                    max_outputs: 0,
                    max_dependencies: 0,
                    max_texture_dimension: 8,
                    max_texture_bytes: 1024,
                },
                &ids,
            )
            .unwrap();
            let mut pool = SourcePool::new(
                adapter.id,
                InputLimits {
                    max_sources: 2,
                    max_storage_bytes: 2 * InputLimits::source_storage_bytes(descriptor(8)).unwrap(),
                },
                &D3d11Control::default(),
                None,
                DeviceGeneration::new(Uuid::new_v4(), 0),
            );
            pool.ensure(&plan).unwrap();
            let shared = ids.map(|id| pool.inputs[&id].shared.clone());
            let demands = shared.each_ref().map(|source| source.acquire_demand());
            ready.send(shared).unwrap();
            let _ = stopping.recv();
            let at = Instant::now();
            drop(demands);
            drop(pool);
            retired.send(at.elapsed()).unwrap();
        });
        let [blocked, healthy] = started.recv_timeout(WAIT).unwrap();
        wait("both actual SDK inputs publish", || {
            snapshot(&blocked).publications >= 3 && snapshot(&healthy).publications >= 3
        });
        let access = AccessLock::new(&names[0]);
        let before = [snapshot(&blocked), snapshot(&healthy)];
        wait("actual SDK returns Busy while healthy input keeps copying", || {
            snapshot(&blocked).receive_busy >= before[0].receive_busy + 3
                && snapshot(&healthy).publications >= before[1].publications + 3
        });
        stop.send(()).unwrap();
        let elapsed = retirement.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(elapsed < Duration::from_secs(2));
        worker.join().unwrap();
        for source in [&blocked, &healthy] {
            assert_eq!(snapshot(source).state, SourceState::Stopped);
            assert!(source.latest.lock().unwrap().is_none());
        }
        // The peer's real mutex is still held when native worker retirement succeeds.
        drop(access);
        eprintln!(
            "SDK_BUSY_RETIRE adapter={:?} gpu={:?} shutdown_us={} blocked_busy={} healthy_copies={}",
            adapter.id,
            adapter.description,
            elapsed.as_micros(),
            snapshot(&blocked).receive_busy - before[0].receive_busy,
            snapshot(&healthy).publications - before[1].publications
        );
    }
    finished.send(()).unwrap();
}
