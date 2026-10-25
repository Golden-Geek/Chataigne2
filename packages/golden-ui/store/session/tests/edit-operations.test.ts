import { describe, expect, it, vi } from 'vitest';
import { createEditOperations } from '../edit-operations';
import type { UiAck, UiEditIntent } from '../../../types';

const deferred = () => {
	let resolve!: () => void;
	const promise = new Promise<void>((done) => {
		resolve = done;
	});
	return { promise, resolve };
};
const time = (tick: number) => ({ tick, micro: 0, seq: 0 });
const edit = (node: number, value: number): Extract<UiEditIntent, { kind: 'setParam' }> => ({
	kind: 'setParam',
	node,
	behaviour: 'Coalesce',
	value: { kind: 'float', value }
});
function fixture(limit = 64) {
	let at = time(0);
	const service = createEditOperations({
		uuid: (id) => `node-${id}`,
		changed: () => {},
		track: () => {},
		limit,
		eventTime: () => at
	});
	const open = () => {
		let current = true;
		const done = deferred(),
			work = {
				accepted: true,
				finish: vi.fn(() => done.promise),
				cancel: vi.fn(() => done.promise)
			};
		const scope = service.open({ current: () => current, build: () => work });
		return {
			scope,
			work,
			done,
			invalidate: () => {
				current = false;
			}
		};
	};
	return {
		service,
		open,
		advance: (tick: number) => {
			at = time(tick);
			service.observe();
		}
	};
}
describe('session-owned UI edit operations', () => {
	it('retains confirmed work and drafts when its view detaches', async () => {
		const f = fixture(),
			s = f.open();
		s.scope!.publish([edit(1, 8)]);
		const closed = s.scope!.confirm();
		s.scope!.detach();
		expect(s.work.cancel).not.toHaveBeenCalled();
		expect(f.service.get(1, 'node-1')).toEqual({ kind: 'float', value: 8 });
		s.done.resolve();
		await closed;
		expect(f.service.size).toBe(0);
		expect(f.service.get(1, 'node-1')).toBeUndefined();
	});
	it('cancels an unconfirmed view once and holds admission until its prefix settles', async () => {
		const f = fixture(),
			a = f.open(),
			b = f.open();
		let admitted = false;
		void b.scope!.ready.then(() => {
			admitted = true;
		});
		a.scope!.publish([edit(1, 8)]);
		a.scope!.detach();
		a.scope!.detach();
		await Promise.resolve();
		expect(a.work.cancel).toHaveBeenCalledTimes(1);
		expect(admitted).toBe(false);
		expect(f.service.get(1, 'node-1')).toBeUndefined();
		a.done.resolve();
		await b.scope!.ready;
		expect(admitted).toBe(true);
		b.scope!.cancel();
		b.done.resolve();
	});
	it('serializes separate widgets through completion of the preceding history group', async () => {
		const f = fixture(),
			a = f.open(),
			b = f.open();
		await a.scope!.ready;
		let next = false;
		void b.scope!.ready.then(() => {
			next = true;
		});
		const closed = a.scope!.confirm();
		await Promise.resolve();
		expect(next).toBe(false);
		a.done.resolve();
		await closed;
		await b.scope!.ready;
		expect(next).toBe(true);
		b.scope!.cancel();
		b.done.resolve();
	});
	it('keeps only the latest valid draft layer and restores the preceding owner on cancellation', async () => {
		const f = fixture(),
			a = f.open(),
			b = f.open();
		a.scope!.publish([edit(1, 8)]);
		b.scope!.publish([edit(1, 9), edit(2, 5)]);
		expect(f.service.get(1, 'node-1')).toEqual({ kind: 'float', value: 9 });
		expect(f.service.get(1, 'replacement')).toBeUndefined();
		const cancelled = b.scope!.cancel();
		expect(f.service.get(1, 'node-1')).toEqual({ kind: 'float', value: 8 });
		expect(f.service.get(2, 'node-2')).toBeUndefined();
		b.done.resolve();
		await cancelled;
		a.scope!.cancel();
		a.done.resolve();
	});
	it('keeps drafts through a late applied-event boundary without holding subsequent history admission', async () => {
		const f = fixture(),
			a = f.open();
		a.scope!.publish([edit(1, 8)]);
		a.scope!.applied([
			{ success: true, status: 'applied', latest_event_time: time(20) } satisfies UiAck
		]);
		const closed = a.scope!.confirm();
		a.done.resolve();
		await closed;
		expect(f.service.size).toBe(1);
		const b = f.open();
		await b.scope!.ready;
		expect(f.service.get(1, 'node-1')).toEqual({ kind: 'float', value: 8 });
		f.advance(19);
		expect(f.service.get(1, 'node-1')).toBeDefined();
		f.advance(20);
		expect(f.service.get(1, 'node-1')).toBeUndefined();
		b.scope!.cancel();
		b.done.resolve();
	});
	it('drops rejected final drafts even when an earlier write has a pending event boundary', async () => {
		const f = fixture(),
			a = f.open();
		a.scope!.publish([edit(1, 8)]);
		a.scope!.applied([{ success: true, status: 'applied', latest_event_time: time(20) }]);
		a.work.accepted = false;
		const closed = a.scope!.confirm();
		a.done.resolve();
		await closed;
		expect(f.service.size).toBe(0);
	});
	it('invalidates queued and confirmed work on reset without leaving draft layers behind', async () => {
		const f = fixture(),
			a = f.open(),
			b = f.open();
		a.scope!.publish([edit(1, 8)]);
		a.scope!.confirm();
		b.scope!.publish([edit(2, 9)]);
		f.service.reset();
		expect(a.scope!.current).toBe(false);
		expect(b.scope!.current).toBe(false);
		expect(f.service.values.size).toBe(0);
		expect(a.work.cancel).toHaveBeenCalledTimes(1);
		a.done.resolve();
		b.done.resolve();
		await Promise.all([a.scope!.cancel(), b.scope!.cancel()]);
		expect(f.service.size).toBe(0);
	});
	it('uses a fixed admission limit and recovers capacity after completion', async () => {
		const f = fixture(2),
			a = f.open(),
			b = f.open();
		expect(f.open().scope).toBeNull();
		const closed = a.scope!.confirm();
		a.done.resolve();
		await closed;
		const c = f.open();
		expect(c.scope).not.toBeNull();
		b.scope!.cancel();
		c.scope!.cancel();
		b.done.resolve();
		c.done.resolve();
	});
	it('retires invalid source projections using publication observation', async () => {
		const f = fixture(),
			a = f.open();
		a.scope!.publish([edit(1, 8)]);
		a.invalidate();
		f.service.observe();
		expect(f.service.get(1, 'node-1')).toBeUndefined();
		expect(a.work.cancel).toHaveBeenCalledTimes(1);
		a.done.resolve();
	});
});
