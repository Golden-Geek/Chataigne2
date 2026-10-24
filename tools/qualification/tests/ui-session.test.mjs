import { createServer } from 'node:http';
import { once } from 'node:events';
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { nativeUiFetch, nativeUiSession } from '../ui-session.mjs';
import { UI_PROTOCOL_VERSION } from '../../../packages/golden-ui/generated/rust_protocol/protocol-version.ts';

test('native qualification clients renew after a host restart and do not replay uncertain mutations', async () => {
	let token = 'a'.repeat(43);
	let mutations = 0;
	let applied = 0;
	let bootstrap = 0;
	let unavailable = false;
	const server = createServer(async (request, response) => {
		assert.equal(request.headers.origin, undefined);
		assert.ok(!request.url.includes(token));
		let body = '';
		for await (const chunk of request) body += chunk;
		response.setHeader('content-type', 'application/json');
		if (request.url === '/api/ui/session') {
			bootstrap += 1;
			assert.deepEqual(JSON.parse(body), { capabilities: null });
			response.end(
				JSON.stringify({
					token,
					capabilities: { observe: true, author: true, project_files: true, scripts: true },
					expires_in_ms: 3600000,
					protocol_version: UI_PROTOCOL_VERSION,
					websocket_protocols: ['golden-ui', `gc-session.${token}`]
				})
			);
			return;
		}
		mutations += 1;
		assert.equal(body, '{"operation":"save"}');
		if (unavailable) {
			response.writeHead(503).end('{"error":"unavailable"}');
		} else if (request.headers.authorization !== `Bearer ${token}`) {
			response.writeHead(401).end('{"error":"stale session"}');
		} else {
			applied += 1;
			response.end('{"ok":true}');
		}
	});
	server.listen(0, '127.0.0.1');
	await once(server, 'listening');
	try {
		const base = `http://127.0.0.1:${server.address().port}`;
		const first = await nativeUiSession(base).credentials();
		assert.equal(first.token, token);
		const options = {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: '{"operation":"save"}'
		};
		assert.equal((await nativeUiFetch(base, '/api/ui/project-save', options)).status, 200);
		token = 'b'.repeat(43);
		assert.equal((await nativeUiFetch(base, '/api/ui/project-save', options)).status, 200);
		assert.equal(bootstrap, 2);
		assert.equal(mutations, 3);
		assert.equal(applied, 2);
		unavailable = true;
		assert.equal((await nativeUiFetch(base, '/api/ui/project-save', options)).status, 503);
		assert.equal(mutations, 4);
		assert.equal(applied, 2);
	} finally {
		server.closeAllConnections();
		await new Promise((resolve) => server.close(resolve));
	}
});
