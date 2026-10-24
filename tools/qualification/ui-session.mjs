import { createUiSessionProvider } from '../../packages/golden-ui/transport/session.ts';

// Qualification tools use the same bootstrap/renewal implementation as browser clients.
const providers = new Map();

export function nativeUiSession(baseUrl) {
	const origin = new URL(baseUrl.replace(/^ws/i, 'http')).origin;
	let provider = providers.get(origin);
	if (!provider) {
		if (providers.size === 16) providers.delete(providers.keys().next().value);
		provider = createUiSessionProvider({ baseUrl: `${origin}/api/ui` });
		providers.set(origin, provider);
	}
	return provider;
}

export async function nativeUiFetch(baseUrl, path, options = {}) {
	if (path === '/api/ui/health' || path === '/.well-known/chataigne') {
		return fetch(`${baseUrl}${path}`, options);
	}
	const provider = nativeUiSession(baseUrl);
	for (let attempt = 0; attempt < 2; attempt += 1) {
		const credential = await provider.credentials(attempt > 0);
		const headers = new Headers(options.headers);
		headers.set('authorization', `Bearer ${credential.token}`);
		const response = await fetch(`${baseUrl}${path}`, { ...options, headers });
		if (response.status !== 401 || attempt === 1) return response;
		provider.invalidate(credential.token);
	}
	throw new Error('Session renewal did not produce a response');
}
