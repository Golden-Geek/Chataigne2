import { describe, expect, it } from 'vitest';
import type { AudioDeviceInspectorState, AudioStreamStatus } from '../generated';
import { soundCardConnectionHint } from '../sound-card-connection-status';

const stream = (
	direction: 'input' | 'output',
	overrides: Partial<AudioStreamStatus> = {}
): AudioStreamStatus => ({
	direction,
	enabled: false,
	selected_target: null,
	selected_label: null,
	profile_key: null,
	active_target: null,
	readiness: 'disabled',
	permission: 'unknown',
	recovery_policy: 'wait_for_selected',
	retry_attempt: 0,
	next_retry_ms: null,
	format: null,
	error: null,
	...overrides
});

const deviceState = (
	input: AudioStreamStatus,
	output: AudioStreamStatus
): AudioDeviceInspectorState => ({
	discovery_in_progress: false,
	inventory_revision: 1,
	backends: [],
	device_catalog: [],
	devices: [],
	input,
	output,
	engine_sample_rate: 48_000,
	buffer_policy: { kind: 'automatic' }
});

describe('Sound Card connection hint', () => {
	it('summarizes the negotiated configuration when every active direction is ready', () => {
		const state = deviceState(
			stream('input'),
			stream('output', {
				enabled: true,
				selected_label: 'Studio Speakers',
				readiness: 'ready',
				format: {
					sample_rate: 48_000,
					channels: 2,
					sample_format: 'f32',
					buffer_frames: 512,
					estimated_latency_ms: 10.7
				}
			})
		);

		expect(soundCardConnectionHint(true, state)).toEqual({
			tone: 'success',
			message: 'Current configuration: Output: Studio Speakers · 48 kHz · 512-frame buffer.'
		});
	});

	it('surfaces the backend failure for a partially connected configuration', () => {
		const state = deviceState(
			stream('input', {
				enabled: true,
				selected_label: 'Studio Microphone',
				readiness: 'busy',
				error: {
					category: 'device_busy',
					message: 'device is already in use',
					technical_detail: null
				}
			}),
			stream('output', {
				enabled: true,
				selected_label: 'Studio Speakers',
				readiness: 'ready'
			})
		);

		expect(soundCardConnectionHint(true, state)).toEqual({
			tone: 'error',
			message: 'Current configuration: Input: Studio Microphone — device is already in use.'
		});
	});

	it('uses a pending tone while the selected device is being prepared', () => {
		const state = deviceState(
			stream('input'),
			stream('output', {
				enabled: true,
				selected_label: 'Studio Speakers',
				readiness: 'preparing'
			})
		);

		expect(soundCardConnectionHint(false, state)).toEqual({
			tone: 'pending',
			message: 'Current configuration: Output: Studio Speakers — connecting…'
		});
	});
});
