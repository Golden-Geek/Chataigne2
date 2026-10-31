import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ApplicationRuntimeReader, runtimeState } from '../runtime.ts';
import { configurationLabel, serviceLabel } from '../runtime-presentation.ts';
import type { ApplicationRuntimeSnapshot } from '../../media/generated/ApplicationRuntimeSnapshot';
import type { MediaEditResult } from '../../media/generated/MediaEditResult';
import type { ApplicationSnapshot } from '../../media/generated/ApplicationSnapshot';

const flush = async () => {
	for (let i = 0; i < 12; i++) await Promise.resolve();
};
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
}
const connection = () => ({
	status: 'connected',
	snapshotGeneration: 1,
	loading: { visible: false }
});
function snapshot(project: string, revision: number): ApplicationRuntimeSnapshot {
	return {
		supervision: { host: 'Desktop', demand: { kind: 'Running' }, callbacks: { kind: 'Running' } },
		callbacks: { pending: 0, acknowledged: 0, fault: null },
		project,
		revision,
		elapsed_ms: revision * 100,
		applications: [
			{
				callback_backpressure: false,
				application: 'app',
				launch: 'launch',
				phase: 'Ready',
				process_id: 42,
				exit_code: null,
				failures: 0,
				next_action_ms: null,
				message: null,
				configuration: 'Current',
				definition_revision: 3,
				definition_instance: 'definition-lifetime'
			}
		]
	};
}

test('service admission errors survive stale observations and recovery preserves an independent callback fault', async () => {
	const session = connection(),
		state = runtimeState();
	let listener!: (snapshot: ApplicationRuntimeSnapshot) => void;
	const reader = new ApplicationRuntimeReader(
		() => session,
		state,
		async () => ({ kind: 'ApplicationRuntime', snapshot: snapshot('project', 1) }),
		{
			available: () => true,
			subscribe: async (accept) => {
				listener = accept;
				return () => {};
			}
		}
	);
	reader.observe();
	await flush();
	const failed = snapshot('project', 2);
	failed.applications = [];
	failed.supervision.host = 'Headless';
	failed.supervision.demand = { kind: 'Retrying', message: 'Capacity held by retiring owners' };
	listener(failed);
	listener(snapshot('project', 1));
	assert.equal(
		serviceLabel(state.snapshot!.supervision.demand),
		'Retrying: Capacity held by retiring owners'
	);
	assert.deepEqual(state.snapshot!.applications, []);
	const recovered = snapshot('project', 3);
	recovered.supervision.callbacks = {
		kind: 'Halted',
		message: 'Partial delivery cannot be replayed'
	};
	listener(recovered);
	listener(snapshot('foreign', 99));
	listener(failed);
	assert.equal(serviceLabel(state.snapshot!.supervision.demand), 'Running');
	assert.equal(
		serviceLabel(state.snapshot!.supervision.callbacks),
		'Halted: Partial delivery cannot be replayed'
	);
	reader.dispose();
});

test('runtime bootstrap closes the subscribe race and accepts only newer same-project observations without polling', async () => {
	const session = connection(),
		state = runtimeState(),
		reads: ReturnType<typeof deferred<MediaEditResult>>[] = [];
	let listener!: (snapshot: ApplicationRuntimeSnapshot) => void;
	const reader = new ApplicationRuntimeReader(
		() => session,
		state,
		async () => {
			const reply = deferred<MediaEditResult>();
			reads.push(reply);
			return reply.promise;
		},
		{
			available: () => true,
			subscribe: async (accept) => {
				listener = accept;
				return () => {};
			}
		}
	);
	reader.observe();
	await flush();
	listener(snapshot('project', 8));
	reads[0].resolve({ kind: 'ApplicationRuntime', snapshot: snapshot('project', 1) });
	await flush();
	assert.equal(reads.length, 2);
	listener(snapshot('project', 8));
	reads[1].resolve({ kind: 'ApplicationRuntime', snapshot: snapshot('project', 3) });
	await flush();
	listener(snapshot('foreign-project', 99));
	listener(snapshot('project', 7));
	assert.equal(state.snapshot?.revision, 8);
	reader.observe();
	await flush();
	assert.equal(reads.length, 2);
	assert.equal(state.live, true);
	reader.dispose();
});

