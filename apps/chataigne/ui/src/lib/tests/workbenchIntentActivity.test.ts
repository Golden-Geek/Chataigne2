import { describe, expect, it, vi } from 'vitest';
import { UI_PROTOCOL_VERSION } from '../../../../../../packages/golden-ui/generated/rust_protocol/protocol-version';
import { createWorkbenchSession } from '../../../../../../packages/golden-ui/store/workbench.svelte';
import { userActionLabelForIntent } from '../../../../../../packages/golden-ui/store/session/action-activity.svelte';
import type {
	UiAck,
	UiClient,
	UiControlLifecycle,
	UiEditIntent,
	UiEventBatch,
	UiSnapshot
} from '../../../../../../packages/golden-ui/types';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
};

const successfulAck = (): UiAck => ({
	success: true,
	status: 'applied'
});

const initialSnapshot = (): UiSnapshot =>
	({
		protocol_version: UI_PROTOCOL_VERSION,
		scope: { kind: 'wholeGraph' },
		at: { tick: 1, micro: 0, seq: 1 },
		nodes: [],
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

interface PendingSingleIntent {
	intent: UiEditIntent;
	onLifecycle?: (phase: UiControlLifecycle) => void;
	result: ReturnType<typeof deferred<UiAck>>;
}

interface PendingIntentBatch {
	intents: UiEditIntent[];
	onLifecycle?: (phase: UiControlLifecycle) => void;
	result: ReturnType<typeof deferred<UiAck[]>>;
}

const createControlledClient = () => {
	const singles: PendingSingleIntent[] = [];
	const batches: PendingIntentBatch[] = [];
	let batchListener: ((batch: UiEventBatch) => void) | null = null;
	const client = {
		snapshot: async (): Promise<UiSnapshot> => initialSnapshot(),
		subscribe(_scope: unknown, _from: unknown, onBatch: (batch: UiEventBatch) => void): () => void {
			batchListener = onBatch;
			return () => {
				batchListener = null;
			};
		},
		sendIntent(
			intent: UiEditIntent,
			onLifecycle?: (phase: UiControlLifecycle) => void
		): Promise<UiAck> {
			const result = deferred<UiAck>();
			singles.push({ intent, onLifecycle, result });
			return result.promise;
		},
		sendIntents(
			intents: UiEditIntent[],
			onLifecycle?: (phase: UiControlLifecycle) => void
		): Promise<UiAck[]> {
			const result = deferred<UiAck[]>();
			batches.push({ intents, onLifecycle, result });
			return result.promise;
		}
	} as unknown as UiClient;
	return {
		client,
		singles,
		batches,
		hasSubscription: (): boolean => batchListener !== null,
		emitBatch: (batch: UiEventBatch): void => {
			if (!batchListener) {
				throw new Error('workbench is not subscribed');
			}
			batchListener(batch);
		}
	};
};

describe('workbench intent activity', () => {
	it('labels every user-context mutation explicitly', () => {
		expect(userActionLabelForIntent({ kind: 'ensureUserContextScope', owner: 1 })).toBe(
			'Adding user context'
		);
		expect(userActionLabelForIntent({ kind: 'removeUserContextScope', owner: 1 })).toBe(
			'Removing user context'
		);
		expect(
			userActionLabelForIntent({
				kind: 'upsertUserContextEntry',
				owner: 1,
				symbol: 'speed',
				param: 2
			})
		).toBe('Updating user context');
		expect(
			userActionLabelForIntent({
				kind: 'removeUserContextEntry',
				owner: 1,
				symbol: 'speed'
			})
		).toBe('Removing user context entry');
	});

	it('records a visible user action synchronously and follows transport lifecycle phases', async () => {
		const controlled = createControlledClient();
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => controlled.client
		});

		const completion = session.sendIntent({
			kind: 'duplicateNode',
			source: 10,
			new_parent: 20
		});

		expect(session.userActionPendingCount).toBe(1);
		expect(session.userActionCurrentLabel).toBe('Duplicating node');
		expect(session.userActionCurrentPhase).toBe('queued');
		expect(session.userActionOldestQueuedAtMs).not.toBeNull();
		expect(controlled.singles).toHaveLength(1);

		controlled.singles[0].onLifecycle?.('received');
		expect(session.userActionCurrentPhase).toBe('received');
		controlled.singles[0].onLifecycle?.('accepted');
		expect(session.userActionCurrentPhase).toBe('accepted');

		controlled.singles[0].result.resolve(successfulAck());
		await completion;
		await vi.waitFor(() => expect(session.userActionPendingCount).toBe(0));
		expect(session.userActionCurrentLabel).toBeNull();
		expect(session.userActionCurrentPhase).toBeNull();
	});

	it('keeps the finishing state through the transport render frame', async () => {
		const frames: FrameRequestCallback[] = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback): number => {
			frames.push(callback);
			return frames.length;
		});
		try {
			const controlled = createControlledClient();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => controlled.client
			});
			const completion = session.sendIntent({
				kind: 'duplicateNode',
				source: 10,
				new_parent: 20
			});

			controlled.singles[0].onLifecycle?.('applied');
			controlled.singles[0].result.resolve(successfulAck());
			for (let attempt = 0; attempt < 10 && frames.length === 0; attempt += 1) {
				await Promise.resolve();
			}
			expect(session.userActionCurrentPhase).toBe('applied');
			expect(session.userActionPendingCount).toBe(1);
			expect(frames).toHaveLength(1);
			await completion;
			expect(session.userActionPendingCount).toBe(1);

			frames.shift()?.(0);
			for (let attempt = 0; attempt < 10 && frames.length === 0; attempt += 1) {
				await Promise.resolve();
			}
			expect(session.userActionPendingCount).toBe(1);
			expect(frames).toHaveLength(1);

			frames.shift()?.(16);
			await vi.waitFor(() => expect(session.userActionPendingCount).toBe(0));
		} finally {
			vi.unstubAllGlobals();
		}
	});

	it('waits for the final acknowledged event before retiring the finishing badge', async () => {
		const frames: FrameRequestCallback[] = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback): number => {
			frames.push(callback);
			return frames.length;
		});
		let cleanup: (() => void) | null = null;
		try {
			const controlled = createControlledClient();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => controlled.client
			});
			cleanup = session.mount();
			await vi.waitFor(() => expect(controlled.hasSubscription()).toBe(true));

			const completion = session.sendIntent({
				kind: 'duplicateNode',
				source: 10,
				new_parent: 20
			});
			controlled.singles[0].onLifecycle?.('applied');
			controlled.singles[0].result.resolve({
				...successfulAck(),
				earliest_event_time: { tick: 1, micro: 0, seq: 2 },
				latest_event_time: { tick: 1, micro: 0, seq: 3 }
			});
			await completion;
			await Promise.resolve();
			expect(session.userActionPendingCount).toBe(1);
			expect(frames).toHaveLength(0);

			controlled.emitBatch({
				from: { tick: 1, micro: 0, seq: 1 },
				to: { tick: 1, micro: 0, seq: 2 },
				events: [
					{
						time: { tick: 1, micro: 0, seq: 2 },
						kind: {
							kind: 'custom',
							topic: 'preview',
							payload: null,
							retention: 'transient'
						}
					}
				]
			});
			await Promise.resolve();
			expect(session.userActionPendingCount).toBe(1);
			expect(frames).toHaveLength(0);

			controlled.emitBatch({
				from: { tick: 1, micro: 0, seq: 2 },
				to: { tick: 1, micro: 0, seq: 3 },
				events: [
					{
						time: { tick: 1, micro: 0, seq: 3 },
						kind: {
							kind: 'custom',
							topic: 'preview',
							payload: null,
							retention: 'transient'
						}
					}
				]
			});
			for (let attempt = 0; attempt < 10 && frames.length === 0; attempt += 1) {
				await Promise.resolve();
			}
			expect(frames).toHaveLength(1);
			frames.shift()?.(0);
			for (let attempt = 0; attempt < 10 && frames.length === 0; attempt += 1) {
				await Promise.resolve();
			}
			expect(session.userActionPendingCount).toBe(1);
			frames.shift()?.(16);
			await vi.waitFor(() => expect(session.userActionPendingCount).toBe(0));
		} finally {
			cleanup?.();
			vi.unstubAllGlobals();
		}
	});

	it('bounds a missing event boundary without blocking command completion', async () => {
		vi.useFakeTimers();
		const frames: FrameRequestCallback[] = [];
		vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback): number => {
			frames.push(callback);
			return frames.length;
		});
		try {
			const controlled = createControlledClient();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => controlled.client
			});
			const completion = session.sendIntent({
				kind: 'duplicateNode',
				source: 10,
				new_parent: 20
			});
			controlled.singles[0].onLifecycle?.('applied');
			controlled.singles[0].result.resolve({
				...successfulAck(),
				earliest_event_time: { tick: 9, micro: 0, seq: 1 }
			});

			await completion;
			expect(session.userActionPendingCount).toBe(1);
			await vi.advanceTimersByTimeAsync(4_999);
			expect(session.userActionPendingCount).toBe(1);
			expect(frames).toHaveLength(0);

			await vi.advanceTimersByTimeAsync(1);
			expect(frames).toHaveLength(1);
			frames.shift()?.(5_000);
			await Promise.resolve();
			expect(frames).toHaveLength(1);
			frames.shift()?.(5_016);
			for (let attempt = 0; attempt < 10 && session.userActionPendingCount !== 0; attempt += 1) {
				await Promise.resolve();
			}
			expect(session.userActionPendingCount).toBe(0);
			await vi.advanceTimersByTimeAsync(1_000);
			expect(session.userActionPendingCount).toBe(0);
		} finally {
			vi.useRealTimers();
			vi.unstubAllGlobals();
		}
	});

	it('keeps live parameter and node-event traffic silent', async () => {
		const controlled = createControlledClient();
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => controlled.client
		});

		const parameterCompletion = session.sendIntent({
			kind: 'setParam',
			node: 10,
			value: { kind: 'float', value: 0.5 },
			behaviour: 'Coalesce'
		});
		expect(session.userActionPendingCount).toBe(0);
		expect(controlled.singles).toHaveLength(1);
		controlled.singles[0].result.resolve(successfulAck());
		await parameterCompletion;

		const eventCompletion = session.sendIntent({
			kind: 'sendNodeEvent',
			node: 10,
			topic: 'preview.demand',
			payload: null
		});
		expect(session.userActionPendingCount).toBe(0);
		expect(controlled.singles).toHaveLength(2);
		controlled.singles[1].result.resolve(successfulAck());
		await eventCompletion;
	});

	it('uses the edit label for a visible intent batch and counts overlapping actions', async () => {
		const controlled = createControlledClient();
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => controlled.client
		});

		const firstCompletion = session.sendIntent({
			kind: 'duplicateNode',
			source: 1,
			new_parent: 2
		});
		const batch: UiEditIntent[] = [
			{ kind: 'beginEdit', client_edit_id: 'duplicate-state', label: 'Duplicating state' },
			{
				kind: 'duplicateNodes',
				nodes: [{ source: 3, new_parent: 4 }]
			},
			{ kind: 'endEdit', client_edit_id: 'duplicate-state' }
		];
		const batchCompletion = session.sendIntents(batch);

		expect(session.userActionPendingCount).toBe(2);
		expect(session.userActionCurrentLabel).toBe('Duplicating node');
		expect(controlled.batches).toHaveLength(1);

		controlled.singles[0].result.resolve(successfulAck());
		await firstCompletion;
		expect(session.userActionPendingCount).toBe(1);
		expect(session.userActionCurrentLabel).toBe('Duplicating state');

		controlled.batches[0].onLifecycle?.('accepted');
		expect(session.userActionCurrentPhase).toBe('accepted');
		controlled.batches[0].result.resolve(batch.map(() => successfulAck()));
		await batchCompletion;
		await vi.waitFor(() => expect(session.userActionPendingCount).toBe(0));
	});

	it('shows a labeled edit batch even when its parameter intents are individually silent', async () => {
		const controlled = createControlledClient();
		const session = createWorkbenchSession({
			enableGlobalShortcuts: false,
			transportFactory: () => controlled.client
		});
		const batch: UiEditIntent[] = [
			{ kind: 'beginEdit', client_edit_id: 'parameter-batch', label: 'Updating processor' },
			{
				kind: 'setParam',
				node: 10,
				value: { kind: 'float', value: 0.5 },
				behaviour: 'Coalesce'
			},
			{ kind: 'endEdit', client_edit_id: 'parameter-batch' }
		];

		const completion = session.sendIntents(batch);
		expect(session.userActionPendingCount).toBe(1);
		expect(session.userActionCurrentLabel).toBe('Updating processor');

		controlled.batches[0].result.resolve(batch.map(() => successfulAck()));
		await completion;
		await vi.waitFor(() => expect(session.userActionPendingCount).toBe(0));
	});

	it('keeps backend rejection feedback visible for a minimum dwell', async () => {
		vi.useFakeTimers();
		try {
			const controlled = createControlledClient();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => controlled.client
			});

			const completion = session.sendIntent({ kind: 'removeNode', node: 10 });
			controlled.singles[0].onLifecycle?.('rejected');
			controlled.singles[0].result.resolve({
				success: false,
				status: 'rejected',
				error_message: 'node is protected'
			});

			await expect(completion).rejects.toThrow('node is protected');
			expect(session.userActionPendingCount).toBe(1);
			expect(session.userActionCurrentPhase).toBe('rejected');

			await vi.advanceTimersByTimeAsync(1199);
			expect(session.userActionPendingCount).toBe(1);
			await vi.advanceTimersByTimeAsync(1);
			expect(session.userActionPendingCount).toBe(0);
		} finally {
			vi.useRealTimers();
		}
	});

	it('keeps terminal transport errors visible as Failed before retiring them', async () => {
		vi.useFakeTimers();
		try {
			const controlled = createControlledClient();
			const session = createWorkbenchSession({
				enableGlobalShortcuts: false,
				transportFactory: () => controlled.client
			});

			const completion = session.sendIntent({ kind: 'removeNode', node: 10 });
			expect(session.userActionPendingCount).toBe(1);
			controlled.singles[0].result.reject(new Error('runtime refused mutation'));

			await expect(completion).rejects.toThrow('runtime refused mutation');
			expect(session.userActionPendingCount).toBe(1);
			expect(session.userActionCurrentPhase).toBe('rejected');
			expect(session.logRecords.at(-1)?.message).toContain('runtime refused mutation');

			await vi.advanceTimersByTimeAsync(1199);
			expect(session.userActionPendingCount).toBe(1);
			await vi.advanceTimersByTimeAsync(1);
			expect(session.userActionPendingCount).toBe(0);

			const batchCompletion = session.sendIntents([
				{ kind: 'removeNode', node: 11 },
				{ kind: 'removeNode', node: 12 }
			]);
			controlled.batches[0].result.reject(new Error('batch transport timeout'));
			await expect(batchCompletion).rejects.toThrow('batch transport timeout');
			expect(session.userActionPendingCount).toBe(1);
			expect(session.userActionCurrentPhase).toBe('rejected');
			await vi.advanceTimersByTimeAsync(1200);
			expect(session.userActionPendingCount).toBe(0);
		} finally {
			vi.useRealTimers();
		}
	});
});
