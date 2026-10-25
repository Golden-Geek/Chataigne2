import { describe, expect, it } from 'vitest';
import { createWorkbenchIntentPipeline } from '../intent-pipeline';
import type { UiEditIntent } from '../../../types';

const deferred = () => {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
};
const event = (node: number): UiEditIntent => ({
	kind: 'sendNodeEvent',
	node,
	topic: 'barrier',
	payload: null
});
describe('session intent flush', () => {
	it('waits for active batches and queued single intents without discarding them', async () => {
		const batch = deferred(),
			first = deferred(),
			events: number[] = [];
		const pipeline = createWorkbenchIntentPipeline({
			getSnapshotGeneration: () => 1,
			async dispatchIntent(intent) {
				if (intent.kind !== 'sendNodeEvent') throw new Error('Unexpected intent');
				events.push(intent.node);
				if (intent.node === 1) await first.promise;
			},
			async dispatchIntents() {
				await batch.promise;
			}
		});
		const a = pipeline.sendIntent(event(1)),
			b = pipeline.sendIntent(event(2)),
			c = pipeline.sendIntents([event(3)]);
		let flushed = false;
		const flush = pipeline.flush().then(() => {
			flushed = true;
		});
		first.resolve();
		await a;
		await b;
		expect(events).toEqual([1, 2]);
		expect(flushed).toBe(false);
		batch.resolve();
		await c;
		await flush;
		expect(flushed).toBe(true);
	});
	it('refuses both an existing block and a block introduced during drainage', async () => {
		const active = deferred();
		const pipeline = createWorkbenchIntentPipeline({
			getSnapshotGeneration: () => 1,
			dispatchIntent: () => active.promise,
			dispatchIntents: async () => {}
		});
		const sent = pipeline.sendIntent(event(1));
		const flush = pipeline.flush();
		const blocked = pipeline.blockDispatch(new Error('snapshot recovery'));
		await expect(pipeline.flush()).rejects.toThrow('snapshot recovery');
		active.resolve();
		await sent;
		await expect(flush).rejects.toThrow('snapshot recovery');
		blocked();
		await pipeline.flush();
		const resume = pipeline.suspendDispatch();
		await expect(pipeline.flush()).rejects.toThrow('synchronizing');
		resume();
	});
});
