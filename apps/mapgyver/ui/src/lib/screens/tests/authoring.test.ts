import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ScreenEditorController, screenEditorState } from '../authoring.ts';
import { screenPanelState } from '../panel-state.ts';
import type { ScreenEditorQuery } from '../generated/ScreenEditorQuery';
import type { ScreenEditResult } from '../generated/ScreenEditResult';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';

const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
const session = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	projectGeneration: 1 as number | null,
	currentHistoryStateId: 1,
	loading: { visible: false },
	hasActiveEditSession: false,
	editSessionEpoch: 0
});
const query = (selected = 'current'): ScreenEditorQuery => ({
	editor: 'Screen',
	selected,
	document: null,
	locked: null,
	retained: null,
	surface: null
});
const surface = (width = 1): SurfaceSnapshot => ({
	reference: { screen: 'current', surface: 'surface' },
	label: 'Surface',
	mapping_valid: true,
	definition: {
		source: null,
		crop: { x: 0, y: 0, width, height: 1 },
		quad: {
			corners: [
				{ x: 0, y: 0 },
				{ x: 1, y: 0 },
				{ x: 1, y: 1 },
				{ x: 0, y: 1 }
			]
		},
		opacity: 1,
		enabled: true,
		calibration: 'Off'
	}
});
const reply = (id = 'current', selected: SurfaceSnapshot | null = surface()): ScreenEditResult => ({
	kind: 'Editor',
	page: {
		document: 'document',
		document_matches: true,
		target: { kind: 'Screen', screen: id },
		screen: {
			id,
			label: id,
			definition: { target: null, enabled: false, anti_aliasing: 'Raw' },
			surfaces: selected ? [selected] : []
		},
		has_more_surfaces: false,
		selected_surface: selected?.reference.surface ?? null,
		surface: selected
	}
});
const deferred = <T>() => {
	let resolve!: (value: T) => void, reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};

