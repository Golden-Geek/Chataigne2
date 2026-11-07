import { afterEach, describe, expect, it, vi } from 'vitest';
import type { UiAck, UiCreatableUserItem, UiEditIntent } from '../../types';
const state = vi.hoisted(() => ({ session: null as any }));
vi.mock('../workbench.svelte', () => ({ appState: state }));
import { createUserItemAndSelect, sendCreateUserItemIntent } from '../ui-intents';
import {
	createPendingNodeSelection,
	type NodeSelectionIdentity
} from '../session/pending-selection';

const item = {
	node_type: 'factory.alias',
	label: 'Created',
	select_when_created: true
} as UiCreatableUserItem;
function fixture() {
	const projected = new Map<number, NodeSelectionIdentity>([
		[42, { node_id: 42, uuid: 'created' }]
	]);
	const selection = createPendingNodeSelection();
	const observe = () => {
		const ready = selection.takeIfReady(projected, 'project');
		if (ready) host.selectNode(ready[0], 'REPLACE');
	};
	let finish!: () => void;
	const pending = new Promise<void>((resolve) => {
		finish = resolve;
	});
	const receipt: UiAck = {
		success: true,
		status: 'applied',
		created_node: { node_id: 42, uuid: 'created' }
	};
	const host = {
		snapshotGeneration: 1,
		selectionRevision: 1,
		selectNode: vi.fn(),
		requestNodeSelection: vi.fn((nodes: NodeSelectionIdentity[]) => {
			selection.request(nodes, 'project');
			observe();
		}),
		sendIntent: vi.fn(async (_intent: UiEditIntent, applied: (acks: UiAck[]) => void) => {
			await pending;
			if (!receipt.success) throw new Error('Rejected creation');
			applied([receipt]);
		})
	};
	state.session = host;
	return { host, receipt, finish, projected, observe };
}
afterEach(() => {
	state.session = null;
	vi.restoreAllMocks();
});
describe('authoritative manager creation feedback', () => {
	it('uses the exact committed receipt independently of the catalog alias', async () => {
		const f = fixture();
		const pending = createUserItemAndSelect(1, item);
		expect(f.host.selectNode).not.toHaveBeenCalled();
		f.finish();
		await pending;
		expect(f.host.selectNode).toHaveBeenCalledExactlyOnceWith(42, 'REPLACE');
	});
	it.each(['selection', 'generation', 'session', 'recycled', 'removed', 'rejected'])(
		'does not select after %s changes while awaiting the receipt',
		async (change) => {
			const f = fixture();
			vi.spyOn(console, 'error').mockImplementation(() => {});
			const pending = createUserItemAndSelect(1, item);
			if (change === 'selection') f.host.selectionRevision++;
			if (change === 'generation') f.host.snapshotGeneration++;
			if (change === 'session') state.session = {};
			if (change === 'recycled') f.projected.set(42, { node_id: 42, uuid: 'other' });
			if (change === 'removed') f.projected.delete(42);
			if (change === 'rejected') f.receipt.success = false;
			f.finish();
			await pending;
			expect(f.host.selectNode).not.toHaveBeenCalled();
		}
	);
	it('retains an acknowledged identity until its actual graph projection arrives', async () => {
		const f = fixture();
		f.projected.delete(42);
		const pending = createUserItemAndSelect(1, item);
		f.finish();
		await pending;
		expect(f.host.selectNode).not.toHaveBeenCalled();
		f.projected.set(41, { node_id: 41, uuid: 'unrelated' });
		f.observe();
		expect(f.host.selectNode).not.toHaveBeenCalled();
		f.projected.set(42, { node_id: 42, uuid: 'created' });
		f.observe();
		expect(f.host.selectNode).toHaveBeenCalledExactlyOnceWith(42, 'REPLACE');
	});
	it('manager menus select every created root even when a catalog opts out for editor callers', async () => {
		const f = fixture();
		const pending = createUserItemAndSelect(1, { ...item, select_when_created: false });
		f.finish();
		await pending;
		expect(f.host.selectNode).toHaveBeenCalledExactlyOnceWith(42, 'REPLACE');
	});
	it('receipt-only callers never acquire menu selection behavior', async () => {
		const f = fixture();
		const pending = sendCreateUserItemIntent(1, item);
		f.finish();
		expect(await pending).toMatchObject({
			success: true,
			createdNodeId: 42,
			createdNodeUuid: 'created'
		});
		expect(f.host.selectNode).not.toHaveBeenCalled();
	});
});
