import { describe, expect, it, vi } from 'vitest';
import { createEditSession } from '../edit-session';

const deferred = <T>() => {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((complete) => {
		resolve = complete;
	});
	return { promise, resolve };
};
function setup() {
	let host = { hasActiveEditSession: false, editSessionEpoch: 0, snapshotGeneration: 0 };
	const begin = deferred<boolean>(),
		end = deferred<void>();
	const open = vi.fn(async (opened: typeof host) => {
		const result = await begin.promise;
		if (result) {
			opened.hasActiveEditSession = true;
			opened.editSessionEpoch++;
		}
		return result;
	});
	const close = vi.fn(async (closed: typeof host) => {
		await end.promise;
		closed.hasActiveEditSession = false;
	});
	return {
		session: createEditSession({ host: () => host, begin: open, end: close }),
		begin,
		end,
		open,
		close,
		host: () => host,
		replace: () => {
			host = { hasActiveEditSession: true, editSessionEpoch: 1, snapshotGeneration: 0 };
		}
	};
}
describe('UI edit-session acknowledgement ownership', () => {
	it('coalesces begins and closes a begin acknowledged after end was requested', async () => {
		const s = setup(),
			first = s.session.begin(),
			second = s.session.begin(),
			closing = s.session.end();
		expect(s.open).toHaveBeenCalledTimes(1);
		expect(s.close).not.toHaveBeenCalled();
		s.begin.resolve(true);
		await first;
		await second;
		await Promise.resolve();
		expect(s.close).toHaveBeenCalledTimes(1);
		s.end.resolve();
		await closing;
		expect(s.session.active).toBe(false);
	});
	it('coalesces ends and permits a later begin only after the close completes', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		const a = s.session.end(),
			b = s.session.end(),
			next = s.session.begin();
		await Promise.resolve();
		expect(s.open).toHaveBeenCalledTimes(1);
		s.end.resolve();
		await Promise.all([a, b, next]);
		expect(s.close).toHaveBeenCalledTimes(1);
		expect(s.open).toHaveBeenCalledTimes(2);
		expect(s.session.active).toBe(true);
	});
	it('does not close a replacement host with the same epoch', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		s.replace();
		await s.session.end();
		expect(s.close).not.toHaveBeenCalled();
		expect(s.session.active).toBe(false);
	});
	it('cancels a queued reopen when the next interaction also ends before close acknowledges', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		const closing = s.session.end(),
			reopening = s.session.begin(),
			finalEnd = s.session.end();
		s.end.resolve();
		await Promise.all([closing, reopening, finalEnd]);
		expect(s.open).toHaveBeenCalledTimes(1);
		expect(s.close).toHaveBeenCalledTimes(1);
		expect(s.session.active).toBe(false);
	});
	it('does not move a queued reopen to a replacement host', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		const closing = s.session.end(),
			reopening = s.session.begin();
		s.replace();
		s.end.resolve();
		await Promise.all([closing, reopening]);
		expect(s.open).toHaveBeenCalledTimes(1);
		expect(s.session.active).toBe(false);
	});
	it('closes a queued reopen that has started but has not acknowledged when end arrives', async () => {
		const s = setup(),
			nextBegin = deferred<boolean>();
		s.begin.resolve(true);
		await s.session.begin();
		s.open.mockImplementationOnce(async (host) => {
			const result = await nextBegin.promise;
			if (result) {
				host.hasActiveEditSession = true;
				host.editSessionEpoch++;
			}
			return result;
		});
		const closing = s.session.end(),
			reopening = s.session.begin();
		s.end.resolve();
		await closing;
		await Promise.resolve();
		expect(s.open).toHaveBeenCalledTimes(2);
		const finalEnd = s.session.end();
		nextBegin.resolve(true);
		await Promise.all([reopening, finalEnd]);
		expect(s.close).toHaveBeenCalledTimes(2);
		expect(s.session.active).toBe(false);
	});
	it('does not acquire a replacement host when the old begin acknowledges late', async () => {
		const s = setup(),
			begin = s.session.begin(),
			end = s.session.end();
		s.replace();
		s.begin.resolve(true);
		await Promise.all([begin, end]);
		expect(s.close).not.toHaveBeenCalled();
		expect(s.session.active).toBe(false);
	});
	it('does not move a queued reopen into a replaced project on the same host', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		const closing = s.session.end(),
			reopening = s.session.begin();
		s.host().snapshotGeneration++;
		s.end.resolve();
		await Promise.all([closing, reopening]);
		expect(s.open).toHaveBeenCalledTimes(1);
		expect(s.session.active).toBe(false);
	});
	it('does not acquire a late begin in a replaced project on the same host', async () => {
		const s = setup(),
			begin = s.session.begin(),
			end = s.session.end();
		s.host().snapshotGeneration++;
		s.begin.resolve(true);
		await Promise.all([begin, end]);
		expect(s.close).not.toHaveBeenCalled();
		expect(s.session.active).toBe(false);
	});
	it('does not close a later foreign epoch on the same host', async () => {
		const s = setup();
		s.begin.resolve(true);
		await s.session.begin();
		s.host().editSessionEpoch++;
		await s.session.end();
		expect(s.close).not.toHaveBeenCalled();
	});
	it('does not acquire an existing edit session or close a rejected begin', async () => {
		const s = setup();
		s.host().hasActiveEditSession = true;
		await s.session.begin();
		expect(s.open).not.toHaveBeenCalled();
		s.host().hasActiveEditSession = false;
		const begin = s.session.begin(),
			end = s.session.end();
		s.begin.resolve(false);
		await Promise.all([begin, end]);
		expect(s.close).not.toHaveBeenCalled();
	});
});
