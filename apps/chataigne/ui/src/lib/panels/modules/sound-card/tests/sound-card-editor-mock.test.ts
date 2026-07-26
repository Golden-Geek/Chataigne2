import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import SoundCardEditorMockHarness from '../SoundCardEditorMockHarness.svelte';

describe('Sound Card editor evidence harness', () => {
	it('renders deterministic devices, routing, meters, analysis, and diagnostics', () => {
		const body = render(SoundCardEditorMockHarness).body;

		expect(body).toContain('Sound Card evidence harness');
		expect(body).toContain('Studio Input');
		expect(body).toContain('Physical Input 1');
		expect(body).toContain('Input Left');
		expect(body).toContain('A4');
		expect(body).toContain('Spectrum for mock-output-left');
		expect(body).toContain('Dropped events');
		expect(body).toContain('Playback queue pressure');
		expect(body).toContain('Render timing');
		expect(body).toContain('Callback XRuns');
		expect(body).toContain('3.0 MiB');
	});
});
