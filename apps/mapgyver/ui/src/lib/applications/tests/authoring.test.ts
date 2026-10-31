import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ApplicationController, applicationState } from '../authoring.ts';
import { ApplicationCatalog, catalogState } from '../catalog.ts';
import { ApplicationReferences, referenceState } from '../references.ts';
import type { MediaEdit } from '../../media/generated/MediaEdit';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { ApplicationSnapshot } from '../../media/generated/ApplicationSnapshot';

const flush = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
function deferred<T>() {
	let resolve!: (value: T) => void, reject!: (cause: unknown) => void;
	const promise = new Promise<T>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
}
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	currentHistoryStateId: 1,
	loading: { visible: false },
	hasActiveEditSession: false
});
const snapshot = (id: string): ApplicationSnapshot => ({
	id,
	label: id,
	enablement: 'Enabled',
	revision: 1,
	definition_instance: 'definition-lifetime',
	definition: {
		executable: null,
		arguments: [],
		directory: null,
		environment: null,
		window: 'Hidden',
		readiness: { kind: 'ProcessStarted' },
		stop: { graceful_ms: 2000, retirement_ms: 5000 },
		restart: { enabled: true, initial_ms: 500, maximum_ms: 30000 }
	}
});

test('availability sends the explicit intent, waits for backend state and fences replies after selection changes', async () => {
	const session = connection(),
		state = applicationState(),
		calls: MediaEdit[] = [];
	const pending = deferred<MediaEditResult>();
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_session, edit) => {
			calls.push(edit);
			if (edit.kind !== 'Applications') throw new Error('wrong route');
			if (edit.edit.kind === 'SetEnabled') return pending.promise;
			if (edit.edit.kind === 'Inspect')
				return { kind: 'Application', snapshot: snapshot(edit.edit.application) };
			return { kind: 'Applications', page: { applications: [], total: 0, start: 0 } };
		}
	);
	controller.select('first');
	await flush();
	const work = controller.edit({ kind: 'SetEnabled', application: 'first', enabled: false });
	assert.equal(
		state.snapshot?.enablement,
		'Enabled',
		'the UI cannot predict a committed availability outcome'
	);
	await controller.edit({ kind: 'SetEnabled', application: 'first', enabled: false });
	assert.equal(
		calls.filter((edit) => edit.kind === 'Applications' && edit.edit.kind === 'SetEnabled').length,
		1
	);
	controller.select('second');
	await flush();
	pending.reject(new Error('first selection rejected'));
	await work;
	await flush();
	assert.equal(state.selected, 'second');
	assert.equal(state.snapshot?.id, 'second');
	assert.equal(state.error, '');
	assert.equal(state.snapshot?.enablement, 'Enabled');
	controller.dispose();
});

test('catalog search bursts retain one active read and latest 64-item page; replaced projects reject late results', async () => {
	const session = connection(),
		state = catalogState(),
		reads: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	const catalog = new ApplicationCatalog(
		() => session,
		state,
		async (_s, edit) => {
			const reply = deferred<MediaEditResult>();
			reads.push({ edit, reply });
			return reply.promise;
		}
	);
	catalog.refresh();
	for (let i = 1; i <= 1000; i++) {
		state.search = `app${i}`;
		state.start = i * 64;
		catalog.refresh();
	}
	assert.equal(reads.length, 1);
	reads[0].reply.resolve({ kind: 'Applications', page: { applications: [], total: 0, start: 0 } });
	await flush();
	assert.equal(state.page, null);
	assert.equal(reads.length, 2);
	assert.deepEqual(reads[1].edit, {
		kind: 'Applications',
		edit: { kind: 'InspectCatalog', search: 'app1000', start: 64000, count: 64 }
	});
	session.snapshotGeneration++;
	reads[1].reply.resolve({
		kind: 'Applications',
		page: { applications: [], total: 1, start: 64000 }
	});
	await flush();
	assert.equal(state.page, null);
	catalog.dispose();
	catalog.refresh();
	assert.equal(reads.length, 2);
});

test('application selection discards stale snapshots and errors without launching or mutating', async () => {
	const session = connection(),
		state = applicationState(),
		reads: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_s, edit) => {
			const reply = deferred<MediaEditResult>();
			reads.push({ edit, reply });
			return reply.promise;
		}
	);
	controller.select('a');
	controller.select('b');
	reads[0].reply.reject(new Error('old missing definition'));
	await flush();
	assert.equal(state.readError, '');
	assert.equal(state.snapshot, null);
	reads[1].reply.resolve({ kind: 'Application', snapshot: snapshot('b') });
	await flush();
	assert.equal((state.snapshot as ApplicationSnapshot | null)?.id, 'b');
	assert.ok(
		reads.every((read) => read.edit.kind === 'Applications' && read.edit.edit.kind === 'Inspect')
	);
	controller.dispose();
});

