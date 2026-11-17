// Ordinary bundled Inspector control-mode edits; backend requests only prepare/observe fixtures.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
const { page } = native;
page.setDefaultTimeout(10000);
let token,
	stage = 'setup',
	report;
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const receipts = await observeNativeRejectedRequests(page, () => stage);
const controls = screenParameterControls(page);
const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function edit(namespace, payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
const media = (payload) => edit('mapgyver.media', payload);
const screens = (payload) => edit('mapgyver.screens', payload);
async function wait(read, ready) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
const trigger = (label) => controls.parameter(label).locator('.control-mode-trigger').first();
async function mode(label, desired) {
	await trigger(label).click();
	await page.getByRole('menu').getByText(desired, { exact: true }).click();
	await wait(
		() => trigger(label).getAttribute('class'),
		(value) => value.split(' ').includes(desired.toLowerCase())
	);
}
async function samples(target) {
	const values = [];
	for (let index = 0; index < 22; index++) {
		assert.equal(native.app.exitCode, null, native.log());
		const value = await target.read();
		assert(
			Number.isFinite(value) && value >= target.min && value <= target.max,
			`${target.name}: ${value}`
		);
		if (target.step) assert(Math.abs(value / target.step - Math.round(value / target.step)) < 1e-7);
		values.push(value);
		await delay(65);
	}
	assert(Math.max(...values) - Math.min(...values) > 0.3, `${target.name} must keep animating`);
	return values;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const created = await screens({ kind: 'CreateScreen', label: 'Control screen' });
	await screens({ kind: 'RenameSurface', reference: created.surface, label: 'Animation Surface' });
	const source = (
		await media({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'Control source' })
	).reference;
	const composition = (
		await media({ kind: 'CreateMedia', media_kind: 'Composition', label: 'Control composition' })
	).reference;
	const compose = (edit) => media({ kind: 'Composition', reference: composition, edit });
	const instance = (await compose({ kind: 'AddInstances', sources: [source] })).instances[0]
		.instance;
	await compose({ kind: 'RenameInstance', instance, label: 'Animation opacity' });
	const video = (
		await media({ kind: 'CreateMedia', media_kind: 'Video', label: 'Animation Video' })
	).reference;
	async function showMedia(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: label, exact: true })
			.click();
	}
	const targets = [
		{
			name: 'Surface Opacity',
			label: 'Opacity',
			min: 0,
			max: 1,
			show: async () => {
				await page
					.getByRole('tab')
					.filter({ hasText: /^Screens$/ })
					.click();
				await page
					.getByRole('region', { name: 'Screens', exact: true })
					.getByRole('button', { name: 'Animation Surface', exact: true })
					.click();
				await tab('screen-warp').click();
			},
			read: async () =>
				(await screens({ kind: 'InspectSurface', reference: created.surface })).snapshot.definition
					.opacity
		},
		{
			name: 'Composition Opacity',
			label: 'Opacity',
			min: 0,
			max: 1,
			show: async () => {
				await showMedia('Control composition');
				await tab('composition').click();
				await page
					.locator('[data-panel-id="composition"]')
					.getByRole('button', { name: 'Select Animation opacity', exact: true })
					.click();
			},
			read: async () =>
				(await compose({ kind: 'Inspect' })).snapshot.instances[0].definition.opacity
		},
		{
			name: 'Video Speed',
			label: 'Speed',
			min: -16,
			max: 16,
			step: 0.01,
			show: () => showMedia('Animation Video'),
			read: async () =>
				(await media({ kind: 'InspectVideo', media: video.media })).snapshot.settings.speed
		}
	];
	const results = [];
	for (const target of targets) {
		stage = target.name;
		await target.show();
		await mode(target.label, 'Animation');
		const before = await samples(target);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => trigger(target.label).getAttribute('class'),
			(value) => value.split(' ').includes('manual')
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => trigger(target.label).getAttribute('class'),
			(value) => value.split(' ').includes('animation')
		);
		await mode(target.label, 'Manual');
		await controls.setNumber(target.label, 0.5);
		await wait(target.read, (value) => value === 0.5);
		await mode(target.label, 'Animation');
		results.push({ target: target.name, before, modeUndoRedo: true, manualEdit: 0.5 });
	}
	stage = 'project save/reload with all three controls active';
	const project = path.join(directory, 'Animated controls.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	const loaded = await post('/api/ui/project-load', { path: project });
	assert.equal(path.resolve(loaded.path), project);
	for (const [index, target] of targets.entries()) {
		stage = `reloaded ${target.name}`;
		await target.show();
		await wait(
			() => trigger(target.label).getAttribute('class'),
			(value) => value.split(' ').includes('animation')
		);
		results[index].afterReload = await samples(target);
	}
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	assert(receipts.observed.receipts >= 15, 'observe actual applied Inspector requests');
	assert(!native.log().includes('panicked at'), native.log());
	await page.screenshot({ path: path.join(directory, 'inspector-animation.png') });
	report = {
		result: 'pass',
		pid: native.app.pid,
		scope:
			'actual bundled Inspector Animation/Manual, mode Undo/Redo, project reload, finite changing targets',
		results,
		errors,
		receipts
	};
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failed.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), log: native.log(), errors, receipts }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
report.nativeExitCode = 0;
await writeFile(path.join(directory, 'result.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report));
