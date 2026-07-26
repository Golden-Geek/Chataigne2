import { describe, expect, it } from 'vitest';
import type { ParamValue } from 'golden_ui';
import {
	audioDeviceTargetParamValue,
	createGoldenAudioDeviceParameterBinding,
	type GoldenAudioParameterTarget
} from '../types';
import { createMockAudioDeviceState } from '../mock.svelte';
import { selectAudioDirectionTarget, setAudioDirectionEnabled } from '../selector-actions';

const targets = {
	inputEnabled: 'connection/input_enabled',
	inputTarget: 'connection/input_target',
	outputEnabled: 'connection/output_enabled',
	outputTarget: 'connection/output_target',
	recoveryPolicy: 'connection/recovery_policy',
	sampleRate: 'connection/sample_rate',
	bufferPolicy: 'connection/buffer_policy',
	fixedBufferFrames: 'connection/fixed_buffer_frames',
	refreshDevices: 'connection/refresh'
};

describe('Golden parameter binding', () => {
	it('sends stable target IDs and only invokes the supplied intent port', async () => {
		const calls: Array<[GoldenAudioParameterTarget, ParamValue]> = [];
		const port = {
			state: createMockAudioDeviceState(),
			fixedBufferFrames: 256,
			async setParameter(target: GoldenAudioParameterTarget, value: ParamValue) {
				calls.push([target, value]);
				return true;
			}
		};
		const binding = createGoldenAudioDeviceParameterBinding(port, targets, ['connection']);
		const target = {
			kind: 'device' as const,
			backend: 'mock',
			device: 'stable-output'
		};

		await binding.selectOutputTarget(target);
		await binding.setOutputEnabled(false);
		await binding.refreshDevices();

		expect(calls).toEqual([
			[targets.outputTarget, { kind: 'enum', value: audioDeviceTargetParamValue(target) }],
			[targets.outputEnabled, { kind: 'bool', value: false }],
			[targets.refreshDevices, { kind: 'trigger' }]
		]);
		expect(binding.managedChildKeys).toEqual(['connection']);
		expect(binding.fixedBufferFrames).toBe(256);
	});

	it('routes input and output controls only to their matching binding methods', async () => {
		const state = createMockAudioDeviceState();
		const calls: string[] = [];
		const binding = {
			state,
			setInputEnabled: async () => (calls.push('input-enabled'), true),
			selectInputTarget: async () => (calls.push('input-target'), true),
			setOutputEnabled: async () => (calls.push('output-enabled'), true),
			selectOutputTarget: async () => (calls.push('output-target'), true),
			setRecoveryPolicy: async () => true,
			setSampleRate: async () => true,
			setBufferPolicy: async () => true,
			setFixedBufferFrames: async () => true,
			refreshDevices: async () => undefined
		};

		await setAudioDirectionEnabled(binding, 'input', true);
		await setAudioDirectionEnabled(binding, 'output', false);
		await selectAudioDirectionTarget(binding, 'input', state.input.selected_target!);
		await selectAudioDirectionTarget(binding, 'output', state.output.selected_target!);

		expect(calls).toEqual(['input-enabled', 'output-enabled', 'input-target', 'output-target']);
	});
});
