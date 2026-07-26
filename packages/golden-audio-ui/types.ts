import type { ParamValue, UiNodeDto } from 'golden_ui';
import type {
	AudioBufferPolicy,
	AudioDeviceInspectorState,
	AudioDeviceTargetId,
	AudioRecoveryPolicy
} from './generated';

export type IntentResult = boolean;

/**
 * Application-owned bridge consumed by the reusable device inspector.
 *
 * Implementations translate these methods into their own persistence and
 * transport intents. Device identity, recovery, and negotiation stay in the
 * backend represented by `state`.
 */
export interface AudioDeviceInspectorBinding {
	readonly state: AudioDeviceInspectorState;
	readonly fixedBufferFrames?: number;
	readonly managedChildKeys?: readonly string[];
	setInputEnabled(enabled: boolean): Promise<IntentResult>;
	selectInputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
	setOutputEnabled(enabled: boolean): Promise<IntentResult>;
	selectOutputTarget(target: AudioDeviceTargetId): Promise<IntentResult>;
	setRecoveryPolicy(policy: AudioRecoveryPolicy): Promise<IntentResult>;
	setSampleRate(rate: number): Promise<IntentResult>;
	setBufferPolicy(policy: AudioBufferPolicy): Promise<IntentResult>;
	setFixedBufferFrames(frames: number): Promise<IntentResult>;
	refreshDevices(): Promise<void>;
}

export type AudioDeviceInspectorAdapter = (node: UiNodeDto) => AudioDeviceInspectorBinding;

export type GoldenAudioParameterTarget = number | string;

export interface GoldenAudioDeviceParameterTargets {
	inputEnabled: GoldenAudioParameterTarget;
	inputTarget: GoldenAudioParameterTarget;
	outputEnabled: GoldenAudioParameterTarget;
	outputTarget: GoldenAudioParameterTarget;
	recoveryPolicy: GoldenAudioParameterTarget;
	sampleRate: GoldenAudioParameterTarget;
	bufferPolicy: GoldenAudioParameterTarget;
	fixedBufferFrames: GoldenAudioParameterTarget;
	refreshDevices: GoldenAudioParameterTarget;
}

export interface GoldenAudioDeviceParameterPort {
	readonly state: AudioDeviceInspectorState;
	readonly fixedBufferFrames?: number;
	setParameter(target: GoldenAudioParameterTarget, value: ParamValue): Promise<IntentResult>;
}

export const audioDeviceTargetParamValue = (target: AudioDeviceTargetId): string =>
	target.kind === 'system_default'
		? JSON.stringify({ kind: 'system_default', backend: target.backend })
		: JSON.stringify({
				kind: 'device',
				backend: target.backend,
				device: target.device
			});

/**
 * Creates a binding for Golden applications whose audio node exposes ordinary
 * parameters. The caller supplies stable parameter IDs or declared paths and
 * remains responsible for resolving them to edit intents.
 */
export const createGoldenAudioDeviceParameterBinding = (
	port: GoldenAudioDeviceParameterPort,
	targets: GoldenAudioDeviceParameterTargets,
	managedChildKeys: readonly string[] = []
): AudioDeviceInspectorBinding => ({
	get state() {
		return port.state;
	},
	get fixedBufferFrames() {
		return port.fixedBufferFrames;
	},
	managedChildKeys,
	setInputEnabled: (enabled) =>
		port.setParameter(targets.inputEnabled, { kind: 'bool', value: enabled }),
	selectInputTarget: (target) =>
		port.setParameter(targets.inputTarget, {
			kind: 'enum',
			value: audioDeviceTargetParamValue(target)
		}),
	setOutputEnabled: (enabled) =>
		port.setParameter(targets.outputEnabled, { kind: 'bool', value: enabled }),
	selectOutputTarget: (target) =>
		port.setParameter(targets.outputTarget, {
			kind: 'enum',
			value: audioDeviceTargetParamValue(target)
		}),
	setRecoveryPolicy: (policy) =>
		port.setParameter(targets.recoveryPolicy, { kind: 'enum', value: policy }),
	setSampleRate: (rate) =>
		port.setParameter(targets.sampleRate, {
			kind: 'int',
			value: Math.round(rate)
		}),
	setBufferPolicy: (policy) =>
		port.setParameter(targets.bufferPolicy, {
			kind: 'enum',
			value: policy.kind
		}),
	setFixedBufferFrames: (frames) =>
		port.setParameter(targets.fixedBufferFrames, {
			kind: 'int',
			value: Math.round(frames)
		}),
	refreshDevices: async () => {
		await port.setParameter(targets.refreshDevices, { kind: 'trigger' });
	}
});
