import { describe, expect, it } from 'vitest';
import { createWorkbenchIntentPipeline } from '../intent-pipeline';
import type { UiEditIntent } from '../../../types';

const edit = (node: number): UiEditIntent => ({
	kind: 'sendNodeEvent',
	node,
	topic: 'geometry',
	payload: null
});
function fixture() {
	let connected = true,
		generation = 1,
		release!: () => void;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const submitted: UiEditIntent[] = [];
	const pipeline = createWorkbenchIntentPipeline({
		canDispatch: () => connected,
		getSnapshotGeneration: () => generation,
		async dispatchIntent(intent) {
			submitted.push(intent);
			if (intent === first) await held;
		},
		async dispatchIntents(intents) {
			submitted.push(...intents);
		}
	});
	const first = edit(1);
	return {
		pipeline,
		submitted,
		first,
		release,
		disconnect() {
			connected = false;
		},
		reconnect() {
			connected = true;
		},
		replace() {
			generation++;
		}
	};
}
describe('intent transport and document admission', () => {
	it('refuses new single and batch edits while transport admission is lost', async () => {
		const f = fixture();
		f.disconnect();
		await expect(f.pipeline.sendIntent(edit(2))).rejects.toThrow('transport');
		await expect(f.pipeline.sendIntents([edit(3)])).rejects.toThrow('transport');
		expect(f.submitted).toEqual([]);
		f.reconnect();
		await f.pipeline.sendIntent(edit(4));
		expect(f.submitted).toEqual([edit(4)]);
	});
	it('cannot drain an already queued edit into a replacement connection', async () => {
		const f = fixture(),
			active = f.pipeline.sendIntent(f.first),
			queued = f.pipeline.sendIntent(edit(2));
		const completed = Promise.allSettled([active, queued]);
		f.disconnect();
		f.release();
		const result = await completed;
		expect(result.map((item) => item.status)).toEqual(['rejected', 'rejected']);
		if (result[1].status === 'rejected') expect(String(result[1].reason)).toContain('transport');
		expect(f.submitted).toEqual([f.first]);
	});
	it('rejects in-flight completion and queued work after document invalidation', async () => {
		const f = fixture(),
			active = f.pipeline.sendIntent(f.first),
			queued = f.pipeline.sendIntent(edit(2));
		const completed = Promise.allSettled([active, queued]);
		f.replace();
		f.release();
		const result = await completed;
		expect(result.map((item) => item.status)).toEqual(['rejected', 'rejected']);
		for (const item of result)
			if (item.status === 'rejected') expect(String(item.reason)).toContain('snapshot');
		expect(f.submitted).toEqual([f.first]);
	});
});
