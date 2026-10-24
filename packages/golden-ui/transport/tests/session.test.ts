import { afterEach, describe, expect, it, vi } from 'vitest';
import { createUiSessionProvider } from '../session';
import { createHttpAuxiliaryClient } from '../http';
import { createWebSocketUiClient } from '../ws';
import { UI_PROTOCOL_VERSION } from '../../generated/rust_protocol/protocol-version';
import type { UiSessionResponse } from '../../generated/rust_protocol/UiSessionResponse';
import { testSessionProvider } from './fixtures/session';

const credential = (letter = 'a', lifetime = 3600000): UiSessionResponse => ({
	token: letter.repeat(43),
	capabilities: {
		observe: true,
		author: true,
		project_files: true,
		scripts: true
	},
	expires_in_ms: lifetime,
	protocol_version: UI_PROTOCOL_VERSION,
	websocket_protocols: ['golden-ui', `gc-session.${letter.repeat(43)}`]
});

afterEach(() => {
	vi.useRealTimers();
});

describe('host session credentials', () => {
	it('reuses a tab credential after reload and validates it again before use', async () => {
		const entries = new Map<string, string>();
		const storage = {
			getItem: (key: string) => entries.get(key) ?? null,
			setItem: (key: string, value: string) => {
				entries.set(key, value);
			},
			removeItem: (key: string) => {
				entries.delete(key);
			}
		};
		const fetchImpl = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json(credential()))
			.mockResolvedValueOnce(Response.json(credential()))
			.mockResolvedValueOnce(new Response(null, { status: 401 }))
			.mockResolvedValueOnce(Response.json(credential('b')));
		const options = { baseUrl: 'http://localhost:7010/api/ui', fetchImpl, storage };
		await createUiSessionProvider(options).credentials();
		await createUiSessionProvider(options).credentials();
		expect(new Headers(fetchImpl.mock.calls[1][1]?.headers).get('authorization')).toBe(
			`Bearer ${credential().token}`
		);
		const restarted = await createUiSessionProvider(options).credentials();
		expect(restarted.token).toBe(credential('b').token);
		expect(new Headers(fetchImpl.mock.calls[3][1]?.headers).has('authorization')).toBe(false);
		expect([...entries.values()]).toEqual([credential('b').token]);
	});

	it('continues with an admitted memory session when tab storage is unavailable', async () => {
		const denied = () => {
			throw new Error('storage denied');
		};
		const session = createUiSessionProvider({
			baseUrl: '/api/ui',
			fetchImpl: async () => Response.json(credential()),
			storage: { getItem: denied, setItem: denied, removeItem: denied }
		});
		expect((await session.credentials()).token).toBe(credential().token);
	});

	it('shares one bootstrap across concurrent callers and renews the same credential on reconnect', async () => {
		const fetchImpl = vi.fn(async () => Response.json(credential()));
		const session = createUiSessionProvider({
			baseUrl: 'http://localhost:7010/api/ui/',
			fetchImpl
		});
		const results = await Promise.all([
			session.credentials(),
			session.credentials(),
			session.credentials()
		]);
		expect(fetchImpl).toHaveBeenCalledTimes(1);
		expect(results.map((value) => value.token)).toEqual(Array(3).fill(credential().token));
		await session.credentials();
		expect(fetchImpl).toHaveBeenCalledTimes(1);
		await session.credentials(true);
		expect(fetchImpl).toHaveBeenCalledTimes(2);
		const call = fetchImpl.mock.calls[1] as unknown as [string, RequestInit];
		expect(call[0]).toBe('http://localhost:7010/api/ui/session');
		expect(new Headers(call[1].headers).get('authorization')).toBe(`Bearer ${credential().token}`);
	});

	it('replaces a stale host credential after restart without creating an anonymous mutation', async () => {
		const fetchImpl = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json(credential()))
			.mockResolvedValueOnce(new Response(null, { status: 401 }))
			.mockResolvedValueOnce(Response.json(credential('b')));
		const session = createUiSessionProvider({
			baseUrl: 'http://localhost:7010/api/ui',
			fetchImpl
		});
		await session.credentials();
		expect((await session.credentials(true)).token).toBe(credential('b').token);
		expect(new Headers(fetchImpl.mock.calls[2][1]?.headers).has('authorization')).toBe(false);
		session.invalidate(credential().token);
		expect((await session.credentials()).token).toBe(credential('b').token);
		expect(fetchImpl).toHaveBeenCalledTimes(3);
	});

	it('cannot republish an in-flight renewal invalidated by a later rejection', async () => {
		let release!: (response: Response) => void;
		const renewal = new Promise<Response>((resolve) => {
			release = resolve;
		});
		const fetchImpl = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json(credential()))
			.mockReturnValueOnce(renewal)
			.mockResolvedValueOnce(Response.json(credential('b')));
		const session = createUiSessionProvider({ baseUrl: '/api/ui', fetchImpl });
		await session.credentials();
		const pending = session.credentials(true);
		session.invalidate(credential().token);
		release(Response.json(credential()));
		expect((await pending).token).toBe(credential('b').token);
		expect((await session.credentials()).token).toBe(credential('b').token);
	});

	it('renews expired credentials and rejects incompatible protocol metadata', async () => {
		vi.useFakeTimers();
		const fetchImpl = vi
			.fn<typeof fetch>()
			.mockResolvedValueOnce(Response.json(credential('a', 10)))
			.mockResolvedValueOnce(Response.json(credential('a', 10)))
			.mockResolvedValueOnce(Response.json({ ...credential(), protocol_version: 'unknown' }));
		const session = createUiSessionProvider({ baseUrl: '/api/ui', fetchImpl });
		await session.credentials();
		vi.setSystemTime(Date.now() + 11);
		await session.credentials();
		expect(fetchImpl).toHaveBeenCalledTimes(2);
		await expect(session.credentials(true)).rejects.toThrow('protocol mismatch');
	});

	it('does not attempt a broader or anonymous grant after a scope denial', async () => {
		const fetchImpl = vi.fn(async () => Response.json({ error: 'denied' }, { status: 403 }));
		await expect(
			createUiSessionProvider({ baseUrl: '/api/ui', fetchImpl }).credentials()
		).rejects.toThrow('HTTP 403');
		expect(fetchImpl).toHaveBeenCalledTimes(1);
	});
});

