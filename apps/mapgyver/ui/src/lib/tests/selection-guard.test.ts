import assert from 'node:assert/strict';
import test from 'node:test';
import { captureSelectionGuard } from '../selection-guard.ts';

const session = () => ({
	snapshotGeneration: 1,
	selectionRevision: 0,
	status: 'connected',
	loading: { visible: false }
});

test('late creation selection expires on reselecting the same node before reply or graph delta', async () => {
	const current = session();
	const beforeReply = captureSelectionGuard(current);
	await Promise.resolve();
	// Golden increments this revision even when the selected node identity stays the same.
	current.selectionRevision++;
	assert.equal(beforeReply(current), false);
	const beforeDelta = captureSelectionGuard(current);
	assert.equal(beforeDelta(current), true);
	current.selectionRevision++;
	assert.equal(beforeDelta(current), false);
});

test('creation selection accepts its unchanged owner but expires across project/session/loading/disconnect', () => {
	const current = session();
	const accepts = captureSelectionGuard(current);
	assert(accepts(current));
	assert(!accepts(session()));
	current.snapshotGeneration++;
	assert(!accepts(current));
	current.snapshotGeneration--;
	current.status = 'disconnected';
	assert(!accepts(current));
	current.status = 'connected';
	current.loading.visible = true;
	assert(!accepts(current));
	assert(!captureSelectionGuard(null)(current));
});
