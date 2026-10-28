import { afterEach, describe, expect, it, vi } from 'vitest';
import { discoverUiApplication, resolveRuntimeEndpoints } from '../runtime-endpoints';
import { UI_PROTOCOL_VERSION } from '../../generated/rust_protocol/protocol-version';
import type { UiApplicationInfoDto } from '../../generated/rust_protocol/UiApplicationInfoDto';

const application: UiApplicationInfoDto = {
	id: 'mapgyver',
	display_name: 'MapGyver',
	version: '0.1.0',
	data_directory_name: 'MapGyver'
};
const document = () => ({
	version: 1,
	service: 'golden-ui',
	health_path: '/api/ui/health',
	websocket_path: '/api/ui/ws',
	relative_endpoints: true,
	protocol_version: UI_PROTOCOL_VERSION,
	application: { ...application }
});
const response = (value: unknown) => vi.fn<typeof fetch>().mockResolvedValue(Response.json(value));

afterEach(() => vi.useRealTimers());

describe('application host discovery', () => {
	it('discovers only the selected backend origin and returns host-owned identity', async () => {
		const fetchImpl = response(document());
		const endpoints = resolveRuntimeEndpoints(
			{ origin: 'http://192.168.1.20:5174' },
			{
				development: true,
				developmentBackendPort: 7011
			}
		);
		expect(
			await discoverUiApplication(endpoints.httpBaseUrl, {
				expectedApplicationId: 'mapgyver',
				fetchImpl
			})
		).toEqual(application);
		expect(String(fetchImpl.mock.calls[0][0])).toBe(
			'http://192.168.1.20:7011/.well-known/golden-ui'
		);
		expect(fetchImpl.mock.calls[0][1]?.cache).toBe('no-store');
		expect(fetchImpl.mock.calls[0][1]?.signal).toBeInstanceOf(AbortSignal);
	});

	it('rejects the wrong app even when its endpoint and protocol otherwise match', async () => {
		await expect(
			discoverUiApplication('http://localhost:7010/api/ui', {
				expectedApplicationId: 'chataigne',
				fetchImpl: response(document())
			})
		).rejects.toThrow('Expected application chataigne');
	});

	it('accepts nonempty identities without inventing an identifier format', async () => {
		const value = document();
		value.application.id = 'org.example/Map Gyver:β';
		expect(
			(
				await discoverUiApplication('https://app.example/api/ui', {
					expectedApplicationId: value.application.id,
					fetchImpl: response(value)
				})
			).id
		).toBe(value.application.id);
	});

	it('requires complete identity metadata and the generated protocol version', async () => {
		for (const field of ['id', 'display_name', 'version', 'data_directory_name'] as const) {
			for (const invalid of ['', ' ', null, 42]) {
				const value = document();
				Object.assign(value.application, { [field]: invalid });
				await expect(
					discoverUiApplication('http://localhost:7011/api/ui', {
						expectedApplicationId: 'mapgyver',
						fetchImpl: response(value)
					})
				).rejects.toThrow('must be a nonempty string');
			}
		}
		await expect(
			discoverUiApplication('http://localhost:7011/api/ui', {
				expectedApplicationId: 'mapgyver',
				fetchImpl: response({ ...document(), protocol_version: 'old' })
			})
		).rejects.toThrow('UI protocol mismatch');
	});

	it('rejects invalid generic service contracts and unsuccessful HTTP responses', async () => {
		for (const changed of [
			{ service: 'chataigne' },
			{ version: '1' },
			{ relative_endpoints: false },
			{ health_path: 'https://another-host/health' },
			{ websocket_path: '/other/ws' }
		]) {
			await expect(
				discoverUiApplication('http://localhost:7011/api/ui', {
					expectedApplicationId: 'mapgyver',
					fetchImpl: response({ ...document(), ...changed })
				})
			).rejects.toThrow('Invalid Golden UI discovery contract');
		}
		await expect(
			discoverUiApplication('http://localhost:7011/api/ui', {
				expectedApplicationId: 'mapgyver',
				fetchImpl: vi.fn().mockResolvedValue(new Response(null, { status: 503 }))
			})
		).rejects.toThrow('HTTP 503');
	});

	it('cancels an outstanding discovery when its owning route is destroyed', async () => {
		const controller = new AbortController();
		const fetchImpl = vi.fn<typeof fetch>(
			(_url, init) =>
				new Promise((_resolve, reject) => {
					init?.signal?.addEventListener('abort', () => reject(new Error('request cancelled')), {
						once: true
					});
				})
		);
		const pending = discoverUiApplication('http://localhost:7011/api/ui', {
			expectedApplicationId: 'mapgyver',
			fetchImpl,
			signal: controller.signal
		});
		const rejected = expect(pending).rejects.toThrow('request cancelled');
		controller.abort();
		await rejected;
	});

	it('bounds an unavailable request and cleans up its timeout after success', async () => {
		vi.useFakeTimers();
		await discoverUiApplication('http://localhost:7011/api/ui', {
			expectedApplicationId: 'mapgyver',
			fetchImpl: response(document())
		});
		expect(vi.getTimerCount()).toBe(0);
		const fetchImpl = vi.fn<typeof fetch>(
			(_url, init) =>
				new Promise((_resolve, reject) => {
					init?.signal?.addEventListener('abort', () => reject(new Error('request timed out')), {
						once: true
					});
				})
		);
		const pending = discoverUiApplication('http://localhost:7011/api/ui', {
			expectedApplicationId: 'mapgyver',
			fetchImpl
		});
		const rejected = expect(pending).rejects.toThrow('request timed out');
		await vi.advanceTimersByTimeAsync(15000);
		await rejected;
		expect(vi.getTimerCount()).toBe(0);
	});
});
