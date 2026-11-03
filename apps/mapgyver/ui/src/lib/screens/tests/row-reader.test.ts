import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createRowState, ScreenRowReader } from '../row-reader.ts';
import type { ScreenRowSnapshot } from '../generated/ScreenRowSnapshot';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const row = (node: string): ScreenRowSnapshot => ({
	node,
	detail: node,
	enabled: true,
	extent: null
});
test('row reads coalesce to one latest request and fence project replacement, errors and disposal', async () => {
	const state = createRowState();
	const pending: {
		node: string;
		resolve: (row: ScreenRowSnapshot) => void;
		reject: (error: Error) => void;
	}[] = [];
	const reader = new ScreenRowReader(
		state,
		async (_session, node) =>
			new Promise<ScreenRowSnapshot>((resolve, reject) => pending.push({ node, resolve, reject }))
	);
	const session = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	reader.refresh(session, 'old');
	for (let i = 0; i < 100; i++) reader.refresh(session, `latest-${i}`);
	assert.equal(pending.length, 1);
	pending[0].resolve(row('old'));
	await flush();
	assert.equal(state.snapshot, null);
	assert.equal(pending.length, 2);
	assert.equal(pending[1].node, 'latest-99');
	session.snapshotGeneration++;
	reader.refresh(session, 'new-project');
	pending[1].reject(new Error('obsolete'));
	await flush();
	assert.equal(state.error, '');
	pending[2].resolve(row('new-project'));
	await flush();
	assert.equal((state.snapshot as ScreenRowSnapshot | null)?.detail, 'new-project');
	reader.refresh(session, 'pending');
	reader.dispose();
	pending[3].resolve(row('pending'));
	await flush();
	assert.equal(state.snapshot, null);
	assert.equal(pending.length, 4);
});
test('disconnect clears authored rows and no reads start until the session is usable', async () => {
	const state = createRowState();
	let reads = 0;
	const reader = new ScreenRowReader(state, async (_session, node) => {
		reads++;
		return row(node);
	});
	const session = { status: 'connected', snapshotGeneration: 1, loading: { visible: false } };
	reader.refresh(session, 'current');
	await flush();
	assert.equal(state.snapshot?.detail, 'current');
	session.status = 'disconnected';
	reader.refresh(session, 'current');
	assert.equal(state.snapshot, null);
	assert.equal(reads, 1);
	session.status = 'connected';
	reader.refresh(session, 'current');
	await flush();
	assert.equal(reads, 2);
	reader.dispose();
});