test('backend selection and explicit Surface deselection survive refresh without a UI default', async () => {
	const state = screenEditorState(),
		host = session();
	const controller = new ScreenEditorController(
		() => host,
		state,
		async (_, edit) => {
			assert.equal(edit.kind, 'InspectEditor');
			return reply('current', null);
		}
	);
	controller.refresh(query());
	await flush();
	assert.equal(controller.screen?.id, 'current');
	assert.equal(controller.surface, null);
	controller.refresh();
	await flush();
	assert.equal(controller.surface, null);
});
test('own gestures defer inspections until completion', async () => {
	let calls = 0;
	const host = session();
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async () => {
			calls++;
			return reply();
		}
	);
	controller.setInteracting(true);
	controller.refresh(query());
	await flush();
	assert.equal(calls, 0);
	controller.setInteracting(false);
	controller.refresh();
	await flush();
	assert.equal(calls, 1);
});
test('stalled background reads keep loaded controls usable and cannot overwrite an admitted gesture', async () => {
	const state = screenEditorState(),
		host = session(),
		read = deferred<ScreenEditResult>();
	let calls = 0;
	const controller = new ScreenEditorController(
		() => host,
		state,
		async () => (++calls === 1 ? reply() : read.promise)
	);
	controller.refresh(query());
	await flush();
	controller.refresh();
	assert.equal(controller.busy, false);
	controller.setInteracting(true);
	controller.acceptSurface(surface(0.5));
	read.resolve(reply());
	await flush();
	assert.equal(controller.surface?.definition.crop.width, 0.5);
});
test('history replacement fences geometry until that exact projection arrives', async () => {
	const host = session(),
		read = deferred<ScreenEditResult>();
	let calls = 0;
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async () => (++calls === 1 ? reply() : read.promise)
	);
	controller.refresh(query());
	await flush();
	host.currentHistoryStateId++;
	assert.equal(controller.busy, true);
	controller.refresh();
	read.resolve(reply());
	await flush();
	assert.equal(controller.busy, false);
});
test('one active and one latest replacement fence old selection replies without catalog reads', async () => {
	const host = session(),
		first = deferred<ScreenEditResult>();
	const requests: string[] = [];
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async (_, edit) => {
			assert.equal(edit.kind, 'InspectEditor');
			if (edit.kind !== 'InspectEditor') throw Error();
			requests.push(edit.query.selected!);
			return requests.length === 1 ? first.promise : reply(edit.query.selected!);
		}
	);
	controller.refresh(query('a'));
	controller.refresh(query('b'));
	controller.refresh(query('c'));
	assert.deepEqual(requests, ['a']);
	first.resolve(reply('a'));
	await flush();
	assert.deepEqual(requests, ['a', 'c']);
	assert.equal(controller.screen?.id, 'c');
});
test('project, snapshot, disconnect, session and disposal changes fence late responses and errors', async () => {
	for (const change of ['project', 'snapshot', 'disconnect', 'session', 'dispose']) {
		let host = session();
		const read = deferred<ScreenEditResult>(),
			state = screenEditorState();
		const controller = new ScreenEditorController(
			() => host,
			state,
			async () => read.promise
		);
		controller.refresh(query());
		if (change === 'project') host.projectGeneration = 2;
		if (change === 'snapshot') host.snapshotGeneration++;
		if (change === 'disconnect') host.status = 'disconnected';
		if (change === 'session') host = session();
		if (change === 'dispose') controller.dispose();
		read.reject(Error('expired'));
		await flush();
		assert.equal(state.readError, '');
		assert.equal(controller.screen, null);
	}
});
test('same-project reconnect retains presentation while unknown/new projects withdraw it immediately', async () => {
	const host = session();
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async () => reply()
	);
	controller.refresh(query());
	await flush();
	host.status = 'disconnected';
	controller.refresh();
	assert.equal(controller.screen?.id, 'current');
	host.status = 'connected';
	host.snapshotGeneration++;
	controller.refresh();
	assert.equal(controller.screen?.id, 'current');
	await flush();
	host.projectGeneration = null;
	assert.equal(controller.screen, null);
	host.projectGeneration = 2;
	assert.equal(controller.screen, null);
});
test('a replaced-project edit cannot return an identity for adoption in the new project', async () => {
	const host = session(),
		edit = deferred<ScreenEditResult>();
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async (_, request) => (request.kind === 'InspectEditor' ? reply() : edit.promise)
	);
	controller.refresh(query());
	await flush();
	const pending = controller.edit({ kind: 'CreateScreen', label: null });
	host.projectGeneration = 2;
	controller.refresh();
	edit.resolve({ kind: 'Changed' });
	assert.equal(await pending, undefined);
});
test('creation returns the committed backend identity and duplicate edits are not admitted', async () => {
	const host = session(),
		pending = deferred<ScreenEditResult>();
	let edits = 0;
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async (_, request) => {
			if (request.kind === 'InspectEditor') return reply();
			edits++;
			return pending.promise;
		}
	);
	controller.refresh(query());
	await flush();
	const first = controller.edit({ kind: 'CreateScreen', label: null });
	assert.equal(await controller.edit({ kind: 'CreateScreen', label: null }), undefined);
	const value: ScreenEditResult = {
		kind: 'ScreenCreated',
		screen: 'backend',
		surface: { screen: 'backend', surface: 'child' }
	};
	pending.resolve(value);
	assert.deepEqual(await first, value);
	assert.equal(edits, 1);
	assert.equal(
		controller.screen?.id,
		'current',
		'caller selection, not a UI default, establishes the next target'
	);
});
test('rejection preserves committed geometry and its actionable error across background reads', async () => {
	const state = screenEditorState(),
		host = session();
	const controller = new ScreenEditorController(
		() => host,
		state,
		async (_, request) => {
			if (request.kind === 'InspectEditor') return reply();
			throw Error('rejected');
		}
	);
	controller.refresh(query());
	await flush();
	await controller.edit({ kind: 'RemoveScreen', screen: 'current' });
	await flush();
	assert.match(state.error, /rejected/);
	assert.equal(controller.screen?.id, 'current');
});
test('generic Inspector held edits publish bounded live geometry before their session ends', async () => {
	const host = session();
	let width = 1;
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async () => reply('current', surface(width))
	);
	controller.refresh(query());
	await flush();
	host.hasActiveEditSession = true;
	width = 0.5;
	controller.refresh();
	await flush();
	assert.equal(controller.surface?.definition.crop.width, 0.5);
	assert.equal(host.hasActiveEditSession, true);
});
test('live Screen definitions update output geometry while preserving the Surface identity', async () => {
	const host = session();
	let enabled = false;
	const controller = new ScreenEditorController(
		() => host,
		screenEditorState(),
		async () => {
			const result = reply();
			if (result.kind === 'Editor') result.page.screen!.definition.enabled = enabled;
			return result;
		}
	);
	controller.refresh(query());
	await flush();
	enabled = true;
	controller.refresh();
	await flush();
	assert.equal(controller.screen?.definition.enabled, true);
	assert.equal(controller.surface?.reference.surface, 'surface');
});

