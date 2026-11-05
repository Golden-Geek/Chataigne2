import { test } from 'node:test';
import assert from 'node:assert/strict';
import { SpatialBindingReader, bindingState } from '../binding.ts';
import { spatialPanelState, sameTarget, targetKey } from '../targets.ts';
import type { SpatialEditorQuery } from '../../media/generated/SpatialEditorQuery';
import type { SpatialEditorTarget } from '../../media/generated/SpatialEditorTarget';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const session = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	projectGeneration: 1 as number | null,
	currentHistoryStateId: 1,
	loading: { visible: false }
});
const target = (id = 'matrix'): SpatialEditorTarget => ({ kind: 'Matrix', media: id });
const query = (id = 'matrix'): SpatialEditorQuery => ({
	selected: id,
	document: null,
	locked: null,
	retained: null
});
const reply = (id = 'matrix', present = true): MediaEditResult => ({
	kind: 'SpatialEditor',
	binding: { target: target(id), label: id, present, document: 'document', document_matches: true }
});
function deferred() {
	let resolve!: (value: MediaEditResult) => void, reject!: (error: unknown) => void;
	const promise = new Promise<MediaEditResult>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { resolve, reject, promise };
}

test('two panel readers keep independent targets and forward explicit backend binding decisions', async () => {
	const host = session(),
		a = bindingState(),
		b = bindingState();
	const invoke = async (_: unknown, edit: import('../../media/generated/MediaEdit').MediaEdit) => {
		assert.equal(edit.kind, 'InspectSpatialEditor');
		if (edit.kind !== 'InspectSpatialEditor') throw Error();
		return reply(
			edit.query.locked?.kind === 'Matrix'
				? edit.query.locked.media
				: (edit.query.selected ?? 'matrix')
		);
	};
	const left = new SpatialBindingReader(() => host, a, invoke),
		right = new SpatialBindingReader(() => host, b, invoke);
	left.refresh({ ...query('other'), locked: target('a'), document: 'document' });
	right.refresh(query('b'));
	await flush();
	assert.equal(targetKey(left.binding?.target), 'matrix:a');
	assert.equal(targetKey(right.binding?.target), 'matrix:b');
});
test('one active and one latest read reject late target absence or errors', async () => {
	const host = session(),
		old = deferred();
	let calls = 0;
	const reader = new SpatialBindingReader(
		() => host,
		bindingState(),
		async () => (++calls === 1 ? old.promise : reply('latest'))
	);
	reader.refresh(query('old'));
	for (let i = 0; i < 100; i++) reader.refresh(query('new'));
	assert.equal(calls, 1);
	old.resolve(reply('old', false));
	await flush();
	assert.equal(calls, 2);
	assert.equal(targetKey(reader.binding?.target), 'matrix:latest');
});
test('same-project disconnect and snapshot recovery retain presentation; unknown/new projects withdraw it', async () => {
	const host = session(),
		pending = deferred();
	let calls = 0;
	const reader = new SpatialBindingReader(
		() => host,
		bindingState(),
		async () => (++calls === 1 ? reply() : pending.promise)
	);
	reader.refresh(query());
	await flush();
	host.status = 'disconnected';
	reader.refresh(query());
	assert(reader.binding?.present);
	host.status = 'connected';
	host.snapshotGeneration++;
	reader.refresh(query());
	assert(reader.binding?.present);
	pending.resolve(reply());
	await flush();
	host.projectGeneration = null;
	assert.equal(reader.binding, null);
	host.projectGeneration = 2;
	assert.equal(reader.binding, null);
});
test('history/project/session/snapshot/loading/disconnect/dispose fence stale replies and errors', async () => {
	for (const reason of [
		'history',
		'project',
		'session',
		'snapshot',
		'loading',
		'disconnect',
		'dispose'
	]) {
		let host = session();
		const pending = deferred(),
			state = bindingState(),
			reader = new SpatialBindingReader(
				() => host,
				state,
				async () => pending.promise
			);
		reader.refresh(query());
		if (reason === 'history') host.currentHistoryStateId++;
		if (reason === 'project') host.projectGeneration = 2;
		if (reason === 'session') host = session();
		if (reason === 'snapshot') host.snapshotGeneration++;
		if (reason === 'loading') host.loading.visible = true;
		if (reason === 'disconnect') host.status = 'disconnected';
		if (reason === 'dispose') reader.dispose();
		pending.reject(Error('obsolete'));
		await flush();
		assert.equal(reader.binding, null);
		assert.equal(state.error, '');
	}
});
test('authoritative missing targets remain explicit and recover on a fresh Undo projection', async () => {
	const host = session(),
		state = bindingState();
	let present = false;
	const reader = new SpatialBindingReader(
		() => host,
		state,
		async () => reply('same', present)
	);
	reader.refresh(query());
	await flush();
	assert.equal(reader.binding?.present, false);
	assert.equal(targetKey(reader.binding?.target), 'matrix:same');
	present = true;
	host.currentHistoryStateId++;
	reader.refresh(query());
	await flush();
	assert.equal(reader.binding?.present, true);
});
test('typed persisted targets copy independent values, require document scope and compare identities structurally', () => {
	const id = '11111111-2222-4333-8444-555555555555';
	for (const target of [
		{ kind: 'Matrix' as const, media: id },
		{ kind: 'Composition' as const, reference: { media: id, channel: id } }
	]) {
		const input = { document: id, locked: target, retained: target, page: 2 };
		const a = spatialPanelState(input),
			b = spatialPanelState(input);
		assert(sameTarget(a.locked, b.locked));
		assert.equal(spatialPanelState({ locked: target }).locked, null);
		if (a.locked?.kind === 'Matrix') a.locked.media = 'changed';
		else if (a.locked?.kind === 'Composition') a.locked.reference.media = 'changed';
		assert(!sameTarget(a.locked, b.locked));
		assert(sameTarget(b.locked, b.retained));
	}
	assert.equal(spatialPanelState({ page: Infinity }).page, 0);
	assert.equal(spatialPanelState({ page: -1 }).page, 0);
	const state = { document: id, matrix: { media: id, layer: id, cell: id, page: 2 } };
	const first = spatialPanelState(state),
		second = spatialPanelState(state);
	first.matrix!.cell = null;
	assert.equal(second.matrix?.cell, id);
	assert.equal(spatialPanelState({ matrix: state.matrix }).matrix, null);
});
