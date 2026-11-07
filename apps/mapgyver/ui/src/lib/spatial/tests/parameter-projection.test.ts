import assert from 'node:assert/strict';
import { test } from 'node:test';
import { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import { projectMatrixWindow, projectCellLayoutPage } from '../parameter-projection.ts';
import type { GraphState } from 'golden_ui/store/graph.svelte';
import type { ParamValue, UiNodeDto } from 'golden_ui/types';
import type { MatrixViewportSnapshot } from '../../media/generated/MatrixViewportSnapshot';
import type { MatrixCellLayoutPage } from '../../media/generated/MatrixCellLayoutPage';

function fixture() {
	const values = new Map<string, ParamValue>([
		['layer-position', { kind: 'vec2', value: [70, 90] }],
		['layer-size', { kind: 'vec2', value: [500, 400] }],
		['cell-position', { kind: 'vec2', value: [30, 40] }],
		['cell-size', { kind: 'vec2', value: [80, 60] }]
	]);
	let enabled = false;
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		loading: { visible: false },
		graph: {
			state: {
				requiresResync: false,
				nodesById: {
					getByUuid: (uuid: string) =>
						values.has(uuid)
							? ({ node_id: 1, uuid, data: { kind: 'parameter' } } as UiNodeDto)
							: undefined
				}
			} as GraphState
		},
		editOperations: { get: (_id: number, uuid: string) => values.get(uuid), enabled: () => enabled }
	};
	const parameters = editorParameterProjection(host, 1, { tick: 1, micro: 0, seq: 0 });
	const row = {
		item: {
			id: { kind: 'Layer', id: 'layer' },
			label: 'Layer',
			settings: { enabled: true },
			layer: { position: [1, 2], size: { width: 320, height: 240 }, duration_ms: 777 }
		}
	};
	const cell = {
		label: 'Cell',
		extent: { width: 40, height: 20 },
		preview: { kind: 'MatrixCell', media: 'matrix', cell: 'cell' },
		view: {
			cell: {
				id: 'cell',
				definition: {
					source: { media: 'source', channel: 'output' },
					settings: { position: [3, 4], size: null, opacity: 0.4, enabled: true, fit_mode: 'Fill' }
				}
			}
		}
	};
	const bindings = [
		{ item: 'layer', position: 'layer-position', size: 'layer-size' },
		{ item: 'cell', position: 'cell-position', size: 'cell-size' }
	];
	return {
		parameters,
		row,
		cell,
		bindings,
		enable: () => {
			enabled = true;
		}
	};
}

test('visible and independently selected Layer geometry projects the same typed drafts without authoring', () => {
	const { parameters, row, bindings } = fixture();
	const source = {
		rows: [row],
		selected: { row },
		label: 'Stable membership'
	} as unknown as MatrixViewportSnapshot;
	const before = structuredClone(source);
	const projected = projectMatrixWindow(
		source,
		new Map(bindings.map((value) => [value.item, value])),
		parameters
	);
	assert.deepEqual(projected.rows[0].item.layer?.position, [70, 90]);
	assert.deepEqual(projected.selected?.row?.item.layer?.size, { width: 500, height: 400 });
	assert.equal(projected.rows[0].item.layer?.duration_ms, 777);
	assert.deepEqual(source, before);
	assert.equal(projectMatrixWindow(source, new Map(), parameters).rows[0], source.rows[0]);
});

test('Cell position drafts preserve natural sizing until its exact Size Parameter is enabled', () => {
	const { parameters, row, cell, bindings, enable } = fixture();
	const source = {
		row,
		cells: [cell],
		selected: cell,
		parameters: bindings
	} as unknown as MatrixCellLayoutPage;
	const before = structuredClone(source);
	const natural = projectCellLayoutPage(source, parameters);
	assert.deepEqual(natural.cells[0].view.cell.definition.settings.position, [30, 40]);
	assert.equal(natural.selected?.view.cell.definition.settings.size, null);
	assert.deepEqual(natural.cells[0].extent, { width: 40, height: 20 });
	enable();
	const explicit = projectCellLayoutPage(source, parameters);
	assert.deepEqual(explicit.selected?.view.cell.definition.settings.size, {
		width: 80,
		height: 60
	});
	assert.equal(explicit.selected?.view.cell.definition.settings.opacity, 0.4);
	assert.deepEqual(
		explicit.selected?.view.cell.definition.source,
		cell.view.cell.definition.source
	);
	assert.deepEqual(source, before);
});
