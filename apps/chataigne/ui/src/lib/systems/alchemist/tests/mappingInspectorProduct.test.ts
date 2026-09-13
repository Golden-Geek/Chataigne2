import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import type { UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import type { ProcessorUiDto, StateMachinePreviewCatalogDto } from '../../state_machine/generated';
import ProcessorFormulaInspectorHarness from './fixtures/ProcessorFormulaInspectorHarness.svelte';
import { STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC } from '../preview/formulaOutputPreviewStore.svelte';

afterEach(() => {
	appState.session = null;
});

describe('processor Mapping product surface', () => {
	it('renders the backend catalog as Inputs, Filters, Outputs with typed stage shapes and diagnostics', () => {
		const node = {
			node_id: 7,
			uuid: 'processor-7',
			node_type: 'state_processor',
			meta: { label: 'Position Mapping' },
			children: []
		} as unknown as UiNodeDto;
		const shape = (kind: 'tuple' | 'compound', ids: string[], valueType: string) => ({
			kind,
			elements: ids.map((id) => ({
				id,
				label: id.toUpperCase(),
				value_type: valueType,
				minimum: null,
				maximum: null,
				unit: null,
				components: []
			}))
		});
		const input = shape('tuple', ['x', 'y', 'z'], 'float');
		const output = shape('compound', ['position'], 'vec3');
		const processor = {
			id: node.uuid,
			label: 'Position Mapping',
			standard_mapping: true,
			runtime_state: 'active',
			managed_regions: [
				{ id: 'inputs', label: 'Inputs', kind: 'input_set', filter_value_mode: 'routed' },
				{ id: 'filters', label: 'Filters', kind: 'filter_pipeline', filter_value_mode: 'tuple' },
				{ id: 'outputs', label: 'Outputs', kind: 'output_set', filter_value_mode: 'routed' }
			],
			managed_region_instances: [
				{ region_id: 'inputs', items: [] },
				{
					region_id: 'filters',
					items: [
						{
							id: 'item-1',
							anode_id: 'anode-1',
							label: 'Pack Vec3',
							enabled: true,
							anode_enabled: true
						}
					]
				},
				{ region_id: 'outputs', items: [] }
			],
			mapping_pipeline: {
				input,
				stages: [{ item_id: 'item-1', before: input, after: output }],
				output
			},
			mapping_diagnostics: [
				{ code: 'test', severity: 'warning', message: 'Target is missing', item_id: null }
			],
			mapping_outputs: []
		} as unknown as ProcessorUiDto;
		const catalog = {
			processors: [processor],
			processor_lanes: []
		} as StateMachinePreviewCatalogDto;
		appState.session = {
			status: 'connected',
			graph: { state: { nodesById: new Map([[node.node_id, node]]), parentById: new Map() } },
			getCustomEventSequence: () => 1,
			getCustomEventPayload: (topic: string) =>
				topic === STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC ? catalog : null
		} as unknown as typeof appState.session;

		const { body } = render(ProcessorFormulaInspectorHarness, { props: { node } });
		expect(body).toContain('aria-label="Mapping inspector"');
		expect(body).toContain('Inputs');
		expect(body).toContain('Filters');
		expect(body).toContain('Outputs');
		expect(body).toContain('Pack Vec3');
		expect(body).toContain('Tuple (float, float, float)');
		expect(body).toContain('Compound vec3');
		expect(body).toContain('Target is missing');
	});
});
