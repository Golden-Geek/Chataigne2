// Same selected native scene, without continuous desktop readback. No presented-FPS claim.
import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
import { access, writeFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
const directory = process.argv[2];
const browser = await chromium.connectOverCDP(process.env.MAPGYVER_CONTROL_CDP);
const page = browser.contexts().flatMap((c) => c.pages())[0];
const base = new URL(page.url()).origin;
assert.equal(new URL(base).hostname, '127.0.0.1', 'owned local test host required');
let token;
async function post(route, body) {
	const response = await fetch(base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
token = (await post('/api/ui/session', {})).token;
const source = JSON.parse(
	await page.locator('.media-preview-panel .native-preview').getAttribute('data-preview-source')
);
const reference = source.reference;
assert.equal(source.kind, 'Channel');
async function edit(payload) {
	const r = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(r.success, JSON.stringify(r));
	return r.result.value;
}
const transport = (edit) => ({ kind: 'Video', media: reference.media, edit });
await writeFile(`${directory}/uncaptured-ready.json`, JSON.stringify({ base, reference }));
const deadline = Date.now() + 8 * 60 * 1000;
while (
	!(await access(`${directory}/uncaptured-start`).then(
		() => true,
		() => false
	))
) {
	assert(Date.now() < deadline, 'owned control start deadline');
	await delay(50);
}
const phases = [];
for (const playing of [false, true]) {
	await edit(transport({ kind: playing ? 'Play' : 'Pause' }));
	let sampling = true;
	const samples = [];
	let error;
	const observed = (async () => {
		while (sampling) {
			const r = await edit({ kind: 'InspectVideoFrame', reference });
			samples.push(r.frame);
			await delay(5);
		}
	})().catch((e) => {
		error = e;
	});
	const start = performance.now();
	for (let i = 0; i < 120; i++) {
		await delay(Math.max(0, start + i * 25 - performance.now()));
		await edit(
			transport({ kind: 'Seek', position_ms: (((((i + 1) * 37) % 180) + 0.25) * 1000) / 24 })
		);
	}
	sampling = false;
	await observed;
	if (error) throw error;
	phases.push({
		playing,
		elapsed_ms: performance.now() - start,
		samples,
		distinct_non_null_pts: new Set(samples.filter(Boolean).map((f) => f.position_ms)).size
	});
}
await edit(transport({ kind: 'Pause' }));
await edit(transport({ kind: 'Seek', position_ms: 2000 }));
await writeFile(`${directory}/uncaptured.json`, JSON.stringify({ phases }, null, 2));
await browser.close();
console.log(
	JSON.stringify(
		phases.map((p) => ({
			playing: p.playing,
			elapsed_ms: p.elapsed_ms,
			distinct: p.distinct_non_null_pts
		}))
	)
);
