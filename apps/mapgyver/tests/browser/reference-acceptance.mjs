// Run retained Reference browser scopes in the ordinary isolated bundled headless app.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { verifyUnavailableReferenceUi } from './unavailable-reference.mjs';
import { verifyMatrixOutputExposure } from './matrix-output-exposure.mjs';
import { verifyReferenceUi } from './reference.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = await mkdtemp(path.resolve('target/m1314-reference-acceptance-'));
const host = await openInitialScreenHeadless(directory, { frontend: true });
let token, browser;
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(10000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(payload) {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	await post('/api/ui/project-new', {});
	await verifyReferenceUi({
		base: host.base,
		edit,
		inspectMedia: async () => (await edit({ kind: 'InspectManager' })).snapshot
	});
	await verifyUnavailableReferenceUi({
		base: host.base,
		edit,
		post,
		directory,
		inspectMedia: async () => (await edit({ kind: 'InspectManager' })).snapshot
	});
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	await page.goto(host.base);
	await verifyMatrixOutputExposure({ page, edit, post });
	await page.screenshot({ path: path.join(directory, 'exposure.png') });
	const result = { result: 'pass', groups: 3, directory };
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} finally {
	if (browser) await browser.close();
	await host.close();
}
