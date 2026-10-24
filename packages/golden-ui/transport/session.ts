import type { UiSessionRequest } from '../generated/rust_protocol/UiSessionRequest';
import type { UiSessionResponse } from '../generated/rust_protocol/UiSessionResponse';
import { UI_PROTOCOL_VERSION } from '../generated/rust_protocol/protocol-version.ts';

export interface UiSessionProvider {
	credentials(refresh?: boolean): Promise<UiSessionResponse>;
	invalidate(token: string): void;
}

export const createUiSessionProvider = (options: {
	baseUrl: string;
	fetchImpl?: typeof fetch;
	storage?: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'> | null;
}): UiSessionProvider => {
	const fetchImpl = options.fetchImpl ?? fetch;
	const baseUrl = options.baseUrl.replace(/\/+$/, '');
	const storageKey = `golden-ui-session:${baseUrl}`;
	let storage = options.storage;
	if (storage === undefined) {
		try {
			storage = typeof window === 'undefined' ? null : window.sessionStorage;
		} catch {
			storage = null;
		}
	}
	let previousToken: string | null = null;
	try {
		const stored = storage?.getItem(storageKey);
		if (stored && /^[A-Za-z0-9_-]{43}$/.test(stored)) previousToken = stored;
	} catch {
		/* Storage restrictions do not prevent in-memory sessions. */
	}
	let cached: UiSessionResponse | null = null;
	let expiresAt = 0;
	let revision = 0;
	let pending: Promise<UiSessionResponse> | null = null;

	const invalidate = (token: string): void => {
		if (cached?.token === token || previousToken === token) {
			cached = null;
			previousToken = null;
			expiresAt = 0;
			revision += 1;
			try {
				if (storage?.getItem(storageKey) === token) storage.removeItem(storageKey);
			} catch {
				/* The host still rejects revoked credentials. */
			}
		}
	};

	const bootstrap = async (): Promise<UiSessionResponse> => {
		for (let attempt = 0; attempt < 3; attempt += 1) {
			const currentRevision = revision;
			const previous = cached?.token ?? previousToken;
			const controller = new AbortController();
			const timer = setTimeout(() => controller.abort(), 15000);
			try {
				const request: UiSessionRequest = { capabilities: null };
				const response = await fetchImpl(`${baseUrl}/session`, {
					method: 'POST',
					headers: {
						'content-type': 'application/json',
						...(previous ? { authorization: `Bearer ${previous}` } : {})
					},
					body: JSON.stringify(request),
					cache: 'no-store',
					signal: controller.signal
				});
				if (response.status === 401 && previous) {
					invalidate(previous);
					continue;
				}
				const body: unknown = await response.json();
				if (!response.ok) throw new Error(`Session bootstrap failed (HTTP ${response.status})`);
				if (!validResponse(body)) throw new Error('Invalid session bootstrap response');
				if (body.protocol_version !== UI_PROTOCOL_VERSION) {
					throw new Error(
						`UI protocol mismatch: client=${UI_PROTOCOL_VERSION}, server=${body.protocol_version}`
					);
				}
				if (revision !== currentRevision) continue;
				cached = body;
				previousToken = body.token;
				try {
					storage?.setItem(storageKey, body.token);
				} catch {
					/* Continue with the admitted in-memory credential. */
				}
				expiresAt = Date.now() + body.expires_in_ms;
				return body;
			} finally {
				clearTimeout(timer);
			}
		}
		throw new Error('Session changed repeatedly during bootstrap');
	};

	return {
		invalidate,
		async credentials(refresh = false) {
			if (!refresh && cached && Date.now() < expiresAt) return cached;
			if (pending) return pending;
			const operation = bootstrap();
			pending = operation;
			try {
				return await operation;
			} finally {
				if (pending === operation) pending = null;
			}
		}
	};
};

const validResponse = (value: unknown): value is UiSessionResponse => {
	if (typeof value !== 'object' || value === null) return false;
	const response = value as Partial<UiSessionResponse>;
	const capabilities = response.capabilities;
	return (
		typeof response.token === 'string' &&
		/^[A-Za-z0-9_-]{43}$/.test(response.token) &&
		typeof response.expires_in_ms === 'number' &&
		Number.isInteger(response.expires_in_ms) &&
		response.expires_in_ms > 0 &&
		response.expires_in_ms <= 0xffffffff &&
		typeof response.protocol_version === 'string' &&
		Array.isArray(response.websocket_protocols) &&
		response.websocket_protocols.length === 2 &&
		response.websocket_protocols.every(
			(protocol) => typeof protocol === 'string' && /^[A-Za-z0-9._-]{1,128}$/.test(protocol)
		) &&
		typeof capabilities === 'object' &&
		capabilities !== null &&
		typeof capabilities.observe === 'boolean' &&
		typeof capabilities.author === 'boolean' &&
		typeof capabilities.project_files === 'boolean' &&
		typeof capabilities.scripts === 'boolean'
	);
};
