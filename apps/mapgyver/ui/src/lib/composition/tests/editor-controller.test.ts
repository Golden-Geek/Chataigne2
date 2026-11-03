import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CompositionEditorController, createCompositionEditorState } from '../editor-controller.ts';
import type { CompositionEditorQuery } from '../../media/generated/CompositionEditorQuery';
import type { CompositionEditorPage } from '../../media/generated/CompositionEditorPage';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import { compositionPanelState } from '../panel-state.ts';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const session = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 1,
	loading: { visible: false },
	hasActiveEditSession: false
});
const query = (selected = 'owner'): CompositionEditorQuery => ({
	selected,
	document: null,
	locked: null,
	instance: null,
	start: 0,
	count: 128
});
const reference = (id = 'owner') => ({ media: id, channel: `${id}-output` });
const instance = (id: string) => ({
	reference: { composition: reference(), instance: id },
	label: id,
	definition: {
		source: reference('source'),
		opacity: 0.5,
		blend: 'Multiply' as const,
		enabled: true,
		transform: {
			position: [2, 3] as [number, number],
			scale: [-2, 3] as [number, number],
			pivot: [0.3, 0.7] as [number, number],
			rotation_degrees: 27
		}
	}
});
const page = (id = 'owner'): CompositionEditorPage => ({
	document: 'project',
	document_matches: true,
	start: 0,
	total: 1000,
	channel: {
		reference: reference(id),
		label: id,
		definition: {
			source: { kind: 'Composition', canvas: { width: 400, height: 300 } },
			size: { kind: 'Automatic' },
			filter: 'Linear'
		}
	},
	composition: {
		reference: reference(id),
		canvas: { width: 400, height: 300 },
		instances: [instance('visible')]
	},
	selected: instance('off-page')
});
const reply = (id = 'owner'): MediaEditResult => ({ kind: 'CompositionEditor', page: page(id) });
function deferred() {
	let resolve!: (value: MediaEditResult) => void, reject!: (error: Error) => void;
	const promise = new Promise<MediaEditResult>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}

test('each panel owns one bounded active read and one latest replacement, without catalog reads', async () => {
	const host = session(),
		pending = deferred(),
		state = createCompositionEditorState();
	const requests: MediaEdit[] = [];
	const editor = new CompositionEditorController(
		() => host,
		state,
		async (_, edit) => {
			requests.push(edit);
			return requests.length === 1 ? pending.promise : reply('latest');
		}
	);
	editor.refresh(query('old'));
	for (let n = 0; n < 100; n++) editor.refresh(query(`new-${n}`));
	assert.equal(requests.length, 1);
	pending.resolve(reply('old'));
	await flush();
	assert.equal(requests.length, 2);
	assert.deepEqual(requests[1], { kind: 'InspectCompositionEditor', query: query('new-99') });
	assert.equal(editor.channel?.label, 'latest');
	const independent = createCompositionEditorState();
	const other = new CompositionEditorController(
		() => host,
		independent,
		async () => reply('independent')
	);
	other.refresh(query());
	await flush();
	assert.equal(other.channel?.label, 'independent');
	assert.equal(editor.channel?.label, 'latest');
});

test('selected definitions stay editable off page and live geometry does not trigger reads', async () => {
	const host = session(),
		state = createCompositionEditorState();
	let reads = 0;
	const editor = new CompositionEditorController(
		() => host,
		state,
		async () => {
			reads++;
			return reply();
		}
	);
	editor.refresh(query());
	await flush();
	assert.equal(editor.composition?.instances.length, 2);
	assert.equal(editor.instance?.reference.instance, 'off-page');
	const peer = structuredClone(state.page!.composition!.instances[0]);
	editor.setInteracting(true);
	for (let n = 0; n < 100; n++) {
		const update = instance('off-page');
		update.definition.transform.position = [n, n + 1];
		editor.acceptInstance(update);
		editor.refresh(query());
	}
	assert.equal(reads, 1);
	assert.deepEqual(editor.instance?.definition.transform.position, [99, 100]);
	assert.deepEqual(state.page?.composition?.instances[0], peer);
	editor.setInteracting(false);
	editor.refresh();
	await flush();
	assert.equal(reads, 2);
});

test('loaded controls stay usable during a stalled background read and a mutation invalidates that reply', async () => {
	const host = session(),
		state = createCompositionEditorState(),
		stalled = deferred();
	let reads = 0,
		mutations = 0;
	const editor = new CompositionEditorController(
		() => host,
		state,
		async (_, edit) => {
			if (edit.kind === 'InspectCompositionEditor')
				return ++reads === 2 ? stalled.promise : reply();
			mutations++;
			return { kind: 'Changed' };
		}
	);
	editor.refresh(query());
	await flush();
	editor.refresh(query());
	assert.equal(state.refreshing, true);
	assert.equal(editor.busy, false);
	await editor.edit({ kind: 'RemoveInstance', instance: 'off-page' });
	assert.equal(mutations, 1);
	stalled.resolve(reply('obsolete'));
	await flush();
	assert.equal(editor.channel?.label, 'owner');
	assert.equal(reads, 3);
});

