import { describe, expect, it } from 'vitest';

import type { ProcessorLaneCatalogEntryDto } from '../../../state_machine/generated/ProcessorLaneCatalogEntryDto';
import {
	formulaPreviewSessionStore,
	processorPreviewLaneOptions
} from '../formulaPreviewSessionStore.svelte';

const lane = (processorId: string, index: number): ProcessorLaneCatalogEntryDto => ({
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
	is_default_preview: index === 0,
	is_processor_preview: index === 0
});

describe('processor preview checkpoints', () => {
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

	it('follows the effective processor preview and falls back to the multiplex default', () => {
		const defaultLane = { ...lane('processor', 1), is_processor_preview: false };
		const overrideLane = {
			...lane('processor', 2),
			is_default_preview: false,
			is_processor_preview: true
		};
		const multiplexDefault = { ...lane('processor', 0), is_processor_preview: false };
		const lanes = processorPreviewLaneOptions([multiplexDefault, defaultLane, overrideLane]);

		expect(formulaPreviewSessionStore.processorLane(42, lanes)?.id).toBe(lanes[2].id);

		const withoutOverride = lanes.map((option) => ({ ...option, isProcessorPreview: false }));
		expect(formulaPreviewSessionStore.processorLane(42, withoutOverride)?.id).toBe(lanes[0].id);
	});
});
