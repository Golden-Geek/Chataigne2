import { describe, expect, it } from 'vitest';
import { indexTreeRows, treeNavigationIntent } from '../tree-navigation';
import type { OutlinerRow } from '../visible-rows';

const row = (nodeId: number, indent: number, expanded?: boolean): OutlinerRow => ({
	nodeId,
	indent,
	level: indent,
	parentId: null,
	expanded
});

describe('logical tree navigation', () => {
	const rows = [row(1, 0, true), row(2, 1, false), row(3, 1), row(4, 0)];
	const positions = indexTreeRows(rows);
	const intent = (id: number | null, key: string) => treeNavigationIntent(rows, positions, id, key);
	it('uses visible order across descendants and root boundaries', () => {
		expect(intent(1, 'ArrowDown')).toEqual({ kind: 'focus', nodeId: 2 });
		expect(intent(3, 'ArrowDown')).toEqual({ kind: 'focus', nodeId: 4 });
		expect(intent(4, 'ArrowUp')).toEqual({ kind: 'focus', nodeId: 3 });
		expect(intent(3, 'Home')).toEqual({ kind: 'focus', nodeId: 1 });
		expect(intent(2, 'End')).toEqual({ kind: 'focus', nodeId: 4 });
		expect(intent(4, 'ArrowDown')).toEqual({ kind: 'focus', nodeId: 4 });
	});
	it('expands before entering a branch and collapses before returning to a parent', () => {
		expect(intent(2, 'ArrowRight')).toEqual({ kind: 'expand', nodeId: 2, expanded: true });
		expect(intent(1, 'ArrowRight')).toEqual({ kind: 'focus', nodeId: 2 });
		expect(intent(1, 'ArrowLeft')).toEqual({ kind: 'expand', nodeId: 1, expanded: false });
		expect(intent(2, 'ArrowLeft')).toEqual({ kind: 'focus', nodeId: 1 });
		expect(intent(3, 'ArrowRight')).toBeNull();
	});
	it('activates only on Enter or Space', () => {
		for (const key of ['Enter', ' '])
			expect(intent(3, key)).toEqual({ kind: 'activate', nodeId: 3 });
		expect(intent(3, 'Tab')).toBeNull();
		expect(treeNavigationIntent([], new Map(), null, 'Home')).toBeNull();
	});
	it('indexes filtered and hoisted siblings using visible indentation rather than graph parents', () => {
		const rows = [row(10, 0, true), row(30, 1), row(40, 1), row(50, 0)];
		rows[1].parentId = 20;
		const indexed = indexTreeRows(rows);
		expect(indexed.get(30)).toMatchObject({ parentId: 10, position: 1, siblings: 2 });
		expect(indexed.get(40)).toMatchObject({ parentId: 10, position: 2, siblings: 2 });
		expect(indexed.get(50)).toMatchObject({ parentId: null, position: 2, siblings: 2 });
		expect(treeNavigationIntent(rows, indexed, 30, 'ArrowLeft')).toEqual({
			kind: 'focus',
			nodeId: 10
		});
	});
	it('navigates a 100k projection with bounded row reads after indexing', () => {
		let reads = 0;
		const rows = new Proxy(
			Array.from({ length: 100000 }, (_, id) => row(id, 0)),
			{
				get(target, property, receiver) {
					if (typeof property === 'string' && /^\d+$/.test(property)) reads++;
					return Reflect.get(target, property, receiver);
				}
			}
		);
		const indexed = indexTreeRows(rows);
		reads = 0;
		for (let id = 1; id < 100; id++) treeNavigationIntent(rows, indexed, id, 'ArrowDown');
		expect(reads).toBeLessThanOrEqual(100);
	});
});
