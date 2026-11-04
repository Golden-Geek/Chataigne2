import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ScreenAuthoringController, createScreenAuthoringState } from '../authoring.ts';
import type { ScreenEditResult } from '../generated/ScreenEditResult';
import type { ScreenSnapshot } from '../generated/ScreenSnapshot';
import type { SurfaceSnapshot } from '../generated/SurfaceSnapshot';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (error: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
const session = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 1,
	loading: { visible: false }
});
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
const surface = (width = 1): SurfaceSnapshot => ({
	mapping_valid: true,
	reference: { screen: 'current', surface: 'surface' },
	label: 'Surface',
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
		calibration: 'Off',
		enabled: true
	}
});
const ownedCatalog = (): ScreenEditResult =>
	catalog({ ...screen('current'), surfaces: [surface()] });

test('Screen selection and catalog refresh preserve explicit Surface deselection', async () => {
	const current = session(),
		state = createScreenAuthoringState();
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async () => ownedCatalog(),
		media
	});
	await controller.refresh();
	assert.equal(state.selectedScreen, 'current');
	assert.equal(state.selectedSurface, null);
	controller.selectSurface(surface().reference);
	await controller.refresh();
	assert.equal(state.selectedSurface, 'surface');
	controller.selectScreen('current');
	assert.equal(state.selectedSurface, null);
	await controller.refresh();
	assert.equal(state.selectedSurface, null);
});

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

test('background catalog refresh leaves loaded controls usable and cannot overwrite an admitted gesture', async () => {
	const current = session(),
		state = createScreenAuthoringState();
	const pending = deferred<ScreenEditResult>();
	let initial = true;
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async () => {
			if (initial) {
				initial = false;
				return ownedCatalog();
			}
			return pending.promise;
		},
		media
	});
	await controller.refresh();
	const refresh = controller.refresh();
	assert.equal(state.refreshing, true);
	assert.equal(controller.busy, false);
	controller.setInteracting(true);
	controller.acceptSurface(surface(0.5));
	pending.resolve(ownedCatalog());
	await refresh;
	assert.equal(state.catalog?.screens[0].surfaces[0].definition.crop.width, 0.5);
	controller.setInteracting(false);
	controller.dispose();
});

test('history replacement fences geometry until its projection arrives', async () => {
	const current = session(),
		state = createScreenAuthoringState();
	let reply = Promise.resolve(ownedCatalog());
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: () => reply,
		media
	});
	await controller.refresh();
	assert.equal(controller.busy, false);
	const old = deferred<ScreenEditResult>();
	reply = old.promise;
	const stale = controller.refresh();
	current.currentHistoryStateId++;
	assert.equal(controller.busy, true, 'Undo must not admit input against the old geometry');
	old.resolve(ownedCatalog());
	await stale;
	assert.equal(controller.busy, true, 'a pre-Undo reply cannot unlock the stale origin');
	reply = Promise.resolve(ownedCatalog());
	await controller.refresh();
	assert.equal(controller.busy, false);
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

test('generic Inspector edits publish bounded Surface reads before their edit session ends', async () => {
	const current = { ...session(), hasActiveEditSession: false };
	const state = createScreenAuthoringState();
	const pending = [deferred<ScreenEditResult>(), deferred<ScreenEditResult>()];
	let ownerReads = 0,
		catalogs = 0,
		mediaReads = 0;
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) => {
			if (request.kind === 'InspectManager') {
				catalogs++;
				return ownedCatalog();
			}
			assert.equal(request.kind, 'InspectSurface');
			return pending[ownerReads++].promise;
		},
		media: async () => {
			mediaReads++;
			return media();
		}
	});
	await controller.refresh();
	controller.selectSurface(surface().reference);
	current.hasActiveEditSession = true;
	const request = { kind: 'InspectSurface', reference: surface().reference } as const;
	const task = controller.refreshOwner(request);
	for (let sample = 0; sample < 100; sample++) void controller.refreshOwner(request);
	assert.equal(ownerReads, 1, 'one owner read is in flight');
	pending[0].resolve({ kind: 'Surface', snapshot: surface(0.8) });
	await new Promise((resolve) => setImmediate(resolve));
	assert.equal(current.hasActiveEditSession, true);
	assert.equal(
		controller.surface?.definition.crop.width,
		0.8,
		'accepted samples render while later samples are pending'
	);
	assert.equal(ownerReads, 2, 'intermediate samples coalesce into one newest read');
	pending[1].resolve({ kind: 'Surface', snapshot: surface(0.4) });
	await task;
	assert.equal(controller.surface?.definition.crop.width, 0.4);
	assert.equal(catalogs, 1);
	assert.equal(mediaReads, 1, 'live crop samples do not inspect the whole document/media catalog');
	assert.equal(controller.busy, false);
});

test('a live Screen definition updates output geometry without replacing its Surfaces', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) =>
			request.kind === 'InspectManager'
				? ownedCatalog()
				: {
						kind: 'ScreenDefinition',
						screen: 'current',
						definition: {
							enabled: true,
							target: { kind: 'Spout', sender: 'Output', extent: { width: 480, height: 640 } }
						}
					},
		media
	});
	await controller.refresh();
	controller.selectSurface(surface().reference);
	const surfaces = controller.screen?.surfaces;
	await controller.refreshOwner({ kind: 'InspectScreen', screen: 'current' });
	assert.equal(controller.screen?.definition.target?.kind, 'Spout');
	assert.equal(controller.screen?.surfaces, surfaces);
	assert.equal(state.selectedSurface, 'surface');
});

test('canvas ownership fences in-flight Inspector reads and defers further reads until it ends', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const read = deferred<ScreenEditResult>();
	let ownerReads = 0;
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) => {
			if (request.kind === 'InspectManager') return ownedCatalog();
			ownerReads++;
			return read.promise;
		},
		media
	});
	await controller.refresh();
	controller.selectSurface(surface().reference);
	const request = { kind: 'InspectSurface', reference: surface().reference } as const;
	const pending = controller.refreshOwner(request);
	controller.setInteracting(true);
	controller.acceptSurface(surface(0.7));
	for (let sample = 0; sample < 100; sample++) await controller.refreshOwner(request);
	read.resolve({ kind: 'Surface', snapshot: surface(0.2) });
	await pending;
	assert.equal(ownerReads, 1);
	assert.equal(
		controller.surface?.definition.crop.width,
		0.7,
		'late Inspector reads cannot overwrite accepted canvas geometry'
	);
});

test('project replacement and full refresh fence stale live owner responses', async () => {
	const current = session();
	const state = createScreenAuthoringState();
	const read = deferred<ScreenEditResult>();
	const controller = new ScreenAuthoringController(() => current, state, {
		screens: async (_, request) =>
			request.kind === 'InspectManager' ? ownedCatalog() : read.promise,
		media
	});
	await controller.refresh();
	controller.selectSurface(surface().reference);
	const pending = controller.refreshOwner({
		kind: 'InspectSurface',
		reference: surface().reference
	});
	current.snapshotGeneration++;
	await controller.refresh();
	read.resolve({ kind: 'Surface', snapshot: surface(0.2) });
	await pending;
	assert.equal(state.selectedSurface, null);
	assert.equal(controller.screen?.surfaces[0].definition.crop.width, 1);
	assert.equal(state.error, '');
});
