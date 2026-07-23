use crate::{DriftController, DriftControllerConfig};

#[test]
fn simulated_input_clocks_converge_from_minus_to_plus_one_thousand_ppm() {
    for source_ppm in [-1_000.0, -500.0, 0.0, 500.0, 1_000.0] {
        let config = DriftControllerConfig::default();
        let mut controller = DriftController::new(config).unwrap();
        let mut fill = config.target_fill_frames as f64;
        let mut tail_min = f64::MAX;
        let mut tail_max = f64::MIN;
        for block in 0..80_000 {
            let correction = controller.observe_fill(fill.round().max(0.0) as usize);
            let produced = 128.0 * (1.0 + source_ppm / 1_000_000.0);
            let consumed = 128.0 / (1.0 - correction / 1_000_000.0);
            fill += produced - consumed;
            if block >= 70_000 {
                tail_min = tail_min.min(fill);
                tail_max = tail_max.max(fill);
            }
        }
        assert!(
            (fill - config.target_fill_frames as f64).abs() < 32.0,
            "source {source_ppm} ppm settled at fill {fill}"
        );
        assert!(
            (controller.correction_ppm() - source_ppm).abs() < 35.0,
            "source {source_ppm} ppm settled at correction {}",
            controller.correction_ppm()
        );
        assert!(
            tail_max - tail_min < 12.0,
            "source {source_ppm} ppm oscillated across {} frames",
            tail_max - tail_min
        );
    }
}

#[test]
fn drift_controller_reset_forgets_integral_history() {
    let mut controller = DriftController::new(DriftControllerConfig::default()).unwrap();
    for _ in 0..1_000 {
        controller.observe_fill(4_000);
    }
    assert!(controller.correction_ppm() > 0.0);
    controller.reset();
    assert_eq!(controller.correction_ppm(), 0.0);
}
