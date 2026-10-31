import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ScreenAuthoringController, createScreenAuthoringState } from '../authoring.ts';
import type { ScreenEditResult } from '../generated/ScreenEditResult';
import type { ScreenSnapshot } from '../generated/ScreenSnapshot';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
const session = () => ({ status: 'connected', snapshotGeneration: 1, loading: { visible: false } });
const screen = (id: string): ScreenSnapshot => ({
	id,
	label: id,
	definition: { target: null, enabled: false },
	surfaces: []
});
const catalog = (...screens: ScreenSnapshot[]): ScreenEditResult => ({
	kind: 'Manager',
	snapshot: { screens }
});
const media = async () => ({ media: [], assets: [] });

test('a live gesture defers catalog inspection until it finishes', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	let inspections = 0;
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async () => {
			inspections++;
			return catalog(screen('current'));
		},
		media
	});
	await controller.refresh();
	controller.setInteracting(true);
	for (let i = 0; i < 100; i++) await controller.refresh();
	assert.equal(inspections, 1);
	controller.setInteracting(false);
	await controller.refresh();
	assert.equal(inspections, 2);
});

test('a late catalog cannot replace a newer inspection or selection', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const first = deferred<ScreenEditResult>();
	const second = deferred<ScreenEditResult>();
	const pending = [first, second];
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: () => pending.shift()!.promise,
		media
	});
	const a = controller.refresh();
	const b = controller.refresh();
	second.resolve(catalog(screen('new')));
	await b;
	first.resolve(catalog(screen('old')));
	await a;
	assert.equal(state.selectedScreen, 'new');
	assert.equal(state.catalog?.screens[0].id, 'new');
	assert.equal(controller.busy, false);
});

test('replacement, disconnect and disposal fence old responses and errors', async () => {
	for (const mode of ['replacement', 'disconnect', 'dispose']) {
		const current = session();
		const state = createScreenAuthoringState();
		const read = deferred<ScreenEditResult>();
		const controller = new ScreenAuthoringController(() => current, state, {
			screens: () => read.promise,
			media
		});
		const promise = controller.refresh();
		if (mode === 'replacement') current.snapshotGeneration++;
		else if (mode === 'disconnect') current.status = 'disconnected';
		else controller.dispose();
		read.reject(new Error('old failure'));
		await promise;
		assert.equal(state.catalog, null);
		assert.equal(state.error, '');
		assert.equal(state.refreshing, false);
	}
});

test('an edit reply from a replaced project cannot select identities in the new project', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const edit = deferred<ScreenEditResult>();
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) =>
			request.kind === 'InspectManager'
				? catalog(screen(`project-${current.snapshotGeneration}`))
				: edit.promise,
		media
	});
	await controller.refresh();
	const operation = controller.edit({ kind: 'CreateScreen', label: null });
	current.snapshotGeneration++;
	await controller.refresh();
	edit.resolve({
		kind: 'ScreenCreated',
		screen: 'old-created',
		surface: { screen: 'old-created', surface: 'old-surface' }
	});
	await operation;
	assert.equal(state.selectedScreen, 'project-2');
	assert.equal(state.pending, false);
});

test('committed creation selects backend identities and concurrent edits are not duplicated', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const reply = deferred<ScreenEditResult>();
	let edits = 0;
	let created = false;
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) => {
			if (request.kind === 'InspectManager')
				return created
					? catalog(screen('existing'), screen('created'))
					: catalog(screen('existing'));
			edits++;
			return reply.promise;
		},
		media
	});
	await controller.refresh();
	const first = controller.edit({ kind: 'CreateScreen', label: null });
	await controller.edit({ kind: 'CreateScreen', label: null });
	created = true;
	reply.resolve({
		kind: 'ScreenCreated',
		screen: 'created',
		surface: { screen: 'created', surface: 'surface' }
	});
	await first;
	assert.equal(edits, 1);
	assert.equal(state.selectedScreen, 'created');
	assert.equal(state.pending, false);
});

test('rejection preserves committed catalog and surfaces its error', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) => {
			if (request.kind === 'InspectManager') return catalog(screen('existing'));
			throw new Error('invalid quad');
		},
		media
	});
	await controller.refresh();
	const before = state.catalog;
	await controller.edit({ kind: 'RemoveScreen', screen: 'existing' });
	assert.equal(state.catalog, before);
	assert.match(state.error, /invalid quad/);
	assert.equal(state.pending, false);
});
