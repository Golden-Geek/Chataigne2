use allocation_counter::measure;

use crate::{
    AnalysisCaptureError, PreparedVoice, RealtimeScope, VoiceRetirementReason, analysis_frame_pool, voice_slot_pool,
};

#[test]
fn fixed_voice_slots_retain_payload_when_return_queue_is_full() {
    let (mut control, mut realtime) = voice_slot_pool::<u64>(1).unwrap();
    let first = control.next_id(0).unwrap();
    realtime
        .activate(PreparedVoice {
            id: first,
            payload: Box::new(11),
        })
        .unwrap();
    assert!(realtime.retire(first, VoiceRetirementReason::Completed));

    let second = control.next_id(0).unwrap();
    realtime
        .activate(PreparedVoice {
            id: second,
            payload: Box::new(22),
        })
        .unwrap();
    assert!(realtime.retire(second, VoiceRetirementReason::Requested));
    assert_eq!(control.pressure().voice_return_full, 1);

    let mut values = Vec::new();
    assert_eq!(control.reclaim(|voice| values.push(*voice.payload)), 1);
    realtime.flush_retired();
    assert_eq!(control.reclaim(|voice| values.push(*voice.payload)), 1);
    assert_eq!(values, [11, 22]);
    realtime.into_retirement().reclaim();
}

#[test]
fn analysis_frames_are_preallocated_bounded_and_recyclable() {
    let (mut reader, mut writer) = analysis_frame_pool(1, 4).unwrap();
    writer.capture(7, &[1.0, 2.0, 3.0]).unwrap();
    assert_eq!(writer.capture(8, &[4.0]), Err(AnalysisCaptureError::NoFreeFrame));

    let frame = reader.try_recv().unwrap();
    assert_eq!(frame.sequence(), 7);
    assert_eq!(frame.samples(), &[1.0, 2.0, 3.0]);
    reader.recycle(frame).unwrap();
    writer.capture(9, &[5.0; 4]).unwrap();
    let frame = reader.try_recv().unwrap();
    assert_eq!(frame.samples(), &[5.0; 4]);
    reader.recycle(frame).unwrap();
    writer.into_retirement().reclaim();
}

#[test]
fn oversized_analysis_frame_does_not_consume_the_fixed_slot() {
    let (_reader, mut writer) = analysis_frame_pool(1, 2).unwrap();
    assert_eq!(writer.capture(1, &[0.0; 3]), Err(AnalysisCaptureError::FrameTooLarge));
    writer.capture(2, &[1.0, 2.0]).unwrap();
    writer.into_retirement().reclaim();
}

#[test]
fn callback_owned_voice_and_analysis_transfers_do_not_allocate_or_deallocate() {
    let (mut voice_control, mut voices) = voice_slot_pool::<u64>(1).unwrap();
    let voice = voice_control.next_id(0).unwrap();
    voices
        .activate(PreparedVoice {
            id: voice,
            payload: Box::new(42),
        })
        .unwrap();
    let (mut analysis_reader, mut analysis_writer) = analysis_frame_pool(1, 4).unwrap();

    let allocation = measure(|| {
        let _scope = RealtimeScope::enter();
        assert!(voices.retire(voice, VoiceRetirementReason::Completed));
        analysis_writer.capture(1, &[0.0; 4]).unwrap();
    });

    assert_eq!(allocation.count_total, 0);
    assert_eq!(allocation.count_current, 0);
    assert_eq!(allocation.bytes_total, 0);
    assert_eq!(allocation.bytes_current, 0);
    voice_control.reclaim(drop);
    let frame = analysis_reader.try_recv().unwrap();
    analysis_reader.recycle(frame).unwrap();
    voices.into_retirement().reclaim();
    analysis_writer.into_retirement().reclaim();
}
