import { describe, expect, it } from 'vitest';
import { EditorParameterObserver } from '../editor-parameter-observer';
import { VersionedNodeMap } from '../graph-index';
import { IdentifiedNodeMap } from '../graph-node-index';
import type { GraphState } from '../graph.svelte';
import type { UiNodeDto, UiParamDto } from '../../types';

function fixture(): GraphState {
	return {
		rootId: 1,
		nodesById: new IdentifiedNodeMap([
			[1, { uuid: 'owner' } as UiNodeDto],
			[2, { uuid: 'nested' } as UiNodeDto],
			[3, { uuid: 'parameters' } as UiNodeDto],
			[4, { uuid: 'value' } as UiNodeDto],
			[5, { uuid: 'unrelated' } as UiNodeDto]
		]),
		parentById: new VersionedNodeMap([
			[2, 1],
			[3, 2],
			[4, 3]
		]),
		paramsById: new VersionedNodeMap([[4, { value: { kind: 'int', value: 1 } } as UiParamDto]]),
		childrenById: new VersionedNodeMap(),
		structuralChildrenById: new VersionedNodeMap(),
		requiresResync: false
	};
}
function value(graph: GraphState, id: number, next: number): GraphState {
	const params = graph.paramsById.fork();
	params.set(id, { value: { kind: 'int', value: next } } as UiParamDto);
	return { ...graph, paramsById: params };
}

describe('shared editor Parameter observation', () => {
	it('follows nested owned Parameters and never echoes an unchanged query reply', () => {
		const reader = new EditorParameterObserver();
		const initial = fixture();
		expect(reader.observe(initial, ['owner'])).toBe(false);
		const edited = value(initial, 4, 2);
		expect(reader.observe(edited, ['owner'])).toBe(true);
		expect(reader.observe(edited, ['owner'])).toBe(false);
		expect(reader.observe({ ...edited }, ['owner'])).toBe(false);
	});
	it('can restrict metadata views to immediate Parameter ownership', () => {
		const reader = new EditorParameterObserver();
		const initial = fixture();
		reader.observe(initial, ['owner'], 2);
		expect(reader.observe(value(initial, 4, 2), ['owner'], 2)).toBe(false);
		reader.reset();
		reader.observe(initial, ['nested'], 2);
		expect(reader.observe(value(initial, 4, 2), ['nested'], 2)).toBe(true);
	});
	it('handles removed Parameters, new owner scope and observer reset', () => {
		const reader = new EditorParameterObserver();
		const initial = fixture();
		reader.observe(initial, ['owner']);
		const params = initial.paramsById.fork();
		params.delete(4);
		expect(reader.observe({ ...initial, paramsById: params }, ['owner'])).toBe(true);
		expect(reader.observe(initial, ['other'])).toBe(false);
		reader.reset();
		expect(reader.observe(initial, ['owner'])).toBe(false);
	});
	it('uses changed trie paths at50k Parameters, ignoring unrelated edits', () => {
		const initial = fixture();
		for (let id = 10; id < 50_010; id++)
			initial.paramsById.set(id, { value: { kind: 'int', value: id } } as UiParamDto);
		const reader = new EditorParameterObserver();
		reader.observe(initial, ['owner']);
		let visits = 0;
		const get = initial.nodesById.get.bind(initial.nodesById);
		initial.nodesById.get = (id) => {
			visits++;
			return get(id);
		};
		const unrelated = value(initial, 25_000, -1);
		expect(reader.observe(unrelated, ['owner'])).toBe(false);
		expect(reader.observe(value(unrelated, 4, 2), ['owner'])).toBe(true);
		expect(visits).toBeLessThan(12);
	});
});
