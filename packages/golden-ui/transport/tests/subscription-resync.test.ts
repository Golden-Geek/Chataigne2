import { describe, it, expect, vi } from 'vitest';
import {
	createSubscriptionResyncCoordinator,
	type SubscriptionResyncState
} from '../subscription-resync';
import type { EventTime } from '../../types';
const deferred = () => {
	let resolve!: (time: EventTime) => void;
	const promise = new Promise<EventTime>((done) => (resolve = done));
	return { promise, resolve };
};
const drain = async () => {
	for (let index = 0; index < 12; index++) await Promise.resolve();
};
function fixture() {
	const first = deferred(),
		second = deferred();
	const request = vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
	const subscribe = vi.fn();
	const state: SubscriptionResyncState = {
		scope: { kind: 'wholeGraph' },
		closed: false,
		stagedFrames: { stage() {}, stageEnvelope() {}, reset() {} },
		subscribedSocketGeneration: 1,
		resyncing: false,
		resyncGeneration: 0,
		resyncPlane: undefined,
		resyncReason: '',
		resyncRetryTimer: null
	};
	const coordinator = createSubscriptionResyncCoordinator({
		retryMs: 1,
		getSocketGeneration: () => 1,
		isSocketGenerationOpen: () => true,
		requestSnapshot: request,
		sendSubscribe: subscribe,
		sendUnsubscribe: vi.fn()
	});
	return { first, second, request, subscribe, state, coordinator };
}
describe('subscription replacement during snapshot resync', () => {
	it('coalesces later invalidations into one new snapshot before resubscribing', async () => {
		const f = fixture();
		f.coordinator.begin('graph', f.state, undefined, 'first project');
		await drain();
		for (let index = 0; index < 64; index++)
			f.coordinator.begin('graph', f.state, undefined, `new project ${index}`);
		expect(f.request).toHaveBeenCalledTimes(1);
		f.first.resolve({ tick: 10, micro: 0, seq: 0 });
		await drain();
		expect(f.request).toHaveBeenCalledTimes(2);
		expect(f.subscribe).not.toHaveBeenCalled();
		expect(f.request.mock.calls[1][2]).toBe('new project 63');
		f.second.resolve({ tick: 1, micro: 0, seq: 0 });
		await drain();
		expect(f.subscribe).toHaveBeenCalledTimes(1);
		expect(f.state.cursor).toEqual({ tick: 1, micro: 0, seq: 0 });
	});
	it('retires a queued invalidation when the subscription is disposed', async () => {
		const f = fixture();
		f.coordinator.begin('graph', f.state, undefined, 'first');
		await drain();
		f.coordinator.begin('graph', f.state, undefined, 'replacement');
		f.coordinator.dispose(f.state);
		f.first.resolve({ tick: 10, micro: 0, seq: 0 });
		await drain();
		expect(f.request).toHaveBeenCalledTimes(1);
		expect(f.subscribe).not.toHaveBeenCalled();
	});
});
