import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPreviewSession, type PreviewSession } from '../session';
import type { PreviewTransport } from '../transport';
import type { PreviewRequest } from '../generated/PreviewRequest';
import type { PreviewResponse } from '../generated/PreviewResponse';

const geometry = (x = 0) => ({
	rect: { x, y: 0, width: 20, height: 10 },
	clip: { x, y: 0, width: 20, height: 10 },
	visible: true
});
function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((done) => {
		resolve = done;
	});
	return { promise, resolve };
}
function transport(override?: (request: PreviewRequest) => Promise<PreviewResponse> | undefined) {
	const requests: PreviewRequest[] = [];
	let connections = 0;
	const value: PreviewTransport = {
		available: () => true,
		async request(request) {
			requests.push(request);
			const response = override?.(request);
			if (response) return response;
			if (request.kind === 'Connect')
				return {
					kind: 'Connected',
					session: `session-${++connections}`,
					version: 1,
					max_views: 64
				};
			if (request.kind === 'Layout' || request.kind === 'Status')
				return {
					kind: 'Applied',
					revision: request.revision,
					visible_views:
						request.kind === 'Layout' ? request.views.filter((view) => view.visible).length : 1,
					installed: true
				};
			return { kind: 'Disconnected' };
		}
	};
	return { value, requests };
}

