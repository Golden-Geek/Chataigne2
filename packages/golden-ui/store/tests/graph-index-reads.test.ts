import { describe, expect, it } from 'vitest';
import { VersionedNodeMap } from '../graph-index';

describe('persistent graph index reads', () => {
	it('keeps misses coherent through insertion, replacement, deletion and clear', () => {
		const map = new VersionedNodeMap<string | undefined>();
		expect(map.get(1)).toBeUndefined();
		map.set(1, 'first');
		expect(map.get(1)).toBe('first');
		map.set(1, 'second');
		expect(map.get(1)).toBe('second');
		expect(map.delete(1)).toBe(true);
		expect(map.get(1)).toBeUndefined();
		map.set(1, undefined);
		expect(map.get(1)).toBeUndefined();
		expect(map.has(1)).toBe(true);
		map.clear();
		expect(map.get(1)).toBeUndefined();
		expect(map.has(1)).toBe(false);
		map.set(1, 'third');
		expect(map.get(1)).toBe('third');
		map.clear();
		expect(map.get(1)).toBeUndefined();
	});

	it('compares the entire node identity across generations and shared prefixes', () => {
		const keys = [1, 33, 2 ** 32 + 1, 2 ** 33 + 1, Number.MAX_SAFE_INTEGER];
		const map = new VersionedNodeMap(keys.map((key) => [key, String(key)] as const));
		for (let round = 0; round < 4; round++) {
			for (const key of keys) expect(map.get(key)).toBe(String(key));
		}
		map.delete(33);
		expect(map.get(33)).toBeUndefined();
		expect(map.get(1)).toBe('1');
		expect(map.size).toBe(4);
	});

	it('isolates reads when either side of a fork is mutated', () => {
		const original = new VersionedNodeMap([
			[1, 'original'],
			[2, 'shared']
		]);
		expect(original.get(1)).toBe('original');
		const child = original.fork();
		const sibling = original.fork();
		expect(child.get(2)).toBe('shared');
		original.set(1, 'parent');
		original.delete(2);
		child.set(2, 'child');
		expect(child.get(1)).toBe('original');
		expect(child.get(2)).toBe('child');
		expect(sibling.get(1)).toBe('original');
		expect(sibling.get(2)).toBe('shared');
		expect(original.get(1)).toBe('parent');
		expect(original.has(2)).toBe(false);
		child.clear();
		expect(sibling.get(2)).toBe('shared');
		expect(original.get(1)).toBe('parent');
	});

	it('preserves distinct identities sharing all 32 hash bits', () => {
		const first = 4_294_967_297,
			second = 2_654_435_760;
		const original = new VersionedNodeMap([
			[first, 'first'],
			[second, 'second']
		]);
		const next = original.fork();
		next.delete(first);
		expect(next.get(first)).toBeUndefined();
		expect(next.get(second)).toBe('second');
		expect(original.get(first)).toBe('first');
		const changed: number[] = [];
		next.forEachChange(original, (key) => changed.push(key));
		expect(changed).toEqual([first]);
		next.delete(second);
		expect(next.size).toBe(0);
		expect([...next]).toEqual([]);
	});

	it('allocates compact paths and bounds copies for one edit at 50k nodes', () => {
		const map = new VersionedNodeMap<number>();
		for (let key = 0; key < 50_000; key++) map.set(key, key);
		expect(map.mutationCopies()).toBeLessThan(75_000);
		const next = map.fork();
		next.set(25_000, -1);
		expect(next.mutationCopies()).toBeLessThanOrEqual(8);
		expect(map.get(25_000)).toBe(25_000);
		expect(next.get(25_000)).toBe(-1);
		const changed: number[] = [];
		next.forEachChange(map, (key) => changed.push(key));
		expect(changed).toEqual([25_000]);
	});
});
