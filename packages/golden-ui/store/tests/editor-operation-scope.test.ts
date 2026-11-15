import { test } from 'vitest';
import assert from 'node:assert/strict';
import { createEditorOperationScopes } from '../editor-operation-scope';

function fixture() {
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		projectGeneration: 1 as number | null,
		selectionRevision: 1,
		loading: { visible: false }
	};
	let current: typeof host | null = host,
		visible = true,
		available = true;
	const scopes = createEditorOperationScopes(() => current);
	const capture = () =>
		scopes.capture(
			() => visible,
			() => available
		)!;
	return {
		host,
		scopes,
		capture,
		set hostCurrent(value: typeof current) {
			current = value;
		},
		set visible(value: boolean) {
			visible = value;
		},
		set available(value: boolean) {
			available = value;
		}
	};
}

test('view loss permanently refuses new submissions while the original document can settle', () => {
	const f = fixture(),
		lease = f.capture();
	assert(lease.current() && lease.documentCurrent() && lease.selectionCurrent());
	f.visible = false;
	f.scopes.observe();
	assert.equal(lease.current(), false);
	assert.equal(lease.documentCurrent(), true);
	assert.equal(lease.selectionCurrent(), false);
	f.visible = true;
	assert.equal(lease.current(), false, 'returning to an owner cannot revive a pending command');
	lease.release();
	assert(lease.signal.aborted);
});

for (const boundary of [
	'host',
	'project',
	'snapshot',
	'loading',
	'disconnect',
	'removed',
	'dispose'
] as const) {
	test(`${boundary} replacement aborts the old document lease`, () => {
		const f = fixture(),
			lease = f.capture();
		if (boundary === 'host') f.hostCurrent = { ...f.host };
		if (boundary === 'project') f.host.projectGeneration = 2;
		if (boundary === 'snapshot') f.host.snapshotGeneration++;
		if (boundary === 'loading') f.host.loading.visible = true;
		if (boundary === 'disconnect') f.host.status = 'disconnected';
		if (boundary === 'removed') f.available = false;
		if (boundary === 'dispose') f.scopes.dispose();
		f.scopes.observe();
		assert(lease.signal.aborted);
		assert.equal(lease.documentCurrent(), false);
		f.host.loading.visible = false;
		f.host.status = 'connected';
		f.available = true;
		assert.equal(lease.documentCurrent(), false);
		lease.release();
	});
}

test('foreign selection withdraws feedback without canceling admitted document work', () => {
	const f = fixture(),
		lease = f.capture();
	f.host.selectionRevision++;
	assert(lease.current() && lease.documentCurrent());
	assert.equal(lease.selectionCurrent(), false);
	f.scopes.dispose();
	assert.equal(
		f.scopes.capture(
			() => true,
			() => true
		),
		null
	);
});