test('continuous Inspector refreshes present in-flight geometry without waiting for input to stop', async () => {
	const host = session(),
		state = screenEditorState();
	const first = deferred<ScreenEditResult>(),
		second = deferred<ScreenEditResult>();
	let reads = 0;
	const controller = new ScreenEditorController(
		() => host,
		state,
		async () => {
			reads++;
			return reads === 1 ? reply() : reads === 2 ? first.promise : second.promise;
		}
	);
	controller.refresh(query());
	await flush();
	host.hasActiveEditSession = true;
	host.editSessionEpoch++;
	controller.refresh();
	host.currentHistoryStateId++;
	controller.refresh();
	host.currentHistoryStateId++;
	controller.refresh();
	first.resolve(reply('current', surface(0.8)));
	await flush();
	assert.equal(
		controller.surface?.definition.crop.width,
		0.8,
		'a valid in-progress snapshot must become visible while a newer read is pending'
	);
	second.resolve(reply('current', surface(0.6)));
	await flush();
	assert.equal(controller.surface?.definition.crop.width, 0.6);
	assert.equal(reads, 3, 'one in-flight read and one latest replacement remain bounded');
});
test('off-page selection is projected independently and missing targets never fall back to another Screen', async () => {
	const state = screenEditorState(),
		host = session();
	let missing = false;
	const controller = new ScreenEditorController(
		() => host,
		state,
		async () => {
			const result = reply();
			if (result.kind === 'Editor') {
				result.page.screen!.surfaces = [];
				if (missing) {
					result.page.screen = null;
					result.page.surface = null;
				}
			}
			return result;
		}
	);
	controller.refresh(query());
	await flush();
	assert.equal(controller.screen?.surfaces.length, 1);
	missing = true;
	controller.refresh();
	await flush();
	assert.equal(controller.screen, null);
});

test('ending an edit-session epoch fences its pending live snapshot before committed-history refresh', async () => {
	const host = session(),
		state = screenEditorState();
	const pending = deferred<ScreenEditResult>();
	let reads = 0;
	const controller = new ScreenEditorController(
		() => host,
		state,
		async () => (++reads === 2 ? pending.promise : reply())
	);
	controller.refresh(query());
	await flush();
	host.hasActiveEditSession = true;
	host.editSessionEpoch++;
	controller.refresh();
	host.currentHistoryStateId++;
	host.hasActiveEditSession = false;
	host.editSessionEpoch++;
	controller.refresh();
	pending.resolve(reply('current', surface(0.8)));
	await flush();
	assert.equal(
		controller.surface?.definition.crop.width,
		1,
		'the old group reply must not overwrite the final committed projection'
	);
	assert.equal(state.presentedHistory, host.currentHistoryStateId);
	assert.equal(controller.busy, false);
});
test('persisted targets are independent, document-scoped identities without authored definitions', () => {
	const id = '11111111-2222-4333-8444-555555555555';
	const input = { document: id, target: { kind: 'Screen' as const, screen: id }, locked: true };
	const a = screenPanelState(input),
		b = screenPanelState(input);
	if (a.target?.kind === 'Screen') a.target.screen = 'changed';
	assert.deepEqual(b.target, input.target);
	assert.equal(b.locked, true);
	assert.equal(screenPanelState({ target: input.target, locked: true }).target, null);
	assert.equal(screenPanelState(input, 'Surface').target, null);
});
