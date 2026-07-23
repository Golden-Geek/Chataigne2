use std::path::Path;

use ts_rs::{Config, TS};

use crate::{AudioDeviceInspectorState, AudioDeviceSelection, AudioFileFormat, supported_audio_extensions};

const INDEX: &str = "\
export type { AudioBackendState } from './AudioBackendState';\n\
export type { AudioBackendStatus } from './AudioBackendStatus';\n\
export type { AudioBufferPolicy } from './AudioBufferPolicy';\n\
export type { AudioDeviceDescriptor } from './AudioDeviceDescriptor';\n\
export type { AudioDeviceFingerprint } from './AudioDeviceFingerprint';\n\
export type { AudioDeviceId } from './AudioDeviceId';\n\
export type { AudioDeviceInspectorState } from './AudioDeviceInspectorState';\n\
export type { AudioDeviceProfileKey } from './AudioDeviceProfileKey';\n\
export type { AudioDeviceReadiness } from './AudioDeviceReadiness';\n\
export type { AudioDeviceSelection } from './AudioDeviceSelection';\n\
export type { AudioDeviceTargetId } from './AudioDeviceTargetId';\n\
export type { AudioDirection } from './AudioDirection';\n\
export type { AudioErrorCategory } from './AudioErrorCategory';\n\
export type { AudioFileFormat } from './AudioFileFormat';\n\
export type { AudioInspectorError } from './AudioInspectorError';\n\
export type { AudioPermissionState } from './AudioPermissionState';\n\
export type { AudioRecoveryPolicy } from './AudioRecoveryPolicy';\n\
export type { AudioSampleFormat } from './AudioSampleFormat';\n\
export type { AudioStreamStatus } from './AudioStreamStatus';\n\
export type { BackendId } from './BackendId';\n\
export type { NegotiatedStreamFormat } from './NegotiatedStreamFormat';\n\
export type { PhysicalChannelDescriptor } from './PhysicalChannelDescriptor';\n\
export type { PhysicalChannelKey } from './PhysicalChannelKey';\n\
export type { SupportedBufferFrames } from './SupportedBufferFrames';\n\
export type { SupportedStreamConfiguration } from './SupportedStreamConfiguration';\n\
export { supportedAudioExtensions } from './supportedAudioExtensions';\n\
export type { SupportedAudioExtension } from './supportedAudioExtensions';\n";

pub fn export_device_contract(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;
    let config = Config::new().with_out_dir(output_dir.to_path_buf());
    AudioDeviceInspectorState::export_all(&config)?;
    AudioDeviceSelection::export_all(&config)?;
    AudioFileFormat::export_all(&config)?;
    let extensions = supported_audio_extensions()
        .iter()
        .map(|extension| format!("'{extension}'"))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        output_dir.join("supportedAudioExtensions.ts"),
        format!(
            "export const supportedAudioExtensions = [{extensions}] as const;\n\
             export type SupportedAudioExtension = (typeof supportedAudioExtensions)[number];\n"
        ),
    )?;
    std::fs::write(output_dir.join("index.ts"), INDEX)?;
    Ok(())
}
