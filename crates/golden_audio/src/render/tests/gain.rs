use crate::GainSmoother;

#[test]
fn gain_smoother_reaches_target_in_exact_frame_count() {
    let mut smoother = GainSmoother::settled(1.0);
    smoother.set_target(0.0, 4);
    let values = [
        smoother.next_gain(),
        smoother.next_gain(),
        smoother.next_gain(),
        smoother.next_gain(),
    ];
    assert_eq!(values, [0.75, 0.5, 0.25, 0.0]);
    assert_eq!(smoother.remaining(), 0);
    assert_eq!(smoother.next_gain(), 0.0);
}
