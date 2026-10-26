import { describe, expect, it } from 'vitest';
import { fixtureNode } from 'golden_ui/testing/session';
import { VersionedNodeMap } from 'golden_ui/store/graph-index';
import type { UiNodeDto } from 'golden_ui';
import { SpatializerChildIndex } from '../child-index';

function fixture(count = 4) {
	const nodesById = new VersionedNodeMap<UiNodeDto>(),
		parentById = new VersionedNodeMap<number>();
	const owner = fixtureNode(
		0,
		'Values',
		Array.from({ length: count }, (_, i) => i + 1)
	);
	nodesById.set(0, owner);
	for (let id = 1; id <= count; id++) {
		nodesById.set(id, { ...fixtureNode(id, `Value ${id}`), decl_id: `values/cell${id}` });
		parentById.set(id, 0);
	}
	const index = new SpatializerChildIndex();
	index.update({ nodesById, parentById }, 1);
	return { index, owner, nodesById, parentById };
}

describe('Spatializer retained declaration reads', () => {
	it('resolves a large folder once and reads current records after ordinary publications', () => {
		const f = fixture(10000);
		for (let id = 1; id <= 10000; id++)
			expect(f.index.child(f.owner, `cell${id}`)?.node_id).toBe(id);
		expect(f.index.builds).toBe(1);
		const nodes = f.nodesById.fork(),
			updated = { ...nodes.get(9999)!, meta: { ...nodes.get(9999)!.meta, enabled: false } };
		nodes.set(9999, updated);
		f.index.update({ nodesById: nodes, parentById: f.parentById }, 1);
		expect(f.index.child(f.owner, 'cell9999')).toBe(updated);
		expect(f.index.builds).toBe(1);
	});
	it('invalidates declarations and labels, preserving first-child lookup semantics', () => {
		const f = fixture();
		expect(f.index.child(f.owner, 'Value 1')?.node_id).toBe(1);
		const nodes = f.nodesById.fork();
		const node = nodes.get(1)!;
		nodes.set(1, {
			...node,
			decl_id: 'values/renamed',
			meta: { ...node.meta, label: 'New label' }
		});
		f.index.update({ nodesById: nodes, parentById: f.parentById }, 1);
		expect(f.index.child(f.owner, 'Value 1')).toBeNull();
		expect(f.index.child(f.owner, 'cell1')).toBeNull();
		expect(f.index.child(f.owner, 'renamed')?.node_id).toBe(1);
		expect(f.index.child(f.owner, 'New label')?.node_id).toBe(1);
	});
	it('retires cached membership and source identities on deletion and replacement', () => {
		const f = fixture();
		f.index.child(f.owner, 'cell1');
		const nodes = f.nodesById.fork();
		nodes.delete(1);
		f.index.update({ nodesById: nodes, parentById: f.parentById }, 1);
		expect(f.index.child(f.owner, 'cell1')).toBeNull();
		const replacement = { ...f.owner, uuid: 'replacement' };
		nodes.set(0, replacement);
		f.index.update({ nodesById: nodes, parentById: f.parentById }, 2);
		expect(f.index.child(f.owner, 'cell2')).toBeNull();
		expect(f.index.child(replacement, 'cell2')?.node_id).toBe(2);
	});
});
