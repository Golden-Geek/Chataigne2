import { describe, expect, it } from 'vitest';

import type { ProcessorLaneSummaryDto } from '../generated/ProcessorLaneSummaryDto';
import { processorPreviewLaneOptions } from './formulaPreviewSessionStore.svelte';

const lane = (processorId: string, index: number): ProcessorLaneSummaryDto => ({
	processor_id: processorId,
	context_key: {
		parts: [
			{
				axis_id: 'device',
				axis_label: 'Device',
				item_id: `device-${index}`,
				item_label: `Device ${index + 1}`,
				index
			}
		]
	},
	label: `Device ${index + 1}`,
	has_memory: true,
	last_tick: 1n,
	diagnostics_count: 0
});

describe('Phase 5 processor preview checkpoints', () => {
	it('projects P50-L1 summaries without collapsing processor identity', () => {
		const projected = Array.from({ length: 50 }, (_, index) =>
			processorPreviewLaneOptions([lane(`processor-${index}`, 0)])
		);

		expect(projected).toHaveLength(50);
		expect(projected.every((options) => options.length === 1)).toBe(true);
	});

	it('projects all P5-L127 lane choices for the inspector', () => {
		const processors = Array.from({ length: 5 }, (_, processorIndex) =>
			processorPreviewLaneOptions(
				Array.from({ length: 127 }, (_, laneIndex) =>
					lane(`processor-${processorIndex}`, laneIndex)
				)
			)
		);

		expect(processors).toHaveLength(5);
		expect(processors.every((options) => options.length === 127)).toBe(true);
		expect(new Set(processors[0].map((option) => option.id)).size).toBe(127);
	});
});
