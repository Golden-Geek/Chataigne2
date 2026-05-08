use std::time::{Duration, Instant};

use golden_core::node::Folder;

use crate::app::{AppNode, GamepadModule, MidiModule};

fn create_engine(n_gamepad: usize, n_midi: usize) -> crate::app::AppEngine {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);

    for _ in 0..n_gamepad {
        let mut m = GamepadModule::create();
        m.disable_runtime_for_test();
        engine.add_node(m.into(), None);
    }
    for _ in 0..n_midi {
        engine.add_node(MidiModule::create().into(), None);
    }

    // Apply edits until stable (modules declare child nodes during init)
    for _ in 0..20 {
        engine.apply_edits().expect("module init should not fail");
    }
    engine.resolve().expect("schedule should resolve");
    engine
}

/// Run N ticks and return (min_us, max_us, total_us).
fn measure_ticks(engine: &mut crate::app::AppEngine, n: usize) -> (u64, u64, u64) {
    let dt = Duration::from_millis(8); // ~120 Hz
    let mut min_us = u64::MAX;
    let mut max_us = 0u64;
    let mut total_us = 0u64;
    for _ in 0..n {
        let t = Instant::now();
        engine.run_tick(dt).expect("tick should not fail");
        let elapsed = t.elapsed().as_micros() as u64;
        if elapsed < min_us { min_us = elapsed; }
        if elapsed > max_us { max_us = elapsed; }
        total_us += elapsed;
    }
    (min_us, max_us, total_us)
}

#[test]
fn idle_gamepad_modules_tick_time_does_not_scale_with_count() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut e1 = create_engine(1, 0);
    let mut e20 = create_engine(20, 0);

    // Warmup — let each engine settle fully before measuring
    for _ in 0..WARMUP {
        e1.run_tick(Duration::from_millis(8)).ok();
        e20.run_tick(Duration::from_millis(8)).ok();
    }

    let (min1, max1, total1) = measure_ticks(&mut e1, MEASURED);
    let (min20, max20, total20) = measure_ticks(&mut e20, MEASURED);

    let avg1 = total1 / MEASURED as u64;
    let avg20 = total20 / MEASURED as u64;

    eprintln!("  1 gamepad: avg={avg1}us  min={min1}us  max={max1}us  (total {total1}us)");
    eprintln!(" 20 gamepad: avg={avg20}us  min={min20}us  max={max20}us  (total {total20}us)");

    // 20 idle modules should tick in less than 4× the time of 1 idle module.
    // In a correct implementation both should be ~0.
    assert!(
        avg20 < avg1.max(500) * 4,
        "20 gamepad modules tick avg {avg20}us vs 1 module avg {avg1}us — scaling too high"
    );
}

#[test]
fn idle_midi_modules_tick_time_does_not_scale_with_count() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut e1 = create_engine(0, 1);
    let mut e30 = create_engine(0, 30);

    for _ in 0..WARMUP {
        e1.run_tick(Duration::from_millis(8)).ok();
        e30.run_tick(Duration::from_millis(8)).ok();
    }

    let (min1, max1, total1) = measure_ticks(&mut e1, MEASURED);
    let (min30, max30, total30) = measure_ticks(&mut e30, MEASURED);

    let avg1 = total1 / MEASURED as u64;
    let avg30 = total30 / MEASURED as u64;

    eprintln!("  1 midi: avg={avg1}us  min={min1}us  max={max1}us  (total {total1}us)");
    eprintln!(" 30 midi: avg={avg30}us  min={min30}us  max={max30}us  (total {total30}us)");

    assert!(
        avg30 < avg1.max(500) * 4,
        "30 midi modules tick avg {avg30}us vs 1 module avg {avg1}us — scaling too high"
    );
}

#[test]
fn steady_state_tick_budget_1000_nodes() {
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;
    // 10 gamepad modules ≈ 10 × ~100 nodes = ~1000 total nodes
    let mut engine = create_engine(10, 0);

    for _ in 0..WARMUP {
        engine.run_tick(Duration::from_millis(8)).ok();
    }

    let (_min, max, total) = measure_ticks(&mut engine, MEASURED);
    let avg = total / MEASURED as u64;

    eprintln!(" 10 gamepad (~1000 nodes): avg={avg}us  max={max}us  (total {total}us)");

    // In steady state, 1000 nodes idle should tick in under 1ms on average
    assert!(
        avg < 1_000,
        "1000-node engine ticks at {avg}us avg — expected under 1000us"
    );
}
