use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "explicit enum publication comparison measurement"]
fn late_metadata_change_comparison_samples() {
    let original: ParameterEnumOptions = (0..32_768).map(option).collect();
    let mut entries = original.as_slice().to_vec();
    entries.last_mut().unwrap().label.push('!');
    let changed = ParameterEnumOptions::from(entries);
    assert!(original.retained_bytes() < 32 * 1024 * 1024);
    let mut samples = Vec::new();
    for _ in 0..21 {
        let started = Instant::now();
        assert!(!black_box(&original).eq(black_box(&changed)));
        samples.push(started.elapsed().as_nanos());
    }
    println!("32768-option late-label comparison ns: {samples:?}");
}
