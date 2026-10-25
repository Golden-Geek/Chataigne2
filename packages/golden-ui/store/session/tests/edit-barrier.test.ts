import { describe, expect, it, vi } from 'vitest';
import { createEditOperations } from '../edit-operations';

const deferred = () => {
	let resolve!: () => void, reject!: (error: Error) => void;
	const promise = new Promise<void>((yes, no) => {
		resolve = yes;
		reject = no;
	});
	return { promise, resolve, reject };
};
function fixture() {
	const service = createEditOperations({
		uuid: (id) => `node-${id}`,
		eventTime: () => ({ tick: 0, micro: 0, seq: 0 }),
		changed() {},
		track() {}
	});
	const open = () => {
		const done = deferred();
		const work = { finish: vi.fn(() => done.promise), cancel: vi.fn(() => done.promise) };
		const scope = service.open({ current: () => true, build: () => work })!;
		return { scope, work, done };
	};
	return { service, open };
}
describe('managed edit barriers', () => {
	it('drains confirmed work and cancels active work before entering, with admission closed', async () => {
		const f = fixture(),
			confirmed = f.open(),
			active = f.open();
		const finished = confirmed.scope.confirm();
		const action = vi.fn(async () => 'saved');
		const barrier = f.service.runBarrier(action);
		expect(active.work.cancel).toHaveBeenCalledOnce();
		expect(confirmed.work.cancel).not.toHaveBeenCalled();
		expect(f.service.open({ current: () => true, build: () => active.work })).toBeNull();
		confirmed.done.resolve();
		await finished;
		expect(action).not.toHaveBeenCalled();
		active.done.resolve();
		await expect(barrier).resolves.toBe('saved');
		expect(f.service.size).toBe(0);
	});
	it('keeps admission closed through the asynchronous barrier callback', async () => {
		const f = fixture(),
			entered = deferred(),
			finish = deferred();
		const barrier = f.service.runBarrier(async () => {
			entered.resolve();
			await finish.promise;
		});
		await entered.promise;
		expect(f.open().scope).toBeNull();
		finish.resolve();
		await barrier;
		const next = f.open();
		expect(next.scope).not.toBeNull();
		next.done.resolve();
		await next.scope.cancel();
	});
	it('serializes concurrent callbacks and recovers after a callback failure', async () => {
		const f = fixture(),
			entered = deferred(),
			finish = deferred(),
			events: string[] = [];
		const first = f.service
			.runBarrier(async () => {
				events.push('first');
				entered.resolve();
				await finish.promise;
				throw new Error('save failed');
			})
			.catch((error: Error) => error.message);
		await entered.promise;
		const second = f.service.runBarrier(async () => {
			events.push('second');
			return 42;
		});
		expect(events).toEqual(['first']);
		finish.resolve();
		expect(await first).toBe('save failed');
		expect(await second).toBe(42);
		expect(events).toEqual(['first', 'second']);
		const next = f.open();
		next.done.resolve();
		await next.scope.cancel();
	});
	it('waits for all admitted prefixes after one edit fails and skips the callback', async () => {
		const f = fixture(),
			a = f.open(),
			b = f.open(),
			action = vi.fn(async () => {});
		void a.scope.confirm().catch(() => {});
		let settled = false;
		const barrier = f.service.runBarrier(action).catch((error: Error) => {
			settled = true;
			return error.message;
		});
		a.done.reject(new Error('transport retired'));
		await Promise.resolve();
		await Promise.resolve();
		expect(settled).toBe(false);
		b.done.resolve();
		expect(await barrier).toBe('transport retired');
		expect(action).not.toHaveBeenCalled();
		expect(f.service.size).toBe(0);
	});
	it('does not wait for presentation-only graph publication after confirmed completion', async () => {
		const f = fixture(),
			a = f.open();
		a.scope.publish([{ node: 1, value: { kind: 'float', value: 7 } }]);
		a.scope.applied([
			{ success: true, status: 'applied', latest_event_time: { tick: 20, micro: 0, seq: 0 } }
		]);
		const finish = a.scope.confirm();
		a.done.resolve();
		await finish;
		expect(f.service.size).toBe(1);
		await expect(f.service.runBarrier(async () => 42)).resolves.toBe(42);
		expect(f.service.get(1, 'node-1')).toEqual({ kind: 'float', value: 7 });
		f.service.reset();
	});
	it('rejects a barrier whose session generation retires while work is draining', async () => {
		const f = fixture(),
			a = f.open(),
			action = vi.fn(async () => {});
		void a.scope.confirm();
		const outcome = f.service.runBarrier(action).catch((error: Error) => error.message);
		f.service.reset();
		a.done.resolve();
		expect(await outcome).toContain('session changed');
		expect(action).not.toHaveBeenCalled();
	});
});
