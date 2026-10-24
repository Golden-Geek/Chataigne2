// SvelteKit adds the hashes/nonces for the bootstrap it generates. The reusable HTTP host
// also enforces an origin-specific header when these assets are served from an embedded bundle.
export const uiContentSecurityPolicy = {
	mode: /** @type {'auto'} */ ('auto'),
	directives: {
		'default-src': ['self'],
		'script-src': ['self'],
		'style-src': ['self', 'unsafe-inline'],
		'img-src': ['self', 'data:', 'blob:'],
		'font-src': ['self', 'data:'],
		'connect-src': ['self', 'ws:', 'ipc:', 'http://ipc.localhost'],
		'worker-src': ['self', 'blob:'],
		'object-src': ['none'],
		'base-uri': ['none'],
		'form-action': ['none']
	}
};

/**
 * Tighten the development response without replacing SvelteKit's script nonces.
 * @param {string} policy
 * @param {string | URL} location
 * @param {number} backendPort
 */
export function developmentContentSecurityPolicy(policy, location, backendPort = 7010) {
	const frontend = new URL(location);
	if (
		!['http:', 'https:'].includes(frontend.protocol) ||
		!Number.isInteger(backendPort) ||
		backendPort < 1 ||
		backendPort > 65535
	) {
		throw new Error('Development CSP requires an HTTP(S) origin and valid backend port');
	}
	const backend = new URL(frontend.origin);
	backend.port = String(backendPort);
	const frontendSocket = new URL(frontend.origin);
	frontendSocket.protocol = frontend.protocol === 'https:' ? 'wss:' : 'ws:';
	const backendSocket = new URL(backend.origin);
	backendSocket.protocol = backend.protocol === 'https:' ? 'wss:' : 'ws:';
	const directives = policy
		.split(';')
		.map((part) => part.trim())
		.filter((part) => part && !part.startsWith('connect-src '));
	directives.push(
		`connect-src 'self' ${backend.origin} ${backendSocket.origin} ${frontendSocket.origin} ipc: http://ipc.localhost`
	);
	directives.push("frame-ancestors 'none'");
	return directives.join('; ');
}
