import { describe, expect, it } from 'vitest';
import { createPendingNodeSelection, type NodeSelectionIdentity } from '../pending-selection';

describe('authoritative selection with delayed projection', () => {
	it('waits for every exact identity and ignores independent creates', () => {
		const request = createPendingNodeSelection();
		const projected = new Map<number, NodeSelectionIdentity>();
		request.request(
			[
				{ node_id: 90, uuid: 'a' },
				{ node_id: 91, uuid: 'b' }
			],
			'project'
		);
		projected.set(89, { node_id: 89, uuid: 'unrelated' });
		expect(request.takeIfReady(projected, 'project')).toBeUndefined();
		projected.set(91, { node_id: 91, uuid: 'b' });
		expect(request.takeIfReady(projected, 'project')).toBeUndefined();
		projected.set(90, { node_id: 90, uuid: 'a' });
		expect(request.takeIfReady(projected, 'project')).toEqual([90, 91]);
		expect(request.takeIfReady(projected, 'project')).toBeUndefined();
	});
	it('does not override a later user selection', () => {
		const request = createPendingNodeSelection();
		request.request([{ node_id: 90, uuid: 'a' }], 'project');
		request.cancel();
		expect(
			request.takeIfReady(new Map([[90, { node_id: 90, uuid: 'a' }]]), 'project')
		).toBeUndefined();
	});
	it.each(['project', 'replacement'])(
		'rejects reused live IDs or a changed project root (%s)',
		(root) => {
			const request = createPendingNodeSelection();
			request.request([{ node_id: 90, uuid: 'old' }], 'project');
			expect(
				request.takeIfReady(new Map([[90, { node_id: 90, uuid: 'new' }]]), root)
			).toBeUndefined();
			expect(
				request.takeIfReady(new Map([[90, { node_id: 90, uuid: 'old' }]]), 'project')
			).toBeUndefined();
		}
	);
	it('retains only the latest request and snapshots its identities', () => {
		const request = createPendingNodeSelection();
		request.request([{ node_id: 1, uuid: 'first' }], 'project');
		const identity = { node_id: 2, uuid: 'second' };
		request.request([identity], 'project');
		identity.uuid = 'changed outside';
		expect(request.takeIfReady(new Map([[2, { node_id: 2, uuid: 'second' }]]), 'project')).toEqual([
			2
		]);
	});
});
