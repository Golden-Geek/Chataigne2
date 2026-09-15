import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'svelte/server';
import type { UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import ProcessorFormulaInspectorHarness from './fixtures/ProcessorFormulaInspectorHarness.svelte';

afterEach(() => {
	appState.session = null;
});

describe('processor Formula inspector composition', () => {
	it('renders the ordinary processor child tree without preview data', () => {
		const node = {
			node_id: 7,
			uuid: 'processor-7',
			node_type: 'state_processor',
			meta: { label: 'Position Mapping' },
			children: []
		} as unknown as UiNodeDto;

		const { body } = render(ProcessorFormulaInspectorHarness, { props: { node } });
		expect(body).toContain('aria-label="Ordinary processor children"');
		expect(body).toContain('Ordinary manager and parameter nodes');
		expect(body).not.toContain('aria-label="Mapping inspector"');
	});
});
