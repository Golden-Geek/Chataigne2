import { describe, expect, it } from 'vitest';
import { editorParameterProjection } from '../editor-parameter-projection';
import { createEditorParameterProjection } from '../editor-parameter-projection.svelte';
import { VersionedNodeMap } from '../graph-index';
import { IdentifiedNodeMap } from '../graph-node-index';
import type { GraphState } from '../graph.svelte';
import type { ParamValue, UiNodeDto, UiParamDto } from '../../types';

function fixture() {
	let draft: ParamValue | undefined;
	const graph: GraphState = {
		rootId: 1,
		nodesById: new IdentifiedNodeMap([
			[2, { node_id: 2, uuid: 'position', data: { kind: 'parameter' } } as UiNodeDto]
		]),
		childrenById: new VersionedNodeMap(),
		structuralChildrenById: new VersionedNodeMap(),
		parentById: new VersionedNodeMap(),
		paramsById: new VersionedNodeMap([
			[2, { value: { kind: 'vec2', value: [10, 20] } } as UiParamDto]
		]),
		lastEventTime: { tick: 4, micro: 0, seq: 1 },
		requiresResync: false
	};
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		loading: { visible: false },
		graph: { state: graph },
		editOperations: {
			get: (id: number, uuid: string) => (id === 2 && uuid === 'position' ? draft : undefined)
		}
	};
	return {
		host,
		setDraft: (value?: ParamValue) => {
			draft = value;
		},
		reader: editorParameterProjection(host, 1, { tick: 5, micro: 0, seq: 0 })
	};
}

describe('shared editor Parameter projection', () => {
	it('fences graph delivery through the latest applied edit before releasing local presentation', () => {
		const { host } = fixture();
		const projection = createEditorParameterProjection(() => host);
		projection.applied()([
			{ success: true, status: 'applied', latest_event_time: { tick: 7, micro: 0, seq: 0 } }
		]);
		host.graph.state.lastEventTime = { tick: 6, micro: 0, seq: 0 };
		expect(
			projection.read(1, { tick: 5, micro: 0, seq: 0 }).value('position', 'vec2', [70, 80])
		).toEqual([70, 80]);
		host.graph.state.lastEventTime = { tick: 7, micro: 0, seq: 0 };
		expect(
			projection.read(1, { tick: 5, micro: 0, seq: 0 }).value('position', 'vec2', [70, 80])
		).toEqual([10, 20]);
	});
	it('rejects late receipts from an earlier document generation and keeps new Inspector drafts immediate', () => {
		const { host, setDraft } = fixture();
		const projection = createEditorParameterProjection(() => host),
			applied = projection.applied();
		host.snapshotGeneration++;
		applied([
			{ success: true, status: 'applied', latest_event_time: { tick: 9, micro: 0, seq: 0 } }
		]);
		expect(
			projection.read(2, { tick: 3, micro: 0, seq: 0 }).value('position', 'vec2', [1, 2])
		).toEqual([10, 20]);
		setDraft({ kind: 'vec2', value: [90, 100] });
		expect(
			projection.read(2, { tick: 8, micro: 0, seq: 0 }).value('position', 'vec2', [1, 2])
		).toEqual([90, 100]);
	});
	it('does not turn rejection or disconnection into an applied fence', () => {
		const { host } = fixture();
		const projection = createEditorParameterProjection(() => host);
		projection.applied()([
			{ success: false, status: 'rejected', latest_event_time: { tick: 9, micro: 0, seq: 0 } }
		]);
		host.status = 'disconnected';
		projection.applied()([
			{ success: true, status: 'applied', latest_event_time: { tick: 9, micro: 0, seq: 0 } }
		]);
		host.status = 'connected';
		expect(
			projection.read(1, { tick: 3, micro: 0, seq: 0 }).value('position', 'vec2', [1, 2])
		).toEqual([10, 20]);
	});
	it('presents Inspector drafts before either transport receipt or event delivery', () => {
		const { reader, setDraft } = fixture();
		setDraft({ kind: 'vec2', value: [30, 40] });
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([30, 40]);
	});
	it('never rewinds a backend reply to an older graph, then follows new committed values', () => {
		const { reader, host } = fixture();
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([1, 2]);
		host.graph.state = { ...host.graph.state, lastEventTime: { tick: 6, micro: 0, seq: 0 } };
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([10, 20]);
	});
	it('clears cancelled or rejected previews without retaining an authored copy', () => {
		const { reader, setDraft } = fixture();
		setDraft({ kind: 'vec2', value: [30, 40] });
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([30, 40]);
		setDraft();
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([1, 2]);
	});
	it.each(['disconnected', 'loading', 'generation', 'resync'])(
		'fences %s before reading drafts',
		(reason) => {
			const { reader, host, setDraft } = fixture();
			setDraft({ kind: 'vec2', value: [30, 40] });
			if (reason === 'disconnected') host.status = 'disconnected';
			if (reason === 'loading') host.loading.visible = true;
			if (reason === 'generation') host.snapshotGeneration++;
			if (reason === 'resync') host.graph.state.requiresResync = true;
			expect(reader.value('position', 'vec2', [1, 2])).toEqual([1, 2]);
		}
	);
	it('rejects incompatible types and reused numeric identities', () => {
		const { reader, host, setDraft } = fixture();
		setDraft({ kind: 'float', value: 99 });
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([1, 2]);
		host.graph.state.nodesById.set(2, {
			node_id: 2,
			uuid: 'replacement',
			data: { kind: 'parameter' }
		} as UiNodeDto);
		expect(reader.value('position', 'vec2', [1, 2])).toEqual([1, 2]);
	});
});
