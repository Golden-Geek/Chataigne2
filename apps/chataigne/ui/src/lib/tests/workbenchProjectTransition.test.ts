import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { UI_PROTOCOL_VERSION } from '../../../../../../packages/golden-ui/generated/rust_protocol/protocol-version';
import { createWorkbenchIntentPipeline } from '../../../../../../packages/golden-ui/store/session/intent-pipeline';
import { createWorkbenchSession } from '../../../../../../packages/golden-ui/store/workbench.svelte';
import type {
	UiClient,
	UiEditIntent,
	UiNodeDto,
	UiSnapshot
} from '../../../../../../packages/golden-ui/types';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
};

const node = (nodeId: number): UiNodeDto =>
	({
		node_id: nodeId,
		uuid: `00000000-0000-0000-0000-${nodeId.toString().padStart(12, '0')}`,
		decl_id: 'root',
		node_type: 'folder',
		meta: {
			label: 'Root',
			short_name: 'root',
			enabled: true,
			can_be_disabled: false,
			user_permissions: {
				can_edit_name: false,
				can_remove_and_duplicate: false,
				can_edit_constraints: false,
				can_edit_tags: false,
				can_edit_color: false
			},
			tags: []
		},
		data: { kind: 'node', node_type: 'folder' },
		user_role: 'regular',
		user_item_kind: '',
		accepted_user_item_kinds: [],
		creatable_user_items: [],
		children: []
	}) as UiNodeDto;

const snapshot = (nodeId: number, seq: number): UiSnapshot =>
	({
		protocol_version: UI_PROTOCOL_VERSION,
		scope: { kind: 'wholeGraph' },
		at: { tick: 1, micro: 0, seq },
		nodes: [node(nodeId)],
		schema: { node_types: [], declared_descriptions: [], enums: [] },
		history: {
			can_undo: false,
			can_redo: false,
			undo_len: 0,
			redo_len: 0,
			active_edit_session: false,
			current_history_state_id: 0
		},
		logger: { max_entries: 100, records: [] },
		project_file: {
			display_name: 'Test',
			extension: 'noisette',
			current_path: null
		}
	}) as UiSnapshot;

const nodeEvent = (nodeId: number): UiEditIntent => ({
	kind: 'sendNodeEvent',
	node: nodeId,
	topic: 'test.demand',
	payload: null
});

describe('workbench project graph transitions', () => {
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
