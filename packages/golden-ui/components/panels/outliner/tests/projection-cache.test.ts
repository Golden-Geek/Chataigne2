import { describe, expect, it } from 'vitest';
import type { UiNodeDto } from '../../../../types';
import { VersionedNodeMap } from '../../../../store/graph-index';
import { createOutlinerProjection, collectVisibleOutlinerRows } from '../visible-rows';

const node = (id: number, children: number[] = [], collapsed = false): UiNodeDto =>
	({
		node_id: id,
		uuid: `node-${id}`,
		children,
		meta: { label: String(id), presentation: { collapsed } }
	}) as UiNodeDto;
const options = (
	nodesById: Map<number, UiNodeDto>,
	opennessByNodeId: Record<string, boolean> = {}
) => ({
	graph: { rootId: 0, nodesById },
	opennessByNodeId,
	autoExpandAncestorNodeIds: new Set<number>(),
	initiallyExpandedDepth: 8
});

describe('panel-owned tree node projection cache', () => {
	it('retains row identity for metadata that cannot change visibility and reconciled openness', () => {
		const source = new VersionedNodeMap([
			[0, node(0, [1])],
			[1, node(1)]
		]);
		const collect = createOutlinerProjection();
		const first = collect(options(source));
		const after = source.fork();
		after.set(1, { ...source.get(1)!, meta: { ...source.get(1)!.meta, label: 'Renamed' } });
		expect(collect(options(after))).toBe(first);
		expect(collect(options(after, { '0': true }))).toBe(first);
	});
	it('matches full projection across branch edits, hidden ancestors, filtering and structural replacement', () => {
		let source = new VersionedNodeMap([
			[0, node(0, [1, 2, 3])],
			[1, node(1, [4, 5])],
			[2, node(2, [6])],
			[3, node(3)],
			[4, node(4)],
			[5, node(5)],
			[6, node(6)]
		]);
		const collect = createOutlinerProjection();
		let openness: Record<string, boolean> = {};
		const visible = (candidate: UiNodeDto) => candidate.node_id !== 1;
		for (let turn = 0; turn < 96; turn++) {
			const after = source.fork();
			const id = turn % 3;
			const original = after.get(id)!;
			after.set(id, {
				...original,
				meta: { ...original.meta, presentation: { collapsed: turn % 4 < 2 } }
			});
			if (turn % 9 === 0)
				after.set(0, { ...after.get(0)!, children: [...after.get(0)!.children].reverse() });
			source = after;
			openness = { ...openness, [String((turn + 1) % 3)]: turn % 5 < 3 };
			const config = {
				...options(source, openness),
				rowVisible: visible,
				matchesQuery:
					turn % 13 === 0 ? (candidate: UiNodeDto) => candidate.node_id === 5 : undefined,
				autoExpandAncestorNodeIds: new Set(turn % 7 === 0 ? [0, 1] : [])
			};
			expect(collect(config)).toEqual(collectVisibleOutlinerRows(config));
		}
	});
	it('reprojects only the edited branch and preserves unchanged suffix row objects', () => {
		const source = new VersionedNodeMap([
			[0, node(0, [1, 2])],
			[1, node(1, [3], true)],
			[2, node(2)],
			[3, node(3)]
		]);
		const collect = createOutlinerProjection();
		const first = collect(options(source));
		const expanded = collect(options(source, { '1': true }));
		expect(expanded.map((row) => row.nodeId)).toEqual([0, 1, 3, 2]);
		expect(expanded[0]).toBe(first[0]);
		expect(expanded[3]).toBe(first[2]);
	});
	it('does not revisit unchanged persistent records when branches open and close', () => {
		const source = new VersionedNodeMap(
			[node(0, [1, 2]), node(1, [3], true), node(2), node(3)].map((n) => [n.node_id, n])
		);
		const collect = createOutlinerProjection();
		let reads = 0;
		const get = source.get.bind(source);
		source.get = (id) => {
			reads++;
			return get(id);
		};
		expect(collect(options(source)).map((row) => row.nodeId)).toEqual([0, 1, 2]);
		expect(reads).toBe(3);
		expect(collect(options(source, { '1': true })).map((row) => row.nodeId)).toEqual([0, 1, 3, 2]);
		expect(reads).toBe(4);
		collect(options(source));
		expect(reads).toBe(4);
	});
	it('reconciles metadata, removals and new records from published change history', () => {
		const source = new VersionedNodeMap(
			[node(0, [1, 2]), node(1, [3]), node(2), node(3)].map((n) => [n.node_id, n])
		);
		const collect = createOutlinerProjection();
		collect(options(source));
		const after = source.fork();
		after.set(1, node(1, [3], true));
		expect(collect(options(after)).map((row) => row.nodeId)).toEqual([0, 1, 2]);
		const removed = after.fork();
		removed.delete(2);
		removed.set(0, node(0, [1, 4]));
		removed.set(4, node(4));
		expect(collect(options(removed)).map((row) => row.nodeId)).toEqual([0, 1, 4]);
	});
	it('does not retain records across an independent identity replacement or explicit release', () => {
		const collect = createOutlinerProjection();
		const source = new VersionedNodeMap([
			[0, node(0, [1])],
			[1, node(1)]
		]);
		collect(options(source));
		const replacement = new VersionedNodeMap([
			[0, node(0, [2])],
			[2, node(2)]
		]);
		expect(collect(options(replacement)).map((row) => row.nodeId)).toEqual([0, 2]);
		collect.clear();
		expect(collect(options(source)).map((row) => row.nodeId)).toEqual([0, 1]);
	});
	it('observes in-place changes in ordinary mutable maps', () => {
		const source = new Map([
			[0, node(0, [1])],
			[1, node(1)]
		]);
		const collect = createOutlinerProjection();
		collect(options(source));
		source.set(0, node(0, [2]));
		source.set(2, node(2));
		expect(collect(options(source)).map((row) => row.nodeId)).toEqual([0, 2]);
	});
});
