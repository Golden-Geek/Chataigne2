import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

const directory = await mkdtemp(path.resolve('target/m1314-target-picker-ui-'));
const host = await openInitialScreenHeadless(directory, { frontend: true });
let token, browser, page;
let stage = 'seed';
async function post(route, value) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(value),
		signal: AbortSignal.timeout(10000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function operation(namespace, payload) {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
const edit = (payload) => operation('mapgyver.media', payload);
const screens = (payload) => operation('mapgyver.screens', payload);
async function wait(read, predicate) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const result = await read();
		if (predicate(result)) return result;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(result)}`);
		await delay(20);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { folder } = await edit({ kind: 'CreateFolder', parent: null, label: 'Picker sources' });
	const { reference: red } = await edit({
		kind: 'CreateMediaInFolder',
		folder,
		media_kind: 'SolidColor',
		label: 'Catalog color'
	});
	await edit({ kind: 'RenameChannel', reference: red, label: 'Red' });
	let blue;
	for (let index = 1; index <= 70; index++) {
		const result = await edit({
			kind: 'AddChannel',
			media: red.media,
			label: index === 70 ? 'Blue' : `Variant ${index}`
		});
		if (index === 70) blue = result.reference;
	}
	const { media } = await edit({
		kind: 'CreateReference',
		target: null,
		label: 'Picker reference'
	});
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Catalog composition'
	});
	await edit({
		kind: 'Composition',
		reference: composition,
		edit: {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Forbidden nested media',
			position: { x: 0, y: 0 }
		}
	});
	await edit({ kind: 'CreateScript', label: 'Forbidden script' });
	const input = async () =>
		(await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 1 } })).page.channels[0]
			.definition.source.target;
	const { screen, surface } = await screens({ kind: 'CreateScreen', label: 'Picker screen' });
	await screens({ kind: 'RenameSurface', reference: surface, label: 'Picker surface' });
	const surfaceSource = async () =>
		(await screens({ kind: 'InspectManager' })).snapshot.screens
			.find((entry) => entry.id === screen)
			.surfaces.find((entry) => entry.reference.surface === surface.surface).definition.source;
	browser = await chromium.launch({ headless: true });
	page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(8000);
	const errors = [],
		fullTreeRequests = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('request', (request) => {
		if (request.url().endsWith('/api/ui/reference-targets')) fullTreeRequests.push(request.url());
	});
	const rejected = observeRejectedRequests(page, () => stage);
	await page.goto(host.base);
	const tab = (name) =>
		page.getByRole('tab').filter({ hasText: new RegExp(`^${RegExp.escape(name)}$`) });
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const trigger = (label) => parameter(label).locator('button.choice-trigger');
	const item = (name) =>
		page.getByRole('menuitem', { name: new RegExp(`^${RegExp.escape(name)}(?:\\s|$)`) });
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	const selectReference = async () => {
		await tab('Medias').click();
		await page
			.locator('section[aria-label="Medias"]')
			.getByRole('button', { name: 'Picker reference', exact: true })
			.click();
		await trigger('Media').waitFor();
	};
	await selectReference();
	stage = 'catalog-only folder browse';
	await trigger('Media').click();
	await item('Picker sources').waitFor();
	assert.equal(await item('Forbidden script').count(), 0);
	assert.equal(await item('Forbidden nested media').count(), 0);
	assert.equal(await item('Catalog color').count(), 0);
	assert.equal(await item('Red').count(), 0);
	await page.screenshot({ path: path.join(directory, 'catalog.png') });
	await item('Picker sources').click();
	await item('Catalog color').click();
	await wait(input, (value) => value?.channel === red.channel);
	await wait(
		() => trigger('Channel').innerText(),
		(value) => value.includes('Red')
	);
	stage = 'one-step Media and default Channel history';
	await undo();
	await wait(input, (value) => value === null);
	await redo();
	await wait(input, (value) => value?.channel === red.channel);
	stage = 'paged dependent Channel dropdown';
	await trigger('Channel').click();
	await item('Next page').waitFor();
	assert.equal(await item('Picker sources').count(), 0);
	assert.equal(await item('Blue').count(), 0);
	assert((await page.getByRole('menuitem').count()) <= 70);
	await item('Next page').click();
	await item('Blue').waitFor();
	await page.screenshot({ path: path.join(directory, 'channels.png') });
	await item('Blue').click();
	await wait(input, (value) => value?.channel === blue.channel);
	await edit({ kind: 'RenameChannel', reference: blue, label: 'Azure' });
	await wait(
		() => trigger('Channel').innerText(),
		(value) => value.includes('Azure')
	);
	await edit({ kind: 'ReorderChannel', reference: blue, after: null });
	assert.deepEqual(await input(), blue);
	stage = 'deleted explicit Channel and recovery';
	await edit({ kind: 'RemoveChannel', reference: blue });
	assert.deepEqual(await input(), blue);
	await wait(
		() => trigger('Channel').innerText(),
		(value) => value.includes('unavailable')
	);
	await undo();
	await wait(
		() => trigger('Channel').innerText(),
		(value) => value.includes('Azure') && !value.includes('unavailable')
	);
	assert.deepEqual(await input(), blue);
	stage = 'Surface uses the same controls and Reference-owned output';
	await tab('Screens').click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: 'Picker surface', exact: true })
		.click();
	await trigger('Source media').click();
	await item('Picker reference').click();
	const source = await wait(surfaceSource, (value) => value?.media === media);
	assert.notEqual(source.channel, blue.channel);
	await wait(
		() => trigger('Source channel').innerText(),
		(value) => value.includes('Output')
	);
	await trigger('Source channel').click();
	await item('None').click();
	await wait(surfaceSource, (value) => value === null);
	await wait(
		() => trigger('Source media').innerText(),
		(value) => value.includes('Picker reference')
	);
	await trigger('Source channel').click();
	await item('Output').click();
	await wait(surfaceSource, (value) => value?.channel === source.channel);
	await trigger('Source media').click();
	await item('None').click();
	await wait(surfaceSource, (value) => value === null);
	await undo();
	await wait(surfaceSource, (value) => value?.channel === source.channel);
	await redo();
	await wait(surfaceSource, (value) => value === null);
	stage = 'New/Load and exact selected identity';
	const project = path.join(directory, 'picker.mapgyver');
	await post('/api/ui/project-save', { path: project });
	await post('/api/ui/project-new', {});
	await post('/api/ui/project-load', { path: project });
	assert.deepEqual(await input(), blue);
	await selectReference();
	await wait(
		() => trigger('Channel').innerText(),
		(value) => value.includes('Azure')
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	assert.deepEqual(fullTreeRequests, []);
	const result = {
		result: 'pass',
		groups: 7,
		scope:
			'bundled catalog-only folder browsing, paged Channel choice, backend defaults, history, rename/reorder, explicit missing/recovery, shared Surface controls, independent Reference output and project replacement',
		directory
	};
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} catch (error) {
	if (page) await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(path.join(directory, 'failure.txt'), `${stage}\n${error.stack}\n${host.log()}`);
	throw error;
} finally {
	await browser?.close();
	await host.close();
}
