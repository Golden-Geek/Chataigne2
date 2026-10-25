import { snapshot } from './fixtures/workbench-graph';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { createWorkbenchIntentPipeline } from '../../../../../../packages/golden-ui/store/session/intent-pipeline';
import { createWorkbenchSession } from '../../../../../../packages/golden-ui/store/workbench.svelte';
import type {
	UiClient,
	UiEditIntent,
	UiSnapshot
} from '../../../../../../packages/golden-ui/types';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
};

const nodeEvent = (nodeId: number): UiEditIntent => ({
	kind: 'sendNodeEvent',
	node: nodeId,
	topic: 'test.demand',
	payload: null
});

describe('workbench project graph transitions', () => {
	it.each(['undo', 'redo'] as const)(
		'reports a retired %s barrier without rejecting fire-and-forget UI callers',
		async (action) => {
			const pending = deferred<void>();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => ({}) as UiClient
			});
			const operation = session.editOperations.open({
				current: () => true,
				build: () => ({ finish: () => pending.promise, cancel: () => pending.promise })
			})!;
			void operation.confirm();
			const outcome = expect(session[action]()).resolves.toBeUndefined();
			session.editOperations.reset();
			pending.resolve();
			await outcome;
		}
	);
	it('drains active work and cancels every intent produced inside the barrier', async () => {
		let snapshotGeneration = 1;
		const firstDispatch = deferred<void>();
		const dispatched: UiEditIntent[] = [];
		const pipeline = createWorkbenchIntentPipeline({
			getSnapshotGeneration: () => snapshotGeneration,
			async dispatchIntent(intent) {
				dispatched.push(intent);
				if (dispatched.length === 1) {
					await firstDispatch.promise;
				}
			},
			async dispatchIntents(intents) {
				dispatched.push(...intents);
			}
		});

		const activeIntent = pipeline.sendIntent(nodeEvent(10));
		await vi.waitFor(() => expect(dispatched).toHaveLength(1));

		let transitionStarted = false;
		let oldSnapshotIntentOutcome!: Promise<string>;
		let finalGraphIntentOutcome!: Promise<string>;
		let finalGraphEditOutcome!: Promise<string>;
		const transition = pipeline.runProjectGraphTransition(async () => {
			transitionStarted = true;
			oldSnapshotIntentOutcome = pipeline.sendIntent(nodeEvent(10)).then(
				() => 'resolved',
				(error: Error) => error.message
			);
			snapshotGeneration = 2;
			finalGraphIntentOutcome = pipeline.sendIntent(nodeEvent(30)).then(
				() => 'resolved',
				(error: Error) => error.message
			);
			finalGraphEditOutcome = pipeline.sendIntent({ kind: 'clearLogs' }).then(
				() => 'resolved',
				(error: Error) => error.message
			);
		});

		await Promise.resolve();
		expect(transitionStarted).toBe(false);

		firstDispatch.resolve();
		await activeIntent;
		await transition;

		expect(await oldSnapshotIntentOutcome).toContain('produced during a project graph transition');
		expect(await finalGraphIntentOutcome).toContain('produced during a project graph transition');
		expect(await finalGraphEditOutcome).toContain('produced during a project graph transition');
		expect(dispatched).toEqual([nodeEvent(10)]);
	});

	it('requests a fresh post-replacement snapshot after an older refresh settles', async () => {
		const oldSnapshot = deferred<UiSnapshot>();
		const freshSnapshot = deferred<UiSnapshot>();
		const snapshotRequests = [oldSnapshot, freshSnapshot];
		const dispatchedIntents: UiEditIntent[] = [];
		const client = {
			snapshot: vi.fn(() => {
				const request = snapshotRequests.shift();
				if (!request) throw new Error('unexpected snapshot request');
				return request.promise;
			}),
			sendIntent: vi.fn(async (intent: UiEditIntent) => {
				dispatchedIntents.push(intent);
				return { success: true, status: 'applied' as const };
			})
		} as unknown as UiClient;
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => client
		});

		const refresh = session.refreshSnapshot();
		let transitionStartedAtGeneration: number | null = null;
		let staleTeardownOutcome!: Promise<string>;
		let barrierSubscriptionOutcome!: Promise<string>;
		const transition = session.runProjectGraphTransition(async ({ markRuntimeGraphReplaced }) => {
			transitionStartedAtGeneration = session.snapshotGeneration;
			markRuntimeGraphReplaced();
			const refreshed = await session.refreshSnapshotAfterCurrentSynchronization();
			expect(refreshed).toBe(true);
			void tick().then(() => {
				staleTeardownOutcome = session.sendIntent(nodeEvent(1)).then(
					() => 'resolved',
					(error: Error) => error.message
				);
				barrierSubscriptionOutcome = session.sendIntent(nodeEvent(2)).then(
					() => 'resolved',
					(error: Error) => error.message
				);
			});
		});

		await Promise.resolve();
		expect(transitionStartedAtGeneration).toBeNull();

		oldSnapshot.resolve(snapshot(1, 1));
		await expect(refresh).resolves.toBe(true);
		await vi.waitFor(() => expect(client.snapshot).toHaveBeenCalledTimes(2));
		expect(session.graphTransitionRevision).toBe(0);
		freshSnapshot.resolve(snapshot(2, 2));
		await transition;

		expect(transitionStartedAtGeneration).toBe(1);
		expect(session.snapshotGeneration).toBe(2);
		expect(session.graphTransitionRevision).toBe(1);
		expect(await staleTeardownOutcome).toContain('produced during a project graph transition');
		expect(await barrierSubscriptionOutcome).toContain(
			'produced during a project graph transition'
		);
		await session.sendIntent(nodeEvent(2));
		expect(dispatchedIntents).toEqual([nodeEvent(2)]);
	});

	it('keeps dispatch gated after refresh failure until a later snapshot recovers', async () => {
		let snapshotRequest = 0;
		const dispatchedIntents: UiEditIntent[] = [];
		const client = {
			snapshot: vi.fn(async () => {
				snapshotRequest += 1;
				if (snapshotRequest === 1) {
					return snapshot(1, 1);
				}
				if (snapshotRequest === 2) {
					throw new Error('required snapshot failed');
				}
				return snapshot(2, 2);
			}),
			sendIntent: vi.fn(async (intent: UiEditIntent) => {
				dispatchedIntents.push(intent);
				return { success: true, status: 'applied' as const };
			})
		} as unknown as UiClient;
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => client
		});

		const transitionResult = await session.runProjectGraphTransition(
			async ({ markRuntimeGraphReplaced }) => {
				markRuntimeGraphReplaced();
				await expect(session.refreshSnapshot()).resolves.toBe(true);
				return session.refreshSnapshotAfterCurrentSynchronization();
			}
		);

		expect(transitionResult).toBe(false);
		expect(session.graphTransitionRevision).toBe(0);

		const blockedOutcomes = await Promise.all(
			Array.from({ length: 256 }, () =>
				session.sendIntent(nodeEvent(1)).then(
					() => 'resolved',
					(error: Error) => error.message
				)
			)
		);
		expect(blockedOutcomes).toHaveLength(256);
		expect(blockedOutcomes.every((outcome) => outcome.includes('not synchronized'))).toBe(true);
		await expect(session.sendIntents([nodeEvent(1)])).rejects.toThrow('not synchronized');
		expect(dispatchedIntents).toEqual([]);

		await expect(session.refreshSnapshot()).resolves.toBe(true);
		expect(session.graphTransitionRevision).toBe(1);
		expect(dispatchedIntents).toEqual([]);

		await session.sendIntent(nodeEvent(2));
		expect(dispatchedIntents).toEqual([nodeEvent(2)]);
	});
});
