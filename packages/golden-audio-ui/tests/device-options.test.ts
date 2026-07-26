import { describe, expect, it } from 'vitest';
import {
	audioDeviceOptionGroups,
	audioDeviceTargetKey,
	findAudioDeviceTarget
} from '../device-options';
import { createMockAudioDeviceState } from '../mock.svelte';

describe('audio device options', () => {
	it('groups compatible devices by backend and preserves stable target IDs', () => {
		const state = createMockAudioDeviceState();
		const groups = audioDeviceOptionGroups(state, 'input');
		const deviceTarget = state.devices[0].target;
		const key = audioDeviceTargetKey(deviceTarget);

		expect(groups).toHaveLength(1);
		expect(groups[0].backendLabel).toBe('Mock Audio');
		expect(groups[0].options.map((option) => option.label)).toEqual([
			'System Default Input',
			'Studio Input'
		]);
		expect(findAudioDeviceTarget(groups, key)).toEqual(deviceTarget);
		expect(groups[0].options.some((option) => option.label === 'Studio Output')).toBe(false);
	});

	it('keeps a persisted missing selection visible without rewriting it', () => {
		const state = createMockAudioDeviceState();
		const missingTarget = {
			kind: 'device' as const,
			backend: 'removed-backend',
			device: 'stable-missing-id'
		};
		state.output = {
			...state.output,
			selected_target: missingTarget,
			selected_label: 'Disconnected Interface',
			readiness: 'missing',
			active_target: null
		};

		const groups = audioDeviceOptionGroups(state, 'output');
		const option = groups.flatMap((group) => group.options).find((candidate) => candidate.missing);

		expect(option?.label).toBe('Disconnected Interface');
		expect(option?.target).toEqual(missingTarget);
	});
});