test('replaced projects dispose late listeners and discard their events and replies', async () => {
	const session = connection(),
		state = runtimeState();
	const subscriptions: {
		accept(snapshot: ApplicationRuntimeSnapshot): void;
		reply: ReturnType<typeof deferred<() => void>>;
	}[] = [];
	let disposed = 0;
	const reader = new ApplicationRuntimeReader(
		() => session,
		state,
		async () => ({ kind: 'ApplicationRuntime', snapshot: snapshot('current', 1) }),
		{
			available: () => true,
			subscribe: (accept) => {
				const reply = deferred<() => void>();
				subscriptions.push({ accept, reply });
				return reply.promise;
			}
		}
	);
	reader.observe();
	session.snapshotGeneration++;
	reader.observe();
	subscriptions[0].reply.resolve(() => {
		disposed++;
	});
	await flush();
	subscriptions[0].accept(snapshot('old', 99));
	assert.equal(state.snapshot, null);
	assert.equal(disposed, 1);
	subscriptions[1].reply.resolve(() => {
		disposed++;
	});
	await flush();
	assert.equal((state.snapshot as ApplicationRuntimeSnapshot | null)?.project, 'current');
	reader.dispose();
	subscriptions[1].accept(snapshot('current', 200));
	assert.equal(state.snapshot, null);
	assert.equal(disposed, 2);
});

test('snapshot-only readers fence disconnected reads and do not schedule periodic engine inspection', async () => {
	const session = connection(),
		state = runtimeState(),
		replies: ReturnType<typeof deferred<MediaEditResult>>[] = [];
	const reader = new ApplicationRuntimeReader(
		() => session,
		state,
		async () => {
			const reply = deferred<MediaEditResult>();
			replies.push(reply);
			return reply.promise;
		},
		{ available: () => false, subscribe: async () => () => {} }
	);
	reader.observe();
	session.status = 'disconnected';
	reader.observe();
	replies[0].resolve({ kind: 'ApplicationRuntime', snapshot: snapshot('retired', 10) });
	await flush();
	assert.equal(state.snapshot, null);
	assert.equal(state.live, false);
	assert.equal(replies.length, 1);
	reader.dispose();
});

test('event registration retries independently of engine inspection and cancels on disposal', async () => {
	const session = connection(),
		state = runtimeState();
	let attempts = 0,
		reads = 0;
	const reader = new ApplicationRuntimeReader(
		() => session,
		state,
		async () => {
			reads++;
			return { kind: 'ApplicationRuntime', snapshot: snapshot('project', reads) };
		},
		{
			available: () => true,
			subscribe: async () => {
				if (++attempts === 1) throw new Error('temporary event outage');
				return () => {};
			}
		}
	);
	reader.observe();
	await flush();
	assert.equal(reads, 1);
	assert.equal(state.live, false);
	assert.match(state.transportError, /outage/);
	await new Promise((resolve) => setTimeout(resolve, 1100));
	await flush();
	assert.equal(attempts, 2);
	assert.equal(state.live, true);
	assert.equal(state.transportError, '');
	assert.equal(reads, 2);
	reader.dispose();
});

test('configuration presentation uses worker policy and hides stale saved-revision comparisons', () => {
	const status = snapshot('project', 1).applications[0];
	const saved: ApplicationSnapshot = {
		id: 'app',
		label: 'App',
		enabled: true,
		revision: 4,
		definition_instance: 'definition-lifetime',
		definition: {
			executable: null,
			arguments: [],
			directory: null,
			environment: null,
			window: 'Hidden',
			readiness: { kind: 'ProcessStarted' },
			stop: { graceful_ms: 10, retirement_ms: 20 },
			restart: { enabled: false, initial_ms: 10, maximum_ms: 20 }
		}
	};
	assert.match(configurationLabel(status, saved), /awaiting/);
	status.definition_revision = 4;
	status.definition_instance = 'old-definition-lifetime';
	assert.match(configurationLabel(status, saved), /awaiting/);
	status.definition_instance = saved.definition_instance;
	status.configuration = 'Pending';
	assert.match(configurationLabel(status, saved), /previous configuration/);
	status.configuration = 'Retained';
	status.definition_revision = null;
	assert.match(configurationLabel(status, saved), /retained/);
	status.configuration = 'NotRunning';
	assert.match(configurationLabel(status, saved), /No configuration/);
});
