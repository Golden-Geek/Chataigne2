import type {
	AudioBufferPolicy,
	AudioDeviceDescriptor,
	AudioDeviceInspectorState,
	AudioDeviceTargetId,
	AudioRecoveryPolicy,
	AudioStreamStatus
} from './generated';
import type { AudioDeviceInspectorBinding, IntentResult } from './types';

export interface MockAudioDeviceCall {
	readonly method: string;
	readonly value?: unknown;
}

const mockDevice = (
	device: string,
	label: string,
	inputChannels: number,
	outputChannels: number
): AudioDeviceDescriptor => ({
	target: { kind: 'device', backend: 'mock', device },
	label,
	stable_id: true,
	fingerprint: {
		vendor: 'Golden',
		product: label,
		serial: device,
		transport: 'virtual',
		backend_path: null,
		input_channels: inputChannels,
		output_channels: outputChannels,
		properties: {}
	},
	profile_key: `mock:${device}`,
	input_channels: Array.from({ length: inputChannels }, (_, index) => ({
		key: `input:${index}`,
		label: `Input ${index + 1}`,
		position: null
	})),
	output_channels: Array.from({ length: outputChannels }, (_, index) => ({
		key: `output:${index}`,
		label: `Output ${index + 1}`,
		position: null
	})),
	supported_configurations: [
		{
			direction: inputChannels > 0 ? 'input' : 'output',
			channels: Math.max(inputChannels, outputChannels),
			sample_format: 'f32',
			min_sample_rate: 44_100,
			max_sample_rate: 96_000,
			buffer_frames: { min: 32, max: 2048, preferred: 128 }
		}
	],
	is_system_default_input: inputChannels > 0,
	is_system_default_output: outputChannels > 0
});

const readyStream = (
	direction: 'input' | 'output',
	target: AudioDeviceTargetId,
	label: string
): AudioStreamStatus => ({
	direction,
	enabled: true,
	selected_target: target,
	selected_label: label,
	profile_key: `mock:${direction}`,
	active_target: target,
	readiness: 'ready',
	permission: 'granted',
	recovery_policy: 'wait_for_selected',
	retry_attempt: 0,
	next_retry_ms: null,
	format: {
		sample_rate: 48_000,
		channels: 2,
		sample_format: 'f32',
		buffer_frames: 128,
		estimated_latency_ms: 5.3
	},
	error: null
});

export const createMockAudioDeviceState = (): AudioDeviceInspectorState => {
	const inputTarget: AudioDeviceTargetId = {
		kind: 'device',
		backend: 'mock',
		device: 'studio-input'
	};
	const outputTarget: AudioDeviceTargetId = {
		kind: 'device',
		backend: 'mock',
		device: 'studio-output'
	};
	return {
		discovery_in_progress: false,
		backends: [
			{
				backend: 'mock',
				label: 'Mock Audio',
				state: 'available',
				detail: null
			}
		],
		devices: [
			mockDevice('studio-input', 'Studio Input', 2, 0),
			mockDevice('studio-output', 'Studio Output', 0, 2)
		],
		input: readyStream('input', inputTarget, 'Studio Input'),
		output: readyStream('output', outputTarget, 'Studio Output'),
		engine_sample_rate: 48_000,
		buffer_policy: { kind: 'automatic' }
	};
};

export class MockAudioDeviceInspectorAdapter implements AudioDeviceInspectorBinding {
	state = $state<AudioDeviceInspectorState>(createMockAudioDeviceState());
	fixedBufferFrames = $state(128);
	readonly managedChildKeys: readonly string[] = [];
	readonly calls = $state<MockAudioDeviceCall[]>([]);
	private rejectNext = false;
	private refreshAction: (() => Promise<void>) | null = null;

	constructor(state?: AudioDeviceInspectorState) {
		if (state) this.state = state;
	}

	rejectNextIntent(): void {
		this.rejectNext = true;
	}

	setRefreshAction(action: (() => Promise<void>) | null): void {
		this.refreshAction = action;
	}

	private admit(method: string, value?: unknown): IntentResult {
		this.calls.push({ method, value });
		if (this.rejectNext) {
			this.rejectNext = false;
			return false;
		}
		return true;
	}

	private updateStream(
		direction: 'input' | 'output',
		update: (stream: AudioStreamStatus) => AudioStreamStatus
	): void {
		this.state = {
			...this.state,
			[direction]: update(this.state[direction])
		};
	}

	async setInputEnabled(enabled: boolean): Promise<IntentResult> {
		if (!this.admit('setInputEnabled', enabled)) return false;
		this.updateStream('input', (stream) => ({ ...stream, enabled }));
		return true;
	}

	async selectInputTarget(target: AudioDeviceTargetId): Promise<IntentResult> {
		if (!this.admit('selectInputTarget', target)) return false;
		this.updateStream('input', (stream) => ({
			...stream,
			selected_target: target
		}));
		return true;
	}

	async setOutputEnabled(enabled: boolean): Promise<IntentResult> {
		if (!this.admit('setOutputEnabled', enabled)) return false;
		this.updateStream('output', (stream) => ({ ...stream, enabled }));
		return true;
	}

	async selectOutputTarget(target: AudioDeviceTargetId): Promise<IntentResult> {
		if (!this.admit('selectOutputTarget', target)) return false;
		this.updateStream('output', (stream) => ({
			...stream,
			selected_target: target
		}));
		return true;
	}

	async setRecoveryPolicy(policy: AudioRecoveryPolicy): Promise<IntentResult> {
		if (!this.admit('setRecoveryPolicy', policy)) return false;
		this.updateStream('input', (stream) => ({
			...stream,
			recovery_policy: policy
		}));
		this.updateStream('output', (stream) => ({
			...stream,
			recovery_policy: policy
		}));
		return true;
	}

	async setSampleRate(rate: number): Promise<IntentResult> {
		if (!this.admit('setSampleRate', rate)) return false;
		this.state = { ...this.state, engine_sample_rate: Math.round(rate) };
		return true;
	}

	async setBufferPolicy(policy: AudioBufferPolicy): Promise<IntentResult> {
		if (!this.admit('setBufferPolicy', policy)) return false;
		this.state = { ...this.state, buffer_policy: policy };
		return true;
	}

	async setFixedBufferFrames(frames: number): Promise<IntentResult> {
		if (!this.admit('setFixedBufferFrames', frames)) return false;
		this.fixedBufferFrames = Math.round(frames);
		if (this.state.buffer_policy.kind === 'fixed') {
			this.state = {
				...this.state,
				buffer_policy: { kind: 'fixed', frames: this.fixedBufferFrames }
			};
		}
		return true;
	}

	async refreshDevices(): Promise<void> {
		this.calls.push({ method: 'refreshDevices' });
		await this.refreshAction?.();
	}
}