test('create uses backend defaults and adopts only the current project result', async () => {
	const session = connection(),
		state = applicationState(),
		calls: MediaEdit[] = [];
	const pending = deferred<MediaEditResult>();
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_s, edit) => {
			calls.push(edit);
			return pending.promise;
		}
	);
	const work = controller.edit({ kind: 'Create', label: null });
	assert.deepEqual(calls, [{ kind: 'Applications', edit: { kind: 'Create', label: null } }]);
	assert.equal(state.pending, true);
	session.snapshotGeneration++;
	pending.resolve({ kind: 'ApplicationCreated', application: 'old-project-app' });
	await work;
	assert.equal(state.selected, null);
	assert.equal(state.pending, false);
	assert.equal(calls.length, 1);
	controller.dispose();
});

test('definition failures retain selection and saved data; active edit groups block authoring', async () => {
	const session = connection(),
		state = applicationState(),
		calls: MediaEdit[] = [];
	state.selected = 'a';
	state.snapshot = snapshot('a');
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_s, edit) => {
			calls.push(edit);
			if (edit.kind === 'Applications' && edit.edit.kind === 'Configure')
				throw new Error('backend rejected executable');
			if (edit.kind === 'Applications' && edit.edit.kind === 'Inspect')
				return { kind: 'Application', snapshot: snapshot('a') };
			return { kind: 'Applications', page: { applications: [], start: 0, total: 0 } };
		}
	);
	session.hasActiveEditSession = true;
	await controller.edit({ kind: 'Remove', application: 'a' });
	assert.equal(calls.length, 0);
	session.hasActiveEditSession = false;
	await controller.edit({
		kind: 'Configure',
		application: 'a',
		definition: snapshot('a').definition
	});
	await flush();
	assert.match(state.error, /backend rejected/);
	assert.equal(state.selected, 'a');
	assert.equal(state.snapshot?.id, 'a');
	assert.equal(state.pending, false);
	controller.dispose();
});

test('saved-reference lookup preserves backend missing states and ignores a stale column selection', async () => {
	const session = connection(),
		state = referenceState();
	let ids = ['old'];
	const reads: { edit: MediaEdit; reply: ReturnType<typeof deferred<MediaEditResult>> }[] = [];
	const reader = new ApplicationReferences(
		() => session,
		() => ids,
		state,
		async (_s, edit) => {
			const reply = deferred<MediaEditResult>();
			reads.push({ edit, reply });
			return reply.promise;
		}
	);
	reader.refresh();
	ids = ['missing', 'new'];
	reader.refresh();
	reads[0].reply.resolve({
		kind: 'ApplicationReferences',
		references: [{ application: 'old', summary: null }]
	});
	await flush();
	assert.equal(state.references.length, 0);
	assert.deepEqual(reads[1].edit, {
		kind: 'Applications',
		edit: { kind: 'InspectReferences', applications: ['missing', 'new'] }
	});
	reads[1].reply.resolve({
		kind: 'ApplicationReferences',
		references: [
			{ application: 'missing', summary: null },
			{
				application: 'new',
				summary: { id: 'new', label: 'Shared app', enabled: true, configured: false }
			}
		]
	});
	await flush();
	assert.equal(state.references[0].summary, null);
	assert.equal(state.references[1].summary?.label, 'Shared app');
	reader.dispose();
	reader.refresh();
	assert.equal(reads.length, 2);
});

test('committed creation selects the backend identity once and removal never invents another selection', async () => {
	const session = connection(),
		state = applicationState(),
		calls: MediaEdit[] = [];
	const controller = new ApplicationController(
		() => session,
		state,
		catalogState(),
		async (_s, edit) => {
			calls.push(edit);
			if (edit.kind !== 'Applications') throw new Error('unexpected route');
			if (edit.edit.kind === 'Create')
				return { kind: 'ApplicationCreated', application: 'backend-id' };
			if (edit.edit.kind === 'Inspect')
				return { kind: 'Application', snapshot: snapshot(edit.edit.application) };
			if (edit.edit.kind === 'Remove') return { kind: 'Changed' };
			return {
				kind: 'Applications',
				page: {
					applications: [{ id: 'other', label: 'Another app', configured: false, enabled: true }],
					total: 1,
					start: 0
				}
			};
		}
	);
	const work = controller.edit({ kind: 'Create', label: null });
	await controller.edit({ kind: 'Create', label: null });
	await work;
	await flush();
	assert.equal(state.selected, 'backend-id');
	assert.equal(state.snapshot?.id, 'backend-id');
	assert.equal(
		calls.filter((edit) => edit.kind === 'Applications' && edit.edit.kind === 'Create').length,
		1
	);
	await controller.edit({ kind: 'Remove', application: 'backend-id' });
	await flush();
	assert.equal(state.selected, null);
	assert.equal(state.snapshot, null);
	assert.ok(
		calls.every(
			(edit) =>
				edit.kind === 'Applications' && !['Configure', 'InspectRuntime'].includes(edit.edit.kind)
		)
	);
	controller.dispose();
});
