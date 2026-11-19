use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/decode/subtitles/tests/fixtures")
        .join(name)
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn active(timeline: &SubtitleTimeline, position: u64) -> Vec<&str> {
    timeline.active_at(ms(position)).map(|cue| cue.text.as_str()).collect()
}

#[test]
fn ffmpeg_reads_utf8_subrip_sidecar_timing_markup_and_overlapping_cues() {
    let timeline = load_local(&fixture("overlap.srt"), || false).unwrap();
    assert_eq!(timeline.cues().len(), 3);
    assert!(active(&timeline, 124).is_empty());
    assert_eq!(active(&timeline, 125), ["Café, first\nSecond line"]);
    assert_eq!(active(&timeline, 500), ["Café, first\nSecond line", "Overlap"]);
    assert_eq!(active(&timeline, 750), ["Overlap"]);
    assert_eq!(active(&timeline, 900), ["After"]);
    assert!(active(&timeline, 1000).is_empty());
}

#[test]
fn ffmpeg_reads_webvtt_text_and_ignores_presentation_metadata() {
    let timeline = load_local(&fixture("styled.vtt"), || false).unwrap();
    assert_eq!(active(&timeline, 150), ["Hello, world\nUnstyled"]);
    assert!(active(&timeline, 600).is_empty());
    assert_eq!(active(&timeline, 800), ["Again"]);
}

#[test]
fn discovery_preserves_video_timing_when_there_are_no_subtitle_tracks() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decode/tests/fixtures/h264.mp4");
    let super::super::MediaMetadata {
        video,
        subtitles: tracks,
        ..
    } = super::super::probe_media(&path, || false).unwrap();
    assert!(video.duration.is_some_and(|duration| duration > 0));
    assert!(tracks.is_empty());
    assert!(matches!(
        load_embedded(&path, 0, 0, video.time_base, || false),
        Err(SubtitleError::Unsupported)
    ));
    assert!(matches!(
        load_local(&fixture("absent.srt"), || false),
        Err(SubtitleError::NotAFile)
    ));
}

#[test]
fn ass_normalization_preserves_commas_lines_spaces_and_literal_braces() {
    let dialogue = r"12,0,Default,,0,0,0,,{\i1}Café, first{\i0}\N{\p1}m 0 0 l 3 4{\p0}line\hwith\h\{braces\}";
    assert_eq!(
        text::ass_dialogue(dialogue).unwrap(),
        "Café, first\nline\u{00a0}with\u{00a0}{braces}"
    );
    assert!(matches!(
        text::ass_dialogue("bad event"),
        Err(SubtitleError::InvalidText)
    ));
}

#[test]
fn source_origin_subtraction_rescales_before_rounding_and_admits_negative_container_time() {
    let video = TimeBase {
        numerator: 1001,
        denominator: 30_000,
    };
    let subtitle = TimeBase {
        numerator: 1,
        denominator: 1_000,
    };
    let first = ScaledTime::new(1, video).unwrap();
    assert_eq!(ScaledTime::new(2, video).unwrap().relative_to(first), 33_366_667);
    assert_eq!(ScaledTime::new(34, subtitle).unwrap().relative_to(first), 633_334);
    assert_eq!(ScaledTime::new(33, subtitle).unwrap().relative_to(first), -366_666);
    let before_zero = ScaledTime::new(-1, video).unwrap();
    assert_eq!(
        ScaledTime::new(0, subtitle).unwrap().relative_to(before_zero),
        33_366_667
    );
}

#[test]
fn overlapping_timeline_is_bounded_and_seek_order_does_not_change_its_result() {
    let mut builder = timeline::CueBuilder::default();
    for index in (0..8).rev() {
        builder
            .push(SubtitleCue {
                start: ms(index),
                end: ms(10),
                text: index.to_string(),
            })
            .unwrap();
    }
    let timeline = builder.finish().unwrap();
    assert_eq!(active(&timeline, 9).len(), 8);
    assert!(active(&timeline, 10).is_empty());
    assert_eq!(active(&timeline, 1), ["0", "1"]);
    let mut builder = timeline::CueBuilder::default();
    for index in 0..9 {
        builder
            .push(SubtitleCue {
                start: ms(index),
                end: ms(10),
                text: index.to_string(),
            })
            .unwrap();
    }
    assert!(matches!(builder.finish(), Err(SubtitleError::LimitExceeded)));
}
