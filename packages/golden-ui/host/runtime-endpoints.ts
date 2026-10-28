import type { UiApplicationInfoDto } from '../generated/rust_protocol/UiApplicationInfoDto';
import type { UiDiscoveryDto } from '../generated/rust_protocol/UiDiscoveryDto';
import { UI_PROTOCOL_VERSION } from '../generated/rust_protocol/protocol-version';
import { validateApplicationInfo } from './application';

export interface RuntimeLocation {
	readonly origin: string;
}

export interface RuntimeEndpointOptions {
	readonly development: boolean;
	readonly developmentBackendPort?: number;
}

export interface RuntimeEndpoints {
	readonly httpBaseUrl: string;
	readonly wsUrl: string;
}

export const resolveRuntimeEndpoints = (
	location: RuntimeLocation,
	{ development, developmentBackendPort = 7010 }: RuntimeEndpointOptions
): RuntimeEndpoints => {
	const origin = new URL(location.origin);
	if (development) {
		if (
			!Number.isInteger(developmentBackendPort) ||
			developmentBackendPort < 1 ||
			developmentBackendPort > 65535
		) {
			throw new Error('Development backend port must be an integer from 1 through 65535');
		}
		origin.port = String(developmentBackendPort);
	}
	const websocketOrigin = new URL(origin);
	websocketOrigin.protocol = origin.protocol === 'https:' ? 'wss:' : 'ws:';
	return {
		httpBaseUrl: new URL('/api/ui', origin).toString().replace(/\/$/, ''),
		wsUrl: new URL('/api/ui/ws', websocketOrigin).toString()
	};
};

export const discoverUiApplication = async (
	httpBaseUrl: string,
	options: {
		expectedApplicationId: string;
		fetchImpl?: typeof fetch;
		signal?: AbortSignal;
	}
): Promise<UiApplicationInfoDto> => {
	if (options.expectedApplicationId.trim().length === 0) {
		throw new Error('An expected application identity is required');
	}
	const controller = new AbortController();
	const cancel = (): void => controller.abort();
	options.signal?.addEventListener('abort', cancel, { once: true });
	if (options.signal?.aborted) cancel();
	const timer = setTimeout(cancel, 15000);
	try {
		const response = await (options.fetchImpl ?? fetch)(
			new URL('/.well-known/golden-ui', httpBaseUrl),
			{
				cache: 'no-store',
				signal: controller.signal
			}
		);
		if (!response.ok) throw new Error(`Runtime discovery failed (HTTP ${response.status})`);
		const value: unknown = await response.json();
		if (typeof value !== 'object' || value === null)
			throw new Error('Invalid runtime discovery response');
		const discovery = value as Partial<UiDiscoveryDto>;
		if (
			discovery.version !== 1 ||
			discovery.service !== 'golden-ui' ||
			discovery.relative_endpoints !== true ||
			discovery.health_path !== '/api/ui/health' ||
			discovery.websocket_path !== '/api/ui/ws'
		) {
			throw new Error('Invalid Golden UI discovery contract');
		}
		if (discovery.protocol_version !== UI_PROTOCOL_VERSION) {
			throw new Error(
				`UI protocol mismatch: client=${UI_PROTOCOL_VERSION}, server=${discovery.protocol_version}`
			);
		}
		const application = validateApplicationInfo(discovery.application);
		if (application.id !== options.expectedApplicationId) {
			throw new Error(
				`Expected application ${options.expectedApplicationId}; runtime is ${application.id}`
			);
		}
		return application;
	} finally {
		clearTimeout(timer);
		options.signal?.removeEventListener('abort', cancel);
	}
};
