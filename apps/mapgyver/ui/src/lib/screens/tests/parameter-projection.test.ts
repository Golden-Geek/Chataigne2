import assert from 'node:assert/strict';
import { test } from 'node:test';
import { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import type { GraphState } from 'golden_ui/store/graph.svelte';
import type { ParamValue, UiNodeDto } from 'golden_ui/types';
import { projectSurface } from '../parameter-projection.ts';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';

test('Surface projection shares crop, corner and opacity drafts without owning mapping validity or source policy', () => {
	const values = new Map<string, ParamValue>([
		['origin', { kind: 'vec2', value: [0.2, 0.3] }],
		['size', { kind: 'vec2', value: [0.5, 0.4] }],
		['corner', { kind: 'vec2', value: [0.1, 0.2] }],
		['opacity', { kind: 'float', value: 0.7 }]
	]);
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
		editOperations: {
			get: (_id: number, uuid: string) => values.get(uuid),
			enabled: () => undefined
		}
	};
	const source: SurfaceSnapshot = {
		reference: { screen: 'screen', surface: 'surface' },
		label: 'Surface',
		mapping_valid: false,
		definition: {
			source: { media: 'media', channel: 'channel' },
			enabled: true,
			calibration: 'Grid',
			opacity: 1,
			crop: { x: 0, y: 0, width: 1, height: 1 },
			quad: {
				corners: [
					{ x: 0, y: 0 },
					{ x: 1, y: 0 },
					{ x: 1, y: 1 },
					{ x: 0, y: 1 }
				]
			}
		}
	};
	const before = structuredClone(source);
	const binding = {
		surface: 'surface',
		crop_origin: 'origin',
		crop_size: 'size',
		corners: ['corner', 'missing', 'missing', 'missing'] as [string, string, string, string],
		opacity: 'opacity'
	};
	const parameters = editorParameterProjection(host, 1, { tick: 1, micro: 0, seq: 0 });
	const projected = projectSurface(source, binding, parameters);
	assert.deepEqual(projected.definition.crop, { x: 0.2, y: 0.3, width: 0.5, height: 0.4 });
	assert.deepEqual(projected.definition.quad.corners[0], { x: 0.1, y: 0.2 });
	assert.deepEqual(projected.definition.quad.corners[1], source.definition.quad.corners[1]);
	assert.equal(projected.definition.opacity, 0.7);
	assert.equal(projected.mapping_valid, false);
	assert.deepEqual(projected.definition.source, source.definition.source);
	assert.equal(projected.definition.calibration, 'Grid');
	assert.deepEqual(source, before);
	assert.equal(projectSurface(source, undefined, parameters), source);
	assert.deepEqual(
		projectSurface(
			source,
			binding,
			editorParameterProjection(host, 2, { tick: 1, micro: 0, seq: 0 })
		),
		source
	);
});