describe('window preview session', () => {
	const owned: PreviewSession[] = [];
	const own = (value: PreviewTransport) => {
		const session = createPreviewSession(value);
		owned.push(session);
		return session;
	};
	const flush = () => vi.advanceTimersByTimeAsync(0);
	beforeEach(() => vi.useFakeTimers());
	afterEach(async () => {
		for (const session of owned.splice(0)) void session.dispose();
		await flush();
		vi.useRealTimers();
	});

	it('reports browser capability unavailable without sending native requests', async () => {
		const request = vi.fn();
		const session = own({ available: () => false, request });
		expect(await session.connect()).toBe(false);
		expect(session.state.availability).toBe('unavailable');
		expect(session.state.installed).toBe(false);
		expect(request).not.toHaveBeenCalled();
	});

	it.each([
		null,
		{ kind: 'Connected', session: 'invalid-capability', version: 1, max_views: null }
	])(
		'withholds an invalid native control response rather than leaving negotiation pending',
		async (response) => {
			const endpoint = transport((request) =>
				request.kind === 'Connect'
					? Promise.resolve(response as unknown as PreviewResponse)
					: undefined
			);
			const session = own(endpoint.value);
			expect(await session.connect()).toBe(false);
			expect(session.state.availability).toBe('unavailable');
			expect(session.state.installed).toBe(false);
		}
	);

	it('coalesces repeated layout measurements and uses monotonically increasing revisions', async () => {
		const endpoint = transport();
		const session = own(endpoint.value);
		let x = 0;
		const view = session.register('channel', () => geometry(x));
		await session.connect();
		await flush();
		await session.whenSettled();
		const initial = endpoint.requests.filter((request) => request.kind === 'Layout').length;
		for (x = 1; x <= 20; x++) view.invalidate();
		x = 20;
		await flush();
		await session.whenSettled();
		const layouts = endpoint.requests.filter((request) => request.kind === 'Layout');
		expect(layouts).toHaveLength(initial + 1);
		expect(layouts.at(-1)?.views[0].rect.x).toBe(20);
		expect(layouts.map((layout) => layout.revision)).toEqual(layouts.map((_, index) => index + 1));
		expect(session.state.installed).toBe(true);
	});

	it('retains only the newest desired geometry while one IPC is in flight', async () => {
		const blocked = deferred<PreviewResponse>();
		let block = false;
		let outstanding = 0;
		let peak = 0;
		const endpoint = transport((request) => {
			if (request.kind !== 'Layout' || !block) return;
			block = false;
			outstanding++;
			peak = Math.max(peak, outstanding);
			return blocked.promise.finally(() => {
				outstanding--;
			});
		});
		const session = own(endpoint.value);
		let x = 0;
		const view = session.register('channel', () => geometry(x));
		await session.connect();
		await flush();
		await session.whenSettled();
		block = true;
		x = 1;
		view.invalidate();
		await flush();
		const waiting = endpoint.requests.at(-1);
		expect(waiting?.kind).toBe('Layout');
		for (x = 2; x <= 40; x++) view.invalidate();
		x = 40;
		await flush();
		expect(endpoint.requests.at(-1)).toBe(waiting);
		expect(session.state.installed).toBe(false);
		blocked.resolve({
			kind: 'Applied',
			revision: waiting!.kind === 'Layout' ? waiting!.revision : 0,
			visible_views: 1,
			installed: true
		});
		await flush();
		await session.whenSettled();
		const latest = endpoint.requests.at(-1);
		expect(latest?.kind === 'Layout' && latest.views[0].rect.x).toBe(40);
		expect(peak).toBe(1);
		expect(session.state.installed).toBe(true);
	});

	it('does not publish unrelated DOM invalidations when measured native geometry is unchanged', async () => {
		const endpoint = transport();
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		await flush();
		await session.whenSettled();
		const before = endpoint.requests.length;
		for (let count = 0; count < 100; count++) session.invalidate();
		await flush();
		await session.whenSettled();
		expect(endpoint.requests).toHaveLength(before);
		expect(session.state.installed).toBe(true);
	});

	it('covers changed geometry until the native owner confirms installation through Status', async () => {
		let installed = false;
		const endpoint = transport((request) => {
			if (request.kind === 'Layout' || request.kind === 'Status')
				return Promise.resolve({
					kind: 'Applied',
					revision: request.revision,
					visible_views: 1,
					installed
				});
		});
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		await flush();
		expect(session.state.availability).toBe('available');
		expect(session.state.installed).toBe(false);
		installed = true;
		await vi.advanceTimersByTimeAsync(100);
		await session.whenSettled();
		expect(endpoint.requests.at(-1)?.kind).toBe('Status');
		expect(session.state.installed).toBe(true);
	});

	it('reconciles an installation reply across an unrelated queued DOM measurement', async () => {
		const pending = deferred<PreviewResponse>();
		const endpoint = transport((request) =>
			request.kind === 'Layout' ? pending.promise : undefined
		);
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		const layout = endpoint.requests.at(-1);
		expect(layout?.kind).toBe('Layout');
		session.invalidate();
		pending.resolve({
			kind: 'Applied',
			revision: layout!.kind === 'Layout' ? layout!.revision : 0,
			visible_views: 1,
			installed: false
		});
		await flush();
		expect(session.state.installed).toBe(false);
		await vi.advanceTimersByTimeAsync(100);
		await session.whenSettled();
		expect(endpoint.requests.at(-1)?.kind).toBe('Status');
		expect(session.state.installed).toBe(true);
	});

	it('does not let a delayed old connection resurrect a disposed session', async () => {
		const connected = deferred<PreviewResponse>();
		const endpoint = transport((request) =>
			request.kind === 'Connect' ? connected.promise : undefined
		);
		const session = own(endpoint.value);
		const pending = session.connect();
		const disposal = session.dispose();
		connected.resolve({ kind: 'Connected', session: 'late-session', version: 1, max_views: 64 });
		await pending;
		await disposal;
		expect(session.state.availability).toBe('disposed');
		expect(endpoint.requests.at(-1)).toEqual({ kind: 'Disconnect', session: 'late-session' });
	});

	it('disposes through an installed empty layout before disconnecting the native session', async () => {
		let empty = false,
			cleared = false;
		const endpoint = transport((request) => {
			if (request.kind === 'Layout') empty = request.views.length === 0;
			if ((request.kind === 'Layout' || request.kind === 'Status') && empty)
				return Promise.resolve({
					kind: 'Applied',
					revision: request.revision,
					visible_views: 0,
					installed: cleared
				});
		});
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		await flush();
		await session.whenSettled();
		const disposal = session.dispose();
		await flush();
		expect(session.state.installed).toBe(false);
		expect(endpoint.requests.at(-1)?.kind).toBe('Layout');
		expect(endpoint.requests.some((request) => request.kind === 'Disconnect')).toBe(false);
		cleared = true;
		await vi.advanceTimersByTimeAsync(100);
		await disposal;
		expect(endpoint.requests.at(-1)?.kind).toBe('Disconnect');
		expect(session.state.installed).toBe(true);
		expect(session.state.error).toBeNull();
	});

	it('never claims a cleared underlay when disposal installation exhausts its retry bound', async () => {
		let empty = false;
		const endpoint = transport((request) => {
			if (request.kind === 'Layout') empty = request.views.length === 0;
			if ((request.kind === 'Layout' || request.kind === 'Status') && empty)
				return Promise.resolve({
					kind: 'Applied',
					revision: request.revision,
					visible_views: 0,
					installed: false
				});
		});
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		await flush();
		await session.whenSettled();
		const disposal = session.dispose();
		await flush();
		await vi.advanceTimersByTimeAsync(1000);
		await disposal;
		expect(session.state.installed).toBe(false);
		expect(session.state.error).toMatch(/clear could not be confirmed/);
		expect(endpoint.requests.filter((request) => request.kind === 'Status')).toHaveLength(10);
		expect(endpoint.requests.some((request) => request.kind === 'Disconnect')).toBe(false);
	});

	it('reconnects with a new client and ignores an old in-flight layout acknowledgement', async () => {
		const delayed = deferred<PreviewResponse>();
		let block = false;
		const endpoint = transport((request) => {
			if (request.kind === 'Layout' && block) {
				block = false;
				return delayed.promise;
			}
		});
		const session = own(endpoint.value);
		let x = 0;
		const view = session.register('channel', () => geometry(x));
		await session.connect();
		await flush();
		await session.whenSettled();
		block = true;
		x = 10;
		view.invalidate();
		await flush();
		const old = endpoint.requests.at(-1);
		const reconnected = session.reconnect();
		delayed.resolve({
			kind: 'Applied',
			revision: old!.kind === 'Layout' ? old!.revision : 0,
			visible_views: 1,
			installed: true
		});
		await reconnected;
		await flush();
		await session.whenSettled();
		const clients = endpoint.requests.filter((request) => request.kind === 'Connect');
		expect(clients).toHaveLength(2);
		expect(clients[0].client).not.toBe(clients[1].client);
		expect(session.state.session).toBe('session-2');
		expect(session.state.installed).toBe(true);
	});

	it('renews unchanged geometry once per second without covering an installed view', async () => {
		const endpoint = transport();
		const session = own(endpoint.value);
		session.register('channel', () => geometry());
		await session.connect();
		await flush();
		await session.whenSettled();
		const states: boolean[] = [];
		const unsubscribe = session.subscribe((state) => states.push(state.installed));
		const before = endpoint.requests.length;
		await vi.advanceTimersByTimeAsync(1001);
		await session.whenSettled();
		unsubscribe();
		expect(endpoint.requests.length).toBe(before + 1);
		expect(states.every(Boolean)).toBe(true);
	});

	it('unregisters explicitly and bounds view registration before metadata can grow', async () => {
		const endpoint = transport();
		const session = own(endpoint.value);
		const view = session.register('channel', () => geometry());
		await session.connect();
		await flush();
		await session.whenSettled();
		view.unregister();
		await flush();
		await session.whenSettled();
		const latest = endpoint.requests.at(-1);
		expect(latest?.kind === 'Layout' && latest.views).toEqual([]);
		for (let count = 0; count < 64; count++) session.register('channel', () => geometry());
		expect(() => session.register('overflow', () => geometry())).toThrow(/capacity/);
	});

	it('withdraws a timed-out capability without starting parallel replacement IPCs', async () => {
		const delayed = deferred<PreviewResponse>();
		let block = false;
		const endpoint = transport((request) => {
			if (request.kind === 'Layout' && block) {
				block = false;
				return delayed.promise;
			}
		});
		const session = own(endpoint.value);
		let x = 0;
		const view = session.register('channel', () => geometry(x));
		await session.connect();
		await flush();
		await session.whenSettled();
		block = true;
		x = 5;
		view.invalidate();
		await flush();
		const old = endpoint.requests.at(-1);
		const before = endpoint.requests.length;
		await vi.advanceTimersByTimeAsync(1500);
		expect(session.state.availability).toBe('unavailable');
		expect(session.state.installed).toBe(false);
		await vi.advanceTimersByTimeAsync(1000);
		expect(endpoint.requests).toHaveLength(before);
		delayed.resolve({
			kind: 'Applied',
			revision: old!.kind === 'Layout' ? old!.revision : 0,
			visible_views: 1,
			installed: true
		});
		await flush();
		await session.whenSettled();
		expect(session.state.availability).toBe('available');
		expect(session.state.session).toBe('session-2');
	});

	it('truthfully withdraws a rejected capability and retries on a bounded backoff', async () => {
		let failed = true;
		const endpoint = transport((request) =>
			request.kind === 'Connect' && failed
				? Promise.resolve({ kind: 'Rejected', code: 'Closed' })
				: undefined
		);
		const session = own(endpoint.value);
		expect(await session.connect()).toBe(false);
		expect(session.state.availability).toBe('unavailable');
		failed = false;
		await vi.advanceTimersByTimeAsync(1000);
		await session.whenSettled();
		expect(session.state.availability).toBe('available');
		expect(endpoint.requests.filter((request) => request.kind === 'Connect')).toHaveLength(2);
	});
});
