import { describe, expect, it } from 'vitest';
import { VersionedNodeMap } from '../graph-index';

describe('published graph index changes', () => {
	it('skips shared history and reports additions, replacements and removals', () => {
		const before = new VersionedNodeMap<number>();
		for (let id = 0; id < 50_000; id++) before.set(id, id);
		const after = before.fork();
		after.set(12, -12);
		after.delete(45_000);
		after.set(2 ** 40, 7);
		const changes: [number, number | undefined, number | undefined][] = [];
		after.forEachChange(before, (...entry) => changes.push(entry));
		expect(changes.sort((a, b) => a[0] - b[0])).toEqual([
			[12, -12, 12],
			[45_000, undefined, 45_000],
			[2 ** 40, 7, undefined]
		]);
		expect(before.get(12)).toBe(12);
		expect(before.has(45_000)).toBe(true);
		expect(before.has(2 ** 40)).toBe(false);
		const unchanged: number[] = [];
		after.fork().forEachChange(after, (id) => unchanged.push(id));
		expect(unchanged).toEqual([]);
	});
	it('distinguishes a present undefined value from absence and supports clear', () => {
		const before = new VersionedNodeMap<number | undefined>([
			[1, undefined],
			[2, 2]
		]);
		const after = before.fork();
		after.delete(1);
		after.set(3, undefined);
		const changed: number[] = [];
		after.forEachChange(before, (id) => changed.push(id));
		expect(changed.sort()).toEqual([1, 3]);
		const cleared = after.fork();
		cleared.clear();
		const removed: number[] = [];
		cleared.forEachChange(after, (id) => removed.push(id));
		expect(removed.sort()).toEqual([2, 3]);
	});
	it('matches a full-map reference across independent forks and replacements', () => {
		let previous = new VersionedNodeMap<number>();
		for (let round = 0; round < 32; round++) {
			const next = round % 9 === 0 ? new VersionedNodeMap<number>() : previous.fork();
			for (let index = 0; index < 50; index++) {
				const key = (round * 79 + index * 137) % 1024;
				if (index % 4 === 0) next.delete(key);
				else next.set(key, round * 100 + index);
			}
			const expected = [...new Set([...previous.keys(), ...next.keys()])]
				.filter(
					(key) =>
						previous.has(key) !== next.has(key) || !Object.is(previous.get(key), next.get(key))
				)
				.sort((a, b) => a - b);
			const actual: number[] = [];
			next.forEachChange(previous, (key) => actual.push(key));
			expect(actual.sort((a, b) => a - b)).toEqual(expected);
			previous = next;
		}
	});
});
