use super::*;

#[test]
fn stale_duplicate_incomplete_and_unbounded_busy_signals_cannot_complete() {
    let mut signals = Signals::default();
    assert!(
        signals
            .record(
                EditorActivity::BusyFinished {
                    session: 1,
                    elapsed_ms: 65_001.0
                },
                1
            )
            .is_err()
    );
    assert!(signals.record(EditorActivity::BusyStarted { session: 1 }, 2).is_err());
    signals.record(EditorActivity::BusyStarted { session: 2 }, 2).unwrap();
    assert!(signals.record(EditorActivity::BusyStarted { session: 2 }, 2).is_err());
    for elapsed_ms in [f64::NAN, f64::INFINITY, 64_999.0, 120_001.0] {
        assert!(
            signals
                .record(EditorActivity::BusyFinished { session: 2, elapsed_ms }, 2)
                .is_err()
        );
    }
    assert!(
        signals
            .record(
                EditorActivity::BusyFinished {
                    session: 1,
                    elapsed_ms: 65_001.0
                },
                2
            )
            .is_err()
    );
    signals
        .record(
            EditorActivity::BusyFinished {
                session: 2,
                elapsed_ms: 65_001.0,
            },
            2,
        )
        .unwrap();
    assert!(
        signals
            .record(
                EditorActivity::BusyFinished {
                    session: 2,
                    elapsed_ms: 65_001.0
                },
                2
            )
            .is_err()
    );
}
