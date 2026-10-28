import { afterEach, describe, expect, it, vi } from 'vitest';
import { createUiSessionProvider } from '../session';
import { getUiClientInstanceId } from '../client-instance';
import { UI_PROTOCOL_VERSION } from '../../generated/rust_protocol/protocol-version';
import { createWebSocketUiClient } from '../ws';

const storage = () => {
	const entries = new Map<string, string>();
	return {
		entries,
		getItem: vi.fn((key: string) => entries.get(key) ?? null),
		setItem: vi.fn((key: string, value: string) => {
			entries.set(key, value);
		}),
		removeItem: vi.fn((key: string) => {
			entries.delete(key);
		})
	};
};
const credential = {
	token: 'a'.repeat(43),
	expires_in_ms: 60000,
	protocol_version: UI_PROTOCOL_VERSION,
	websocket_protocols: ['golden-ui', `gc-session.${'a'.repeat(43)}`],
	capabilities: {
		observe: true,
		author: true,
		project_files: true,
		scripts: true
	}
};

const discovery = (identity: string) => ({
	version: 1,
	service: 'golden-ui',
	health_path: '/api/ui/health',
	websocket_path: '/api/ui/ws',
	relative_endpoints: true,
	protocol_version: UI_PROTOCOL_VERSION,
	application: { id: identity, display_name: identity, version: '1', data_directory_name: identity }
});

afterEach(() => vi.unstubAllGlobals());

describe('application transport storage', () => {
	it('separates credentials by application and endpoint while retaining same-app reloads', async () => {
		const tab = storage();
		let hostedApplication = 'app/one';
		const fetchImpl = vi
			.fn<typeof fetch>()
			.mockImplementation(async (input) =>
				Response.json(
					String(input).endsWith('/.well-known/golden-ui')
						? discovery(hostedApplication)
						: credential
				)
			);
		for (const [applicationId, baseUrl] of [
			['app/one', 'http://localhost:7010/api/ui'],
			['app/one', 'http://localhost:7010/api/ui'],
			['app:two', 'http://localhost:7010/api/ui'],
			['app/one', 'http://localhost:7011/api/ui']
		]) {
			hostedApplication = applicationId;
			await createUiSessionProvider({
				applicationId,
				baseUrl,
				fetchImpl,
				storage: tab
			}).credentials();
		}
		const authorization = fetchImpl.mock.calls
			.filter((call) => call[1]?.method === 'POST')
			.map((call) => new Headers(call[1]?.headers).get('authorization'));
		expect(authorization).toEqual([null, `Bearer ${credential.token}`, null, null]);
		expect(tab.entries.size).toBe(3);
		expect([...tab.entries.keys()]).toContain(
			'golden-ui-session:app%2Fone:http://localhost:7010/api/ui'
		);
	});

	it('rejects an endpoint that restarts as another app before credentials, snapshots or storage writes', async () => {
		const tab = storage();
		let hostedApplication = 'chataigne';
		const credentialRequests = vi.fn();
		const fetchImpl = vi.fn<typeof fetch>(async (input) => {
			if (String(input).endsWith('/.well-known/golden-ui')) {
				return Response.json(discovery(hostedApplication));
			}
			credentialRequests();
			return Response.json(credential);
		});
		const session = createUiSessionProvider({
			applicationId: 'chataigne',
			baseUrl: 'http://localhost:7010/api/ui',
			fetchImpl,
			storage: tab
		});
		await session.credentials();
		const socket = vi.fn();
		const client = createWebSocketUiClient({
			applicationId: 'chataigne',
			httpBaseUrl: 'http://localhost:7010/api/ui',
			sessionProvider: session,
			fetchImpl,
			webSocketImpl: socket as unknown as typeof WebSocket
		});
		tab.setItem.mockClear();
		hostedApplication = 'mapgyver';
		await expect(session.credentials(true)).rejects.toThrow('Expected application chataigne');
		await expect(client.snapshot()).rejects.toThrow('Expected application chataigne');
		await expect(session.credentials()).rejects.toThrow('Expected application chataigne');
		expect(credentialRequests).toHaveBeenCalledTimes(1);
		expect(socket).not.toHaveBeenCalled();
		expect(tab.setItem).not.toHaveBeenCalled();
		expect(tab.removeItem).not.toHaveBeenCalled();
		hostedApplication = 'chataigne';
		await expect(session.credentials(true)).resolves.toMatchObject({ token: credential.token });
		expect(credentialRequests).toHaveBeenCalledTimes(2);
		expect(tab.setItem).toHaveBeenCalledOnce();
	});

	it('revalidates the same application before normal credential renewal', async () => {
		const events: string[] = [];
		const session = createUiSessionProvider({
			applicationId: 'chataigne',
			baseUrl: 'http://localhost:7010/api/ui',
			storage: null,
			fetchImpl: async (input) => {
				const identityCheck = String(input).endsWith('/.well-known/golden-ui');
				events.push(identityCheck ? 'discovery' : 'credentials');
				return Response.json(identityCheck ? discovery('chataigne') : credential);
			}
		});
		await session.credentials();
		await session.credentials(true);
		expect(events).toEqual(['discovery', 'credentials', 'discovery', 'credentials']);
	});

	it('keeps anonymous low-level clients in memory without touching persistent credentials', async () => {
		const tab = storage();
		const session = createUiSessionProvider({
			baseUrl: '/api/ui',
			storage: tab,
			fetchImpl: async () => Response.json(credential)
		});
		expect((await session.credentials()).token).toBe(credential.token);
		session.invalidate(credential.token);
		expect(tab.getItem).not.toHaveBeenCalled();
		expect(tab.setItem).not.toHaveBeenCalled();
		expect(tab.removeItem).not.toHaveBeenCalled();
	});

	it('uses separate stable per-tab client identities for each app', () => {
		const tab = storage();
		vi.stubGlobal('window', { sessionStorage: tab });
		const first = getUiClientInstanceId('identity-test/one');
		const second = getUiClientInstanceId('identity-test:two');
		expect(first).not.toBe(second);
		expect(getUiClientInstanceId('identity-test/one')).toBe(first);
		expect(tab.entries.get('golden_ui.identity-test%2Fone.client_instance.v2')).toBe(first);
		expect(tab.entries.get('golden_ui.identity-test%3Atwo.client_instance.v2')).toBe(second);
	});
});
