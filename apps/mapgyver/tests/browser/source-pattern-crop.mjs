// Source preview stays whole; Screen Warping and output sample the authored crop for patterns.
import assert from 'node:assert/strict';
import { access, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const host = await openNativeEditor();
const { page } = host;
let token,
	stage = 'prepare';
const errors = [],
	checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function screens(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.screens',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
async function review(id, mode) {
	await tab(id).click();
	const panel = page.locator(`section.workbench-editor[data-panel-id="${id}"]`);
	await panel.locator('.native-preview').waitFor();
	await writeFile(
		path.join(directory, `review-${mode}.json`),
		JSON.stringify({ pid: host.app.pid, stage, panel: id })
	);
	console.log(JSON.stringify({ readyForReview: mode, pid: host.app.pid }));
	const deadline = Date.now() + 120000;
	for (;;) {
		try {
			await access(path.join(directory, `review-${mode}.done`));
			break;
		} catch {}
		assert.equal(host.app.exitCode, null);
		assert(Date.now() < deadline, `${mode} review expired`);
		await delay(100);
	}
	await page.waitForFunction(
		(id) =>
			document
				.querySelector(`section.workbench-editor[data-panel-id="${id}"] .native-preview`)
				?.getAttribute('data-preview-installed') === 'true',
		id
	);
	checks.push(stage);
}
try {
	token = (await post('/api/ui/session', {})).token;
	const screen = (await screens({ kind: 'InspectManager' })).snapshot.screens[0];
	const reference = screen.surfaces[0].reference;
	await screens({
		kind: 'ConfigureScreen',
		screen: screen.id,
		definition: {
			...screen.definition,
			enabled: true,
			target: { kind: 'AutomaticSpout', extent: { width: 320, height: 180 } }
		}
	});
	const crop = { x: 0.25, y: 0.25, width: 0.5, height: 0.5 };
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: {
			...screen.surfaces[0].definition,
			source: null,
			calibration: 'ColorBars',
			crop
		}
	});
	await screens({ kind: 'RenameSurface', reference, label: 'Cropped pattern' });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Screens$/ })
		.click();
	await page
		.locator('section[aria-label="Screens"]')
		.getByRole('button', { name: 'Cropped pattern', exact: true })
		.click();
	stage = 'whole source ColorBars with center-half crop contour';
	await review('surface-source', 'source');
	stage = 'Screen Warping samples center-half ColorBars, not all eight bars';
	await review('screen-warp', 'bars');
	await screenParameterControls(page).choice('Calibration').selectOption('Grid');
	stage = 'Screen Warping samples the cropped center of the Grid';
	await review('screen-warp', 'grid');
	assert.deepEqual(
		(await screens({ kind: 'InspectSurface', reference })).snapshot.definition.crop,
		crop
	);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ status: 'pass', pid: host.app.pid, crop, checks }, null, 2)
	);
	console.log(JSON.stringify({ status: 'pass', pid: host.app.pid, checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, log: host.log() }, null, 2)
	);
	throw error;
} finally {
	await host.close();
}