describe('authenticated transports', () => {
	it('keeps bootstrap and auxiliary HTTP calls on an explicitly selected LAN socket host', async () => {
		class Socket {
			static OPEN = 1;
			readyState = 0;
		}
		const urls: string[] = [];
		const fetchImpl: typeof fetch = async (url) => {
			urls.push(String(url));
			return Response.json(String(url).endsWith('/session') ? credential() : { ok: true });
		};
		const client = createWebSocketUiClient({
			wsUrl: 'ws://192.168.20.44:7041/api/ui/ws',
			fetchImpl,
			webSocketImpl: Socket as unknown as typeof WebSocket
		});
		await client.projectNew();
		expect(urls).toContain('http://192.168.20.44:7041/api/ui/session');
		expect(urls).toContain('http://192.168.20.44:7041/api/ui/project-new');
		expect(urls.every((url) => url.startsWith('http://192.168.20.44:7041/'))).toBe(true);
	});

	it('retries only a pre-dispatch HTTP 401 and preserves the mutation payload', async () => {
		const calls: Array<{
			url: string;
			authorization: string | null;
			body: BodyInit | null | undefined;
		}> = [];
		let bootstrap = 0;
		let mutation = 0;
		const fetchImpl: typeof fetch = async (url, init) => {
			calls.push({
				url: String(url),
				authorization: new Headers(init?.headers).get('authorization'),
				body: init?.body
			});
			if (String(url).endsWith('/session'))
				return Response.json(credential(bootstrap++ === 0 ? 'a' : 'b'));
			return ++mutation === 1
				? Response.json({ error: 'expired' }, { status: 401 })
				: Response.json({ ok: true });
		};
		await createHttpAuxiliaryClient({
			baseUrl: 'http://localhost:7010/api/ui',
			fetchImpl
		}).projectNew();
		const mutations = calls.filter((call) => call.url.endsWith('/project-new'));
		expect(mutations).toHaveLength(2);
		expect(mutations[0].body).toBe(mutations[1].body);
		expect(mutations.map((call) => call.authorization)).toEqual([
			`Bearer ${credential().token}`,
			`Bearer ${credential('b').token}`
		]);
	});

	it.each([403, 503, 'network'])('does not replay a mutation after %s', async (failure) => {
		const fetchImpl = vi.fn<typeof fetch>(async () => {
			if (failure === 'network') throw new Error('outcome is unknown');
			return Response.json({ error: 'failed' }, { status: failure });
		});
		const client = createHttpAuxiliaryClient({
			baseUrl: '/api/ui',
			fetchImpl,
			sessionProvider: testSessionProvider
		});
		await expect(client.projectNew()).rejects.toThrow();
		expect(fetchImpl).toHaveBeenCalledTimes(1);
	});

	it.each([true, false])(
		'bootstraps one socket and binds authoring identity only when granted (%s)',
		async (author) => {
			const offers: string[][] = [];
			class Socket {
				static OPEN = 1;
				static instances: Socket[] = [];
				readyState = 0;
				onopen: (() => void) | null = null;
				sent: string[] = [];
				constructor(_url: string, protocols: string[]) {
					offers.push(protocols);
					Socket.instances.push(this);
				}
				send(message: string) {
					this.sent.push(message);
				}
			}
			const response = credential();
			response.capabilities.author = author;
			const fetchImpl = vi.fn(async () => Response.json(response));
			const client = createWebSocketUiClient({
				httpBaseUrl: 'http://localhost:7010/api/ui',
				fetchImpl,
				webSocketImpl: Socket as unknown as typeof WebSocket
			});
			client.subscribe({ kind: 'wholeGraph' }, null, () => {});
			client.subscribe({ kind: 'wholeGraph' }, null, () => {});
			expect(offers).toHaveLength(0);
			await vi.waitFor(() => expect(offers).toHaveLength(1));
			expect(offers[0]).toEqual(credential().websocket_protocols);
			expect(fetchImpl).toHaveBeenCalledTimes(1);
			const socket = Socket.instances[0];
			socket.readyState = Socket.OPEN;
			socket.onopen?.();
			const messages = socket.sent.map((message) => JSON.parse(message));
			const hello = messages.find((message) => message.kind === 'hello');
			expect(hello.client_instance_id === null).toBe(!author);
			expect(messages.filter((message) => message.kind === 'subscribe')).toHaveLength(2);
		}
	);

	it('cannot send an expired intent when a delayed bootstrap finally opens the socket', async () => {
		vi.useFakeTimers();
		let release!: (value: UiSessionResponse) => void;
		const pending = new Promise<UiSessionResponse>((resolve) => {
			release = resolve;
		});
		class Socket {
			static OPEN = 1;
			static instances: Socket[] = [];
			readyState = 0;
			onopen: (() => void) | null = null;
			sent: string[] = [];
			constructor() {
				Socket.instances.push(this);
			}
			send(message: string) {
				this.sent.push(message);
			}
		}
		const client = createWebSocketUiClient({
			webSocketImpl: Socket as unknown as typeof WebSocket,
			sessionProvider: { credentials: () => pending, invalidate() {} }
		});
		const intent = client.sendIntent({ kind: 'reevaluateGraph' });
		const rejected = expect(intent).rejects.toThrow('send timeout');
		await vi.advanceTimersByTimeAsync(4001);
		await rejected;
		expect(Socket.instances).toHaveLength(0);
		release(credential());
		await vi.advanceTimersByTimeAsync(0);
		const socket = Socket.instances[0];
		expect(socket).toBeDefined();
		socket.readyState = Socket.OPEN;
		socket.onopen?.();
		await vi.advanceTimersByTimeAsync(0);
		expect(socket.sent.map((message) => JSON.parse(message).kind)).not.toContain('intent');
	});
});
