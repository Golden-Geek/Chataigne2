import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import SoundCardRouteMatrix from '../SoundCardRouteMatrix.svelte';
import type { SoundCardMatrixEndpoint, SoundCardRouteRecord } from '../sound-card-editor-model';

const sources = Array.from({ length: 256 }, (_, index): SoundCardMatrixEndpoint => ({
	key: `int:${index + 1}`,
	label: `Source ${index + 1}`,
	value: { kind: 'int', value: index + 1 }
}));

const destinations = Array.from({ length: 256 }, (_, index): SoundCardMatrixEndpoint => ({
	key: `reference:output-${index + 1}`,
	label: `Output ${index + 1}`,
	value: { kind: 'reference', uuid: `output-${index + 1}` }
}));

const rows: readonly SoundCardRouteRecord[] = [
	{
		id: 9,
		label: 'Route 1',
		source: 'Source 1',
		destination: 'Output 1',
		gainDb: -2,
		sourceKey: 'int:1',
		destinationKey: 'reference:output-1',
		sourceValue: { kind: 'int', value: 1 },
		destinationValue: { kind: 'reference', uuid: 'output-1' },
		gainParameterId: 10,
		gainEventBehaviour: 'Coalesce'
	}
];

const renderMatrix = (active = true): string =>
	render(SoundCardRouteMatrix, {
		props: {
			title: 'Large route matrix',
			rows,
			sources,
			destinations,
			parent: 1,
			nodeType: 'sound_card_playback_route',
			sourceDeclId: 'source_channel',
			destinationDeclId: 'virtual_output',
			active
		}
	}).body;

describe('SoundCardRouteMatrix', () => {
	it('represents a 256 by 256 matrix without mounting 65,536 DOM controls', () => {
		const body = renderMatrix();

		expect(body).toContain('aria-rowcount="256"');
		expect(body).toContain('aria-colcount="256"');
		expect(body).toContain('<canvas');
		expect(body.match(/<option/g)?.length).toBe(512);
		expect(body.match(/<(button|input|select)/g)?.length).toBeLessThan(600);
		expect(body).not.toContain('65536');
	});

	it('retains authored routes while presenting inactive input signal flow', () => {
		const body = renderMatrix(false);

		expect(body).toContain('Input is disabled');
		expect(body).toContain('Source 1');
		expect(body).toContain('Output 1');
		expect(body).toContain('Remove route from Source 1 to Output 1');
	});
});
