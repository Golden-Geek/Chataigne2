use std::collections::HashSet;

use super::super::{
    AudioFileFormat, audio_file_format_for_extension, supported_audio_extensions, supported_audio_formats,
};

#[test]
fn extension_matching_is_case_insensitive() {
    assert_eq!(audio_file_format_for_extension(".WAV"), Some(AudioFileFormat::Wave));
}

#[test]
fn generated_extension_source_is_complete_unique_and_excludes_wma_and_raw_aac() {
    let flattened = supported_audio_formats()
        .iter()
        .flat_map(|format| format.extensions.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(flattened, supported_audio_extensions());
    assert_eq!(flattened.iter().copied().collect::<HashSet<_>>().len(), flattened.len());
    assert!(!flattened.contains(&"wma"));
    assert!(!flattened.contains(&"aac"));
}
