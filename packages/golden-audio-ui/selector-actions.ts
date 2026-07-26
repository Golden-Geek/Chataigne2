import type { AudioDeviceTargetId, AudioDirection } from './generated';
import type { AudioDeviceInspectorBinding, IntentResult } from './types';

export const setAudioDirectionEnabled = (
	binding: AudioDeviceInspectorBinding,
	direction: AudioDirection,
	enabled: boolean
): Promise<IntentResult> =>
	direction === 'input' ? binding.setInputEnabled(enabled) : binding.setOutputEnabled(enabled);

export const selectAudioDirectionTarget = (
	binding: AudioDeviceInspectorBinding,
	direction: AudioDirection,
	target: AudioDeviceTargetId
): Promise<IntentResult> =>
	direction === 'input' ? binding.selectInputTarget(target) : binding.selectOutputTarget(target);
