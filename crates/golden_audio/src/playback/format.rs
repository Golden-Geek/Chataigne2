use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[cfg_attr(feature = "codegen", derive(ts_rs::TS))]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "snake_case")]
pub enum AudioFileFormat {
    Wave,
    Aiff,
    Caf,
    Flac,
    Mp3,
    IsoMp4,
    Ogg,
    Matroska,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioFileFormatDescriptor {
    pub format: AudioFileFormat,
    pub label: &'static str,
    pub extensions: &'static [&'static str],
}

const FORMATS: &[AudioFileFormatDescriptor] = &[
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Wave,
        label: "Wave",
        extensions: &["wav", "wave"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Aiff,
        label: "AIFF",
        extensions: &["aif", "aiff", "aifc"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Caf,
        label: "Core Audio Format",
        extensions: &["caf"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Flac,
        label: "FLAC",
        extensions: &["flac"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Mp3,
        label: "MP3",
        extensions: &["mp3"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::IsoMp4,
        label: "MPEG-4 Audio",
        extensions: &["m4a", "mp4"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Ogg,
        label: "Ogg",
        extensions: &["ogg", "oga"],
    },
    AudioFileFormatDescriptor {
        format: AudioFileFormat::Matroska,
        label: "Matroska / WebM",
        extensions: &["mka", "mkv", "webm"],
    },
];

const EXTENSIONS: &[&str] = &[
    "wav", "wave", "aif", "aiff", "aifc", "caf", "flac", "mp3", "m4a", "mp4", "ogg", "oga", "mka", "mkv", "webm",
];

#[must_use]
pub const fn supported_audio_formats() -> &'static [AudioFileFormatDescriptor] {
    FORMATS
}

#[must_use]
pub const fn supported_audio_extensions() -> &'static [&'static str] {
    EXTENSIONS
}

#[must_use]
pub fn audio_file_format_for_extension(extension: &str) -> Option<AudioFileFormat> {
    let extension = extension.trim_start_matches('.');
    FORMATS
        .iter()
        .find(|descriptor| {
            descriptor
                .extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
        .map(|descriptor| descriptor.format)
}
