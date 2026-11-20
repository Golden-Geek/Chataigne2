// Owned loopback responses following the documented Shadertoy.com API shape.
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { createServer } from 'node:http';

export const SHADERTOY_FIXTURE = {
	key: 'FixtureKey',
	invalidKey: 'FixtureRejected',
	query: 'fixture buffer',
	title: 'Buffered green fixture',
	id: 'ST0001'
};

export async function startShaderToyFixture() {
	const requests = [];
	const ids = Array.from({ length: 9 }, (_, index) => `ST${String(index + 1).padStart(4, '0')}`);
	const server = createServer((request, response) => {
		const url = new URL(request.url, 'http://127.0.0.1');
		const route = decodeURIComponent(url.pathname);
		const key = url.searchParams.get('key');
		const from = Number(url.searchParams.get('from'));
		const count = Number(url.searchParams.get('num'));
		const authenticated = key === SHADERTOY_FIXTURE.key;
		requests.push({ method: request.method, route, from, count, authenticated });
		function send(status, payload) {
			response.writeHead(status, { 'Content-Type': 'application/json' });
			response.end(JSON.stringify(payload));
		}
		if (request.method !== 'GET') return send(405, { Error: 'Method not allowed' });
		if (!authenticated) return send(200, { Error: `Invalid key ${key}` });
		const prefix = '/api/v1/shaders';
		if (route === `${prefix}/query` || route === `${prefix}/query/${SHADERTOY_FIXTURE.query}`) {
			return send(200, { Results: ids.length, Shaders: ids.slice(from, from + count) });
		}
		if (route.startsWith(`${prefix}/query/`)) return send(200, { Results: 0, Shaders: [] });
		const id = route.slice(prefix.length + 1);
		if (!route.startsWith(`${prefix}/`) || !ids.includes(id))
			return send(404, { Error: 'Shader unavailable' });
		return send(200, {
			Shader: {
				info: {
					id,
					name: id === SHADERTOY_FIXTURE.id ? SHADERTOY_FIXTURE.title : `Shader ${id}`,
					username: 'MapGyver Fixture',
					published: 3
				},
				renderpass: [
					{
						name: 'Common',
						type: 'common',
						code: '// SPDX-License-Identifier: CC0-1.0\nvec4 green() { return vec4(0.0, 1.0, 0.0, 1.0); }'
					},
					{
						name: 'Buffer A',
						type: 'buffer',
						outputs: [{ id: 7, channel: 0 }],
						code: 'void mainImage(out vec4 c, in vec2 p) { c = green(); }'
					},
					{
						name: 'Image',
						type: 'image',
						inputs: [{ id: 7, channel: 0, ctype: 'buffer' }],
						code: 'void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel0, p / iResolution.xy); }'
					}
				]
			}
		});
	});
	server.listen(0, '127.0.0.1');
	await once(server, 'listening');
	const port = server.address().port;
	assert(port > 0);
	return { port, requests, close: () => new Promise((resolve) => server.close(resolve)) };
}
