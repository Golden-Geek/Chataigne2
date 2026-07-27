import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import SoundCardRoutingEvidence from '../SoundCardRoutingEvidence.svelte';

describe('Sound Card routing evidence', () => {
	it('matches the requested named-channel patch-bay vocabulary and shape', () => {
		const body = render(SoundCardRoutingEvidence).body;

		expect(body).toContain('Output Routing');
		expect(body).toContain('Output 1');
		expect(body).toContain('Input 6');
		expect(body).toContain('5 connected');
		expect(body.match(/connection-visible/g)).toHaveLength(5);
		expect(body).not.toMatch(/virtual|profile|monitoring|playback|diagnostic/i);
	});
});
