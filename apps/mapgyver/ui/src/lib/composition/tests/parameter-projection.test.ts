import assert from 'node:assert/strict';
import { test } from 'node:test';
import { editorParameterProjection } from 'golden_ui/store/editor-parameter-projection';
import { projectInstanceParameters } from '../parameter-projection.ts';
import type { GraphState } from 'golden_ui/store/graph.svelte';
import type { ParamValue, UiNodeDto, UiParamDto } from 'golden_ui/types';
import type { InstanceSnapshot } from '../../media/generated/InstanceSnapshot';

test('Inspector drafts reach Composition geometry without waiting for the queued backend read', () => {
	const bindings = {
		instance: 'instance',
		position: 'position',
		scale: 'scale',
		rotation_degrees: 'rotation',
		pivot: 'pivot',
		opacity: 'opacity'
	};
	const instance: InstanceSnapshot = {
		reference: { composition: { media: 'media', channel: 'channel' }, instance: 'instance' },
		label: 'Owned source',
		definition: {
			source: { media: 'source', channel: 'source-channel' },
			blend: 'Multiply',
			opacity: 1,
			enabled: true,
			transform: { position: [1, 2], scale: [-2, 3], rotation_degrees: 27, pivot: [0.2, 0.7] }
		}
	};
	const before = structuredClone(instance);
	const graph = {
		nodesById: {
			getByUuid: (uuid: string) =>
				uuid === 'position'
					? ({ node_id: 2, uuid, data: { kind: 'parameter' } } as UiNodeDto)
					: undefined
		},
		paramsById: new Map([[2, { value: { kind: 'vec2', value: [1, 2] } } as UiParamDto]]),
		requiresResync: false,
		lastEventTime: { tick: 1, micro: 0, seq: 0 }
	} as unknown as GraphState;
	let draft: ParamValue | undefined = { kind: 'vec2', value: [71, 83] };
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		loading: { visible: false },
		graph: { state: graph },
		editOperations: { get: () => draft, enabled: () => undefined }
	};
	const reader = editorParameterProjection(host, 1, { tick: 2, micro: 0, seq: 0 });
	const projected = projectInstanceParameters(instance, bindings, reader);
	assert.deepEqual(projected.definition.transform.position, [71, 83]);
	assert.deepEqual(projected.definition.transform.scale, before.definition.transform.scale);
	assert.deepEqual(projected.definition.source, before.definition.source);
	assert.equal(projected.definition.blend, 'Multiply');
	assert.deepEqual(instance, before, 'presentation cannot author or mutate the backend reply');
	draft = undefined;
	assert.deepEqual(
		projectInstanceParameters(instance, bindings, reader),
		before,
		'cancelled draft cannot linger'
	);
	assert.equal(
		projectInstanceParameters(instance, undefined, reader),
		instance,
		'unbound objects retain their authoritative projection'
	);
});

test('Composition enabled presentation follows its owned media metadata rather than a Parameter binding', () => {
	const instance: InstanceSnapshot = {
		reference: { composition: { media: 'owner', channel: 'output' }, instance: 'content' },
		label: 'Content',
		definition: {
			source: null,
			blend: 'Normal',
			opacity: 1,
			enabled: true,
			transform: { position: [0, 0], scale: [1, 1], pivot: [0.5, 0.5], rotation_degrees: 0 }
		}
	};
	const graph = {
		nodesById: {
			getByUuid: (uuid: string) =>
				uuid === 'content'
					? {
							node_id: 3,
							uuid,
							meta: { enabled: false },
							data: { kind: 'node', node_type: 'media' }
						}
					: undefined
		},
		paramsById: new Map(),
		requiresResync: false,
		lastEventTime: { tick: 3, micro: 0, seq: 0 }
	} as unknown as GraphState;
	let enabled: boolean | undefined;
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		loading: { visible: false },
		graph: { state: graph },
		editOperations: { get: () => undefined, enabled: () => enabled }
	};
	const reader = editorParameterProjection(host, 1, { tick: 2, micro: 0, seq: 0 });
	const bindings = {
		instance: 'content',
		position: 'position',
		scale: 'scale',
		rotation_degrees: 'rotation',
		pivot: 'pivot',
		opacity: 'opacity'
	};
	assert.equal(projectInstanceParameters(instance, bindings, reader).definition.enabled, false);
	enabled = true;
	assert.equal(projectInstanceParameters(instance, bindings, reader).definition.enabled, true);
	assert.equal(instance.definition.enabled, true);
});
