use crate::{InterleavedInput, InterleavedOutput, PlanarBuffer, deinterleave, interleave};

#[test]
fn planar_buffer_zeroes_selected_ranges() {
    let mut buffer = PlanarBuffer::new(2, 8).unwrap();
    for channel in 0..2 {
        buffer.channel_mut(channel).fill(1.0);
    }
    buffer.zero_range(2, 3).unwrap();
    assert_eq!(buffer.channel(0), &[1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    assert_eq!(buffer.channel(1), &[1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
}

#[test]
fn interleaved_f32_round_trip_is_planar_and_contains_non_finite_input() {
    let input = [0.25, -0.25, 0.5, f32::NAN, -0.5, 0.75];
    let mut planar = PlanarBuffer::new(2, 3).unwrap();
    let stats = deinterleave(InterleavedInput::F32(&input), 2, 3, &mut planar, 0).unwrap();
    assert_eq!(stats.non_finite_samples, 1);
    assert_eq!(planar.channel(0), &[0.25, 0.5, -0.5]);
    assert_eq!(planar.channel(1), &[-0.25, 0.0, 0.75]);

    let mut output = [0.0; 6];
    let stats = interleave(&planar, 0, 2, 3, InterleavedOutput::F32(&mut output)).unwrap();
    assert_eq!(stats.non_finite_samples, 0);
    assert_eq!(output, [0.25, -0.25, 0.5, 0.0, -0.5, 0.75]);
}

#[test]
fn interleaved_f32_output_contains_non_finite_samples() {
    let mut planar = PlanarBuffer::new(1, 2).unwrap();
    planar.channel_mut(0).copy_from_slice(&[f32::NAN, f32::INFINITY]);
    let mut output = [1.0_f32; 2];

    let stats = interleave(&planar, 0, 1, 2, InterleavedOutput::F32(&mut output)).unwrap();

    assert_eq!(output, [0.0, 0.0]);
    assert_eq!(stats.non_finite_samples, 2);
}

#[test]
fn integer_and_packed_24_bit_conversion_saturates() {
    let mut planar = PlanarBuffer::new(1, 5).unwrap();
    planar.channel_mut(0).copy_from_slice(&[-2.0, -1.0, 0.0, 1.0, 2.0]);

    let mut i16_output = [0_i16; 5];
    let stats = interleave(&planar, 0, 1, 5, InterleavedOutput::I16(&mut i16_output)).unwrap();
    assert_eq!(stats.clipped_samples, 2);
    assert_eq!(i16_output, [i16::MIN, i16::MIN, 0, i16::MAX, i16::MAX]);

    let mut i24_output = [0_i32; 5];
    interleave(&planar, 0, 1, 5, InterleavedOutput::I24(&mut i24_output)).unwrap();
    assert_eq!(i24_output, [-8_388_608, -8_388_608, 0, 8_388_607, 8_388_607]);

    let mut u24_output = [0_u32; 5];
    interleave(&planar, 0, 1, 5, InterleavedOutput::U24(&mut u24_output)).unwrap();
    assert_eq!(u24_output, [0, 0, 8_388_608, 16_777_215, 16_777_215]);
}

#[test]
fn all_boundary_sample_formats_convert_without_shape_drift() {
    let mut planar = PlanarBuffer::new(1, 3).unwrap();
    let i16_input = [i16::MIN, 0, i16::MAX];
    deinterleave(InterleavedInput::I16(&i16_input), 1, 3, &mut planar, 0).unwrap();
    assert_eq!(planar.sample(0, 0), -1.0);
    assert_eq!(planar.sample(0, 1), 0.0);
    assert_eq!(planar.sample(0, 2), 1.0);

    let mut f64_output = [0.0_f64; 3];
    interleave(&planar, 0, 1, 3, InterleavedOutput::F64(&mut f64_output)).unwrap();
    assert_eq!(f64_output, [-1.0, 0.0, 1.0]);

    let mut i32_output = [0_i32; 3];
    interleave(&planar, 0, 1, 3, InterleavedOutput::I32(&mut i32_output)).unwrap();
    assert_eq!(i32_output, [i32::MIN, 0, i32::MAX]);

    let mut u16_output = [0_u16; 3];
    interleave(&planar, 0, 1, 3, InterleavedOutput::U16(&mut u16_output)).unwrap();
    assert_eq!(u16_output, [0, 32_768, u16::MAX]);

    let mut u32_output = [0_u32; 3];
    interleave(&planar, 0, 1, 3, InterleavedOutput::U32(&mut u32_output)).unwrap();
    assert_eq!(u32_output, [0, 2_147_483_648, u32::MAX]);
}
