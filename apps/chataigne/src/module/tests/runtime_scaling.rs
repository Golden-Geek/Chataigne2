use std::time::{Duration, Instant};

use golden_core::node::Folder;

use crate::app::{AppNode, GamepadModule, MidiModule};

use super::lock_performance_test;

fn create_engine(n_gamepad: usize, n_midi: usize) -> crate::app::AppEngine {
    let root: AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);

    for _ in 0..n_gamepad {
        let mut module = GamepadModule::create();
        module.disable_runtime_for_test();
        engine.add_node(module.into(), None);
    }
    for _ in 0..n_midi {
        engine.add_node(MidiModule::create().into(), None);
    }

    // Apply edits until stable because modules declare child nodes during initialization.
    for _ in 0..20 {
        engine.apply_edits().expect("module init should not fail");
    }
    engine.resolve().expect("schedule should resolve");
    engine
}

/// Runs `n` ticks and returns `(min_us, max_us, total_us)`.
fn measure_ticks(engine: &mut crate::app::AppEngine, n: usize) -> (u64, u64, u64) {
    let dt = Duration::from_millis(8);
    let mut min_us = u64::MAX;
    let mut max_us = 0_u64;
    let mut total_us = 0_u64;
    for _ in 0..n {
        let started = Instant::now();
        engine.run_tick(dt).expect("tick should not fail");
        let elapsed = started.elapsed().as_micros() as u64;
        min_us = min_us.min(elapsed);
        max_us = max_us.max(elapsed);
        total_us += elapsed;
    }
    (min_us, max_us, total_us)
}

#[test]
fn idle_gamepad_modules_tick_time_does_not_scale_with_count() {
    let _performance_guard = lock_performance_test();
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut one = create_engine(1, 0);
    let mut twenty = create_engine(20, 0);

    for _ in 0..WARMUP {
        one.run_tick(Duration::from_millis(8)).ok();
        twenty.run_tick(Duration::from_millis(8)).ok();
    }

    let (min_one, max_one, total_one) = measure_ticks(&mut one, MEASURED);
    let (min_twenty, max_twenty, total_twenty) = measure_ticks(&mut twenty, MEASURED);
    let average_one = total_one / MEASURED as u64;
    let average_twenty = total_twenty / MEASURED as u64;

    eprintln!(
        "  1 gamepad: avg={average_one}us min={min_one}us max={max_one}us (total {total_one}us)"
    );
    eprintln!(
        " 20 gamepad: avg={average_twenty}us min={min_twenty}us max={max_twenty}us (total {total_twenty}us)"
    );

    assert!(
        average_twenty < average_one.max(500) * 4,
        "20 gamepad modules tick avg {average_twenty}us vs 1 module avg {average_one}us — scaling too high"
    );
}

#[test]
fn idle_midi_modules_tick_time_does_not_scale_with_count() {
    let _performance_guard = lock_performance_test();
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;

    let mut one = create_engine(0, 1);
    let mut thirty = create_engine(0, 30);

    for _ in 0..WARMUP {
        one.run_tick(Duration::from_millis(8)).ok();
        thirty.run_tick(Duration::from_millis(8)).ok();
    }

    let (min_one, max_one, total_one) = measure_ticks(&mut one, MEASURED);
    let (min_thirty, max_thirty, total_thirty) = measure_ticks(&mut thirty, MEASURED);
    let average_one = total_one / MEASURED as u64;
    let average_thirty = total_thirty / MEASURED as u64;

    eprintln!("  1 midi: avg={average_one}us min={min_one}us max={max_one}us (total {total_one}us)");
    eprintln!(
        " 30 midi: avg={average_thirty}us min={min_thirty}us max={max_thirty}us (total {total_thirty}us)"
    );

    assert!(
        average_thirty < average_one.max(500) * 4,
        "30 midi modules tick avg {average_thirty}us vs 1 module avg {average_one}us — scaling too high"
    );
}

#[test]
fn steady_state_tick_budget_1000_nodes() {
    let _performance_guard = lock_performance_test();
    const WARMUP: usize = 20;
    const MEASURED: usize = 100;
    let mut engine = create_engine(10, 0);

    for _ in 0..WARMUP {
        engine.run_tick(Duration::from_millis(8)).ok();
    }

    let (_min, max, total) = measure_ticks(&mut engine, MEASURED);
    let average = total / MEASURED as u64;

    eprintln!(" 10 gamepad (~1000 nodes): avg={average}us max={max}us (total {total}us)");
    assert!(
        average < 1_000,
        "1000-node engine ticks at {average}us avg — expected under 1000us"
    );
}
