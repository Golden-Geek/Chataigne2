use super::super::{StreamWriteError, streaming_playback_ring};

#[test]
fn stream_ring_rejects_zero_capacity() {
    assert!(streaming_playback_ring(2, 0).is_err());
}

#[test]
fn stream_starvation_emits_silence_and_recovers_without_blocking() {
    let (mut writer, mut reader) = streaming_playback_ring(2, 4).unwrap();
    let mut destination = [1.0_f32; 8];
    assert_eq!(reader.read_interleaved(&mut destination), 0);
    assert_eq!(destination, [0.0; 8]);
    assert_eq!(reader.state().starvation_count, 1);

    writer.write_interleaved(&[0.1, 0.2, 0.3, 0.4]).unwrap();
    assert_eq!(reader.read_interleaved(&mut destination), 2);
    assert_eq!(&destination[..4], &[0.1, 0.2, 0.3, 0.4]);
    assert_eq!(&destination[4..], &[0.0; 4]);
    assert_eq!(reader.state().starvation_count, 2);

    writer
        .write_interleaved(&[0.5, 0.6, 0.7, 0.8, 0.9, 1.0, -0.1, -0.2])
        .unwrap();
    assert_eq!(writer.write_interleaved(&[0.0, 0.0]), Err(StreamWriteError::Full));
    writer.finish();
    assert_eq!(reader.read_interleaved(&mut destination), 4);
    assert!(reader.is_finished());
}

#[test]
fn cancellation_is_shared_with_the_decoder_side() {
    let (writer, reader) = streaming_playback_ring(1, 8).unwrap();
    reader.cancel();
    assert!(writer.is_cancelled());
}