test('history receipts disable pointer actions until the bounded geometry reaches that exact state', async () => {
	const host = session(),
		state = createCompositionEditorState(),
		pending = deferred();
	let reads = 0;
	const editor = new CompositionEditorController(
		() => host,
		state,
		async () => (++reads === 1 ? reply() : pending.promise)
	);
	editor.refresh(query());
	await flush();
	assert.equal(editor.busy, false);
	host.currentHistoryStateId++;
	assert.equal(
		editor.busy,
		true,
		'history change fences the old origin before effects dispatch a read'
	);
	editor.refresh(query());
	assert.equal(editor.busy, true);
	assert.equal(editor.channel?.label, 'owner', 'keep the native registration while reading');
	pending.resolve(reply());
	await flush();
	assert.equal(editor.busy, false);
});

test('project replacement, disconnect, loading, history and disposal fence late reads and errors', async () => {
	for (const mode of ['project', 'disconnect', 'loading', 'history', 'dispose']) {
		const host = session(),
			state = createCompositionEditorState(),
			pending = deferred();
		const editor = new CompositionEditorController(
			() => host,
			state,
			() => pending.promise
		);
		editor.refresh(query());
		if (mode === 'project') host.snapshotGeneration++;
		if (mode === 'disconnect') host.status = 'disconnected';
		if (mode === 'loading') host.loading.visible = true;
		if (mode === 'history') host.currentHistoryStateId++;
		if (mode === 'dispose') editor.dispose();
		pending.reject(new Error('old error'));
		await flush();
		assert.equal(state.page, null);
		assert.equal(state.readError, '');
	}
});

test('only one exact backend mutation is admitted and a project replacement fences its identity', async () => {
	const host = session(),
		state = createCompositionEditorState(),
		pending = deferred();
	let mutations = 0;
	const editor = new CompositionEditorController(
		() => host,
		state,
		async (_, edit) => {
			if (edit.kind === 'InspectCompositionEditor')
				return reply(`project-${host.snapshotGeneration}`);
			mutations++;
			assert.equal(edit.kind, 'Composition');
			return pending.promise;
		}
	);
	editor.refresh(query());
	await flush();
	const operation = editor.edit({ kind: 'DuplicateInstances', instances: ['off-page'] });
	await editor.edit({ kind: 'DuplicateInstances', instances: ['off-page'] });
	assert.equal(mutations, 1);
	host.snapshotGeneration++;
	editor.refresh(query());
	await flush();
	pending.resolve({
		kind: 'InstancesCreated',
		instances: [{ composition: reference(), instance: 'old-created' }]
	});
	assert.equal(await operation, undefined);
	assert.equal(editor.channel?.label, 'project-2');
	assert.equal(state.pending, false);
});

test('mutation rejections remain visible across immutable refresh and selected deletion follows the backend', async () => {
	const host = session(),
		state = createCompositionEditorState();
	let removed = false;
	const editor = new CompositionEditorController(
		() => host,
		state,
		async (_, edit) => {
			if (edit.kind !== 'InspectCompositionEditor') throw new Error('dependency cycle');
			const result = page();
			if (removed) result.selected = instance('visible');
			return { kind: 'CompositionEditor', page: result };
		}
	);
	editor.refresh(query());
	await flush();
	await editor.edit({ kind: 'AddInstances', sources: [reference()] });
	await flush();
	assert.match(state.error, /dependency cycle/);
	assert.equal(editor.instance?.reference.instance, 'off-page');
	removed = true;
	editor.refresh(query());
	await flush();
	assert.equal(editor.instance?.reference.instance, 'visible');
});

test('persisted panel choices copy valid document-scoped locks and reject malformed state', () => {
	const id = '11111111-2222-4333-8444-555555555555';
	const input = { locked: { media: id, channel: id }, document: id, page: 2 };
	const first = compositionPanelState(input),
		second = compositionPanelState(input);
	first.locked!.media = 'changed';
	assert.equal(second.locked?.media, id);
	assert.deepEqual(compositionPanelState({ locked: input.locked, document: null, page: -1 }), {
		locked: null,
		document: null,
		page: 0
	});
	assert.equal(compositionPanelState({ page: Number.POSITIVE_INFINITY }).page, 0);
});
