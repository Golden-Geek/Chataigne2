import { describe, expect, it, vi } from 'vitest';
import { fixtureNode } from 'golden_ui/testing/session';
import { VersionedNodeMap } from 'golden_ui/store/graph-index';
import type { UiNodeDto } from 'golden_ui';
import { SpatializerModuleCatalog } from '../module-catalog';

const module = (id: number, label: string) => fixtureNode(id, label, [], 'spatializer_module');

describe('Spatializer module catalog', () => {
	it('does not enumerate unrelated graph records again for matrix updates', () => {
		const nodes = new VersionedNodeMap<UiNodeDto>();
		nodes.set(0, module(0, 'Spatializer'));
		for (let id = 1; id <= 10000; id++) nodes.set(id, fixtureNode(id, 'Cell', [], 'parameter'));
		const catalog = new SpatializerModuleCatalog(),
			first = catalog.update(nodes, 1);
		const next = nodes.fork();
		next.set(10001, fixtureNode(10001, 'New cell', [], 'parameter'));
		next.set(9999, {
			...next.get(9999)!,
			meta: { ...next.get(9999)!.meta, label: 'Updated cell' }
		});
		const values = vi.spyOn(next, 'values').mockImplementation(() => {
			throw new Error('Full graph enumeration');
		});
		expect(catalog.update(next, 1)).toBe(first);
		expect(values).not.toHaveBeenCalled();
		expect(first.map((node) => node.node_id)).toEqual([0]);
	});
	it('tracks add, rename, replacement, retyping and deletion with current identities', () => {
		const catalog = new SpatializerModuleCatalog(),
			nodes = new VersionedNodeMap<UiNodeDto>();
		nodes.set(1, module(1, 'Zulu'));
		catalog.update(nodes, 1);
		let next = nodes.fork();
		next.set(2, module(2, 'Alpha'));
		expect(catalog.update(next, 1).map((n) => n.node_id)).toEqual([2, 1]);
		let previous = next;
		next = previous.fork();
		const replaced = { ...module(1, 'Aardvark'), uuid: 'new-identity' };
		next.set(1, replaced);
		expect(catalog.update(next, 1)).toEqual([replaced, previous.get(2)]);
		previous = next;
		next = previous.fork();
		next.set(2, fixtureNode(2, 'Not a module', [], 'folder'));
		expect(catalog.update(next, 1)).toEqual([replaced]);
		previous = next;
		next = previous.fork();
		next.delete(1);
		expect(catalog.update(next, 1)).toEqual([]);
	});
	it('retires the previous project and disconnected state', () => {
		const catalog = new SpatializerModuleCatalog(),
			nodes = new VersionedNodeMap<UiNodeDto>();
		nodes.set(1, module(1, 'Old'));
		catalog.update(nodes, 1);
		const replacement = new VersionedNodeMap<UiNodeDto>([[2, module(2, 'New')]]);
		expect(catalog.update(replacement, 2).map((n) => n.node_id)).toEqual([2]);
		expect(catalog.update(null, 2)).toEqual([]);
		expect(catalog.update(replacement, 2).map((n) => n.node_id)).toEqual([2]);
	});
});
