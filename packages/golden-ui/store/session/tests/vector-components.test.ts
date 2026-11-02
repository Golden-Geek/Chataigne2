import { describe, expect, it } from 'vitest';
import { createWorkbenchIntentPipeline } from '../intent-pipeline';
import type { UiEditIntent } from '../../../types';

const component = (
	axis: number,
	value: number,
	behaviour: 'Coalesce' | 'Append' = 'Coalesce'
): UiEditIntent => ({
	kind: 'setParamComponent',
	node: 7,
	component: axis,
	value,
	behaviour
});
const vector = (x: number, y: number): UiEditIntent => ({
	kind: 'setParam',
	node: 7,
	value: { kind: 'vec2', value: [x, y] },
	behaviour: 'Coalesce'
});

function fixture() {
	const submitted: UiEditIntent[] = [];
	const pipeline = createWorkbenchIntentPipeline({
		getSnapshotGeneration: () => 1,
		async dispatchIntent(intent) {
			submitted.push(intent);
		},
		async dispatchIntents(intents) {
			submitted.push(...intents);
		}
	});
	return { submitted, pipeline };
}

describe('vector component intent ordering', () => {
	it('coalesces only adjacent samples of the same axis and resolves every caller', async () => {
		const { submitted, pipeline } = fixture();
		const resume = pipeline.suspendDispatch();
		const requests = [component(0, 0.1), component(0, 0.2), component(1, 0.3), component(0, 0.4)];
		const pending = requests.map((intent) => pipeline.sendIntent(intent));
		resume();
		await Promise.all(pending);
		expect(submitted).toEqual(requests.slice(1));
	});

	it('preserves whole-vector boundaries on both sides of a component write', async () => {
		const { submitted, pipeline } = fixture();
		const resume = pipeline.suspendDispatch();
		const requests = [vector(0.1, 0.2), component(0, 0.3), vector(0.4, 0.5), component(1, 0.6)];
		const pending = requests.map((intent) => pipeline.sendIntent(intent));
		resume();
		await Promise.all(pending);
		expect(submitted).toEqual(requests);
	});

	it('retains explicit Append occurrences and pending return-to-original edits', async () => {
		const { submitted, pipeline } = fixture();
		const resume = pipeline.suspendDispatch();
		const requests = [
			component(0, 0.1, 'Append'),
			component(0, 0.1, 'Append'),
			component(1, 0.5),
			component(1, 0)
		];
		const pending = requests.map((intent) => pipeline.sendIntent(intent));
		resume();
		await Promise.all(pending);
		expect(submitted).toEqual([requests[0], requests[1], requests[3]]);
	});
});
