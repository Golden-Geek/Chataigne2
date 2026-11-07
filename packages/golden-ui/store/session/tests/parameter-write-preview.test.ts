import { describe, expect, it } from 'vitest';
import { createEditOperations } from '../edit-operations';
import { previewParameterWrite, parameterComponentPreview } from '../parameter-write-preview';
import type { AppliedIntentObserver } from '../intent-sender';
import type { GraphState } from '../../graph.svelte';
import type { EventTime } from '../../../types';

function deferred() {
	let resolve!: (value: boolean) => void;
	const promise = new Promise<boolean>((yes) => {
		resolve = yes;
	});
	return { promise, resolve };
}
function fixture(limit = 64) {
	let at: EventTime = { tick: 1, micro: 0, seq: 0 };
	const operations = createEditOperations({
		uuid: (id) => (id === 2 ? 'position' : undefined),
		changed() {},
		track() {},
		eventTime: () => at,
		limit
	});
	const graph = {
		nodesById: new Map([[2, { uuid: 'position' }]]),
		paramsById: new Map([[2, { value: { kind: 'vec2', value: [10, 20] } }]])
	} as unknown as GraphState;
	const host = {
		status: 'connected',
		snapshotGeneration: 1,
		loading: { visible: false },
		graph: { state: graph },
		editOperations: operations
	};
	return {
		host,
		advance: () => {
			at = { tick: 5, micro: 0, seq: 0 };
			operations.observe();
		}
	};
}

describe('ordinary Parameter write presentation', () => {
	it('publishes before dispatch completes and retains the draft through its actual event boundary', async () => {
		const { host, advance } = fixture(),
			pending = deferred();
		let applied: AppliedIntentObserver | undefined;
		const request = previewParameterWrite(
			host,
			2,
			{ kind: 'vec2', value: [30, 40] },
			(observer) => {
				applied = observer;
				return pending.promise;
			}
		);
		expect(host.editOperations.get(2, 'position')).toEqual({ kind: 'vec2', value: [30, 40] });
		expect(host.graph.state.paramsById.get(2)?.value).toEqual({ kind: 'vec2', value: [10, 20] });
		applied?.([
			{ success: true, status: 'applied', latest_event_time: { tick: 5, micro: 0, seq: 0 } }
		]);
		pending.resolve(true);
		expect(await request).toBe(true);
		expect(host.editOperations.get(2, 'position')).toBeDefined();
		advance();
		expect(host.editOperations.get(2, 'position')).toBeUndefined();
	});
	it('composes presentation axes while rejection restores the preceding live layer', async () => {
		const { host } = fixture(),
			pending = deferred();
		const first = previewParameterWrite(
			host,
			2,
			parameterComponentPreview(host, 2, 0, 30),
			() => pending.promise
		);
		expect(parameterComponentPreview(host, 2, 1, 90)).toEqual({ kind: 'vec2', value: [30, 90] });
		expect(
			await previewParameterWrite(
				host,
				2,
				parameterComponentPreview(host, 2, 1, 90),
				async () => false
			)
		).toBe(false);
		expect(host.editOperations.get(2, 'position')).toEqual({ kind: 'vec2', value: [30, 20] });
		pending.resolve(true);
		await first;
		expect(host.editOperations.size).toBe(0);
	});
	it('never revives a stale document draft after a late applied receipt', async () => {
		const { host } = fixture(),
			pending = deferred();
		let applied: AppliedIntentObserver | undefined;
		const request = previewParameterWrite(host, 2, { kind: 'float', value: 30 }, (observer) => {
			applied = observer;
			return pending.promise;
		});
		host.snapshotGeneration++;
		host.editOperations.observe();
		applied?.([
			{ success: true, status: 'applied', latest_event_time: { tick: 5, micro: 0, seq: 0 } }
		]);
		pending.resolve(true);
		await request;
		expect(host.editOperations.get(2, 'position')).toBeUndefined();
	});
	it('bounds preview ownership without dropping or reordering the ordinary dispatch', async () => {
		const { host } = fixture(1),
			pending = deferred();
		const first = previewParameterWrite(
			host,
			2,
			{ kind: 'float', value: 30 },
			() => pending.promise
		);
		let dispatched = false;
		expect(
			await previewParameterWrite(host, 2, { kind: 'float', value: 40 }, async () => {
				dispatched = true;
				return true;
			})
		).toBe(true);
		expect(dispatched).toBe(true);
		expect(host.editOperations.size).toBe(1);
		pending.resolve(true);
		await first;
		expect(host.editOperations.size).toBe(0);
	});
	it('cleans up a throwing dispatch and does not project invalid vector component indices', async () => {
		const { host } = fixture();
		expect(parameterComponentPreview(host, 2, 2, 30)).toBeUndefined();
		await expect(
			previewParameterWrite(host, 2, { kind: 'float', value: 30 }, () => {
				throw new Error('closed');
			})
		).rejects.toThrow('closed');
		expect(host.editOperations.size).toBe(0);
	});
});
