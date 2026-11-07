// Fresh bundled Picture File authoring; no desktop input or native video claim.
import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { openInitialScreenHeadless } from './initial-screen-host.mjs';
import { verifyPictureFiles } from '../browser/picture-files.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');
const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser, token;
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(6000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	await page.goto(host.base);
	await page.getByRole('tab').first().waitFor();
	await verifyPictureFiles({ page, post, edit, directory: path.join(directory, 'files') });
} catch (error) {
	console.error(host.log().slice(-2000));
	if (browser)
		await browser
			.contexts()[0]
			?.pages()[0]
			?.screenshot({ path: path.join(directory, 'failure.png') })
			.catch(() => {});
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
