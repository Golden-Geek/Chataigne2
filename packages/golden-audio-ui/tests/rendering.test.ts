import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import AudioDeviceSelector from '../AudioDeviceSelector.svelte';
import MockAudioDeviceConsumer from '../MockAudioDeviceConsumer.svelte';
import { MockAudioDeviceInspectorAdapter, createMockAudioDeviceState } from '../mock.svelte';
import type { AudioDeviceReadiness } from '../generated';

const renderState = (
	readiness: AudioDeviceReadiness,
	options: { discovery?: boolean; permissionDenied?: boolean } = {}
): string => {
	const state = createMockAudioDeviceState();
	state.discovery_in_progress = options.discovery ?? false;
	state.output = {
		...state.output,
		readiness,
		permission: options.permissionDenied ? 'denied' : state.output.permission,
		error:
			readiness === 'failed'
				? {
						category: 'stream_negotiation_failed',
						message: 'Could not negotiate the selected format.',
						technical_detail: 'mock backend rejected 96 kHz'
					}
				: null
	};
	return render(AudioDeviceSelector, {
		props: { binding: new MockAudioDeviceInspectorAdapter(state) }
	}).body;
};

describe('AudioDeviceSelector rendering', () => {
	it.each([
		['disabled', 'Disabled'],
		['discovering', 'Discovering'],
		['missing', 'Missing'],
		['unavailable', 'Unavailable'],
		['busy', 'Busy'],
		['ready', 'Ready']
	] as const)('renders the %s stream state', (readiness, label) => {
		expect(renderState(readiness)).toContain(label);
	});

	it('renders discovery and permission state as text, not color alone', () => {
		const body = renderState('permission_denied', {
			discovery: true,
			permissionDenied: true
		});
		expect(body).toContain('Discovering audio devices');
		expect(body).toContain('Permission denied');
		expect(body).toContain('Permission: Denied');
	});

	it('renders backend absence and structured diagnostic detail', () => {
		const state = createMockAudioDeviceState();
		state.backends[0] = {
			...state.backends[0],
			state: 'missing_server',
			detail: 'Start the mock audio service.'
		};
		const adapter = new MockAudioDeviceInspectorAdapter(state);
		let body = render(AudioDeviceSelector, {
			props: { binding: adapter }
		}).body;
		expect(body).toContain('Missing server');
		expect(body).toContain('Start the mock audio service.');

		body = renderState('failed');
		expect(body).toContain('Could not negotiate the selected format.');
		expect(body).toContain('mock backend rejected 96 kHz');
		expect(body).toContain('Copy technical detail');
	});

	it('uses native labelled controls and announces status changes', () => {
		const body = renderState('ready');
		expect(body).toContain('<select');
		expect(body).toContain('<label');
		expect(body).toContain('role="status"');
		expect(body).toContain('aria-live="polite"');
		expect(body).toContain('Input device');
		expect(body).toContain('Output device');
	});

	it('renders the standalone mock consumer without any Chataigne adapter', () => {
		const body = render(MockAudioDeviceConsumer).body;
		expect(body).toContain('Golden Audio mock consumer');
		expect(body).toContain('Studio Input');
		expect(body).toContain('Studio Output');
	});
});
