// Finite public-authoring controls for one owned native-alignment fixture.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { chromium } from 'playwright-core';

const directory = path.resolve(process.argv[2]);
const action = process.argv[3];
assert(['setup', 'red', 'blue', 'dialog', 'dialog-state', 'capture'].includes(action));
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
let token;
async function post(route, value) {
	const response = await fetch(ready.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(value),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
}
async function edit(namespace, payload) {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
token = (await post('/api/ui/session', {})).token;
if (action === 'red' || action === 'blue') {
	const surface = await edit('screens', { kind: 'InspectSurface', reference: ready.reference });
	assert(surface.snapshot.definition.source);
	await edit('media', {
		kind: 'ConfigureChannel',
		reference: surface.snapshot.definition.source,
		source: { kind: 'SolidColor', rgba: action === 'red' ? [1, 0, 0, 1] : [0, 0, 1, 1] }
	});
	await delay(500);
	console.log(JSON.stringify({ action, reference: surface.snapshot.definition.source }));
} else if (action === 'capture') {
	const stage = process.argv[4];
	assert(/^[a-z][a-z0-9-]{0,40}$/.test(stage));
	const hwnd = execFileSync(
		'powershell.exe',
		['-NoProfile', '-Command', `(Get-Process -Id ${ready.pid}).MainWindowHandle.ToInt64()`],
		{ encoding: 'utf8', windowsHide: true }
	).trim();
	const args = [
		'apps/mapgyver/tests/native/window_capture.py',
		'--visible',
		'--hwnd',
		hwnd,
		'--pid',
		String(ready.pid)
	];
	if (process.argv[5]) {
		assert(/^[1-9]\d*$/.test(process.argv[5]));
		args.push('--foreground-hwnd', process.argv[5]);
	}
	args.push(path.join(directory, stage));
	try {
		console.log(execFileSync('python', args, { encoding: 'utf8', windowsHide: true }));
	} catch (error) {
		if (error.status !== 2) throw error;
		console.log(error.stdout.toString());
	}
} else {
	const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
	const pages = browser
		.contexts()
		.flatMap((context) => context.pages())
		.filter((page) => page.url().startsWith(ready.base + '/'));
	assert.equal(pages.length, 1);
	const page = pages[0];
	page.setDefaultTimeout(5000);
	if (action === 'dialog-state') {
		console.log(JSON.stringify(await page.evaluate(() => window.__opaqueDialogResult)));
	} else if (action === 'dialog') {
		await page.evaluate(
			(suggested) => {
				window.__opaqueDialogResult = { pending: true };
				void window.__TAURI_INTERNALS__
					.invoke('save_file_dialog', {
						suggested_path: suggested,
						allowed_extensions: ['mapgyver'],
						title: 'MapGyver owned layer check'
					})
					.then((result) => {
						window.__opaqueDialogResult = { pending: false, result };
					})
					.catch((error) => {
						window.__opaqueDialogResult = { pending: false, error: String(error) };
					});
			},
			path.join(directory, 'never-written.mapgyver')
		);
	} else {
		const sourceTab = page.getByRole('tab').filter({ hasText: /^Surface Source:/ });
		const inspectorTab = page.getByRole('tab').filter({ hasText: /^Inspector:/ });
		const group = inspectorTab.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		const box = await group.boundingBox();
		assert(box);
		await sourceTab.dragTo(group, { targetPosition: { x: box.width / 2, y: box.height / 2 } });
		for (const name of ['Screen view', 'Surface source view'])
			await page.getByRole('application', { name, exact: true }).waitFor({ state: 'visible' });
		await page.waitForFunction(
			() =>
				[...document.querySelectorAll('.native-preview')].filter(
					(e) =>
						e.getBoundingClientRect().width > 0 &&
						e.getBoundingClientRect().height > 0 &&
						e.dataset.previewInstalled === 'true'
				).length === 2
		);
		const geometry = await page.locator('.native-preview').evaluateAll((elements) => ({
			dpr: devicePixelRatio,
			views: elements
				.filter((e) => e.getBoundingClientRect().width > 0 && e.getBoundingClientRect().height > 0)
				.map((e) => ({
					label: e.getAttribute('aria-label'),
					rect: JSON.parse(JSON.stringify(e.getBoundingClientRect()))
				}))
		}));
		assert.equal(geometry.views.length, 2);
		assert(
			Math.max(...geometry.views.map((v) => v.rect.width)) >
				Math.min(...geometry.views.map((v) => v.rect.width)) * 1.25
		);
		await writeFile(path.join(directory, 'layers-ready.json'), JSON.stringify(geometry, null, 2));
		console.log(JSON.stringify(geometry));
	}
}
process.exit(0);
