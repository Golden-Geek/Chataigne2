import { describe, expect, it, vi } from 'vitest';
import { createWorkbenchIntentPipeline } from '../../../../../../packages/golden-ui/store/session/intent-pipeline';
import type { UiAck, UiEditIntent } from '../../../../../../packages/golden-ui/types';

const deferred = <T>() => {
	let resolve!: (value: T | PromiseLike<T>) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
};

const appliedAck = (): UiAck => ({
	success: true,
	status: 'applied'
});

const setParam = (node: number, value: number): UiEditIntent => ({
	kind: 'setParam',
	node,
	value: { kind: 'int', value },
	behaviour: 'Coalesce'
});

describe('workbench intent pipeline burst handling', () => {
	it('enqueues and coalesces a large blocked parameter burst without scanning the queue', async () => {
		const firstDispatch = deferred<void>();
		const batches: UiEditIntent[][] = [];
		const pipeline = createWorkbenchIntentPipeline({
			getSnapshotGeneration: () => 1,
			async dispatchIntent() {
				await firstDispatch.promise;
			},
			async dispatchIntents(intents) {
				batches.push(intents);
			}
		});

		const blocker = pipeline.sendIntent({
			kind: 'sendNodeEvent',
			node: 1,
			topic: 'test.block',
			payload: null
		});
		const nodeCount = 20_000;
		const completions: Promise<void>[] = [];
		for (let node = 1; node <= nodeCount; node += 1) {
			completions.push(pipeline.sendIntent(setParam(node, node)));
		}
		for (let node = 1; node <= nodeCount; node += 1) {
			completions.push(pipeline.sendIntent(setParam(node, -node)));
		}

		firstDispatch.resolve();
		await blocker;
		await Promise.all(completions);

		expect(batches.every((batch) => batch.length <= 512)).toBe(true);
		const dispatched = batches.flat();
		expect(dispatched).toHaveLength(nodeCount);
		for (let index = 0; index < dispatched.length; index += 1) {
			const intent = dispatched[index];
			expect(intent).toEqual(setParam(index + 1, -(index + 1)));
		}
	}, 15_000);

	it('keeps every waiter and progress observer on a coalesced parameter intent', async () => {
		const firstDispatch = deferred<void>();
		const lifecycleA = vi.fn();
		const lifecycleB = vi.fn();
		const appliedA = vi.fn();
		const appliedB = vi.fn();
		const acknowledgements = [appliedAck()];
		const batches: UiEditIntent[][] = [];
		const pipeline = createWorkbenchIntentPipeline({
			getSnapshotGeneration: () => 1,
			async dispatchIntent() {
				await firstDispatch.promise;
			},
			async dispatchIntents(intents, progress) {
				batches.push(intents);
				progress?.lifecycle?.('received');
				progress?.applied?.(acknowledgements);
			}
		});

		const blocker = pipeline.sendIntent({
			kind: 'sendNodeEvent',
			node: 1,
			topic: 'test.block',
			payload: null
		});
		const first = pipeline.sendIntent(setParam(10, 1), {
			lifecycle: lifecycleA,
			applied: appliedA
		});
		const replacement = pipeline.sendIntent(setParam(10, 2), {
			lifecycle: lifecycleB,
			applied: appliedB
		});

		firstDispatch.resolve();
		await Promise.all([blocker, first, replacement]);

		expect(batches).toEqual([[setParam(10, 2)]]);
		expect(lifecycleA).toHaveBeenCalledWith('received');
		expect(lifecycleB).toHaveBeenCalledWith('received');
		expect(appliedA).toHaveBeenCalledWith(acknowledgements);
		expect(appliedB).toHaveBeenCalledWith(acknowledgements);
	});
});
