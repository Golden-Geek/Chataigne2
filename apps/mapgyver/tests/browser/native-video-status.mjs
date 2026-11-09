// Real generic Inspector status rows; codec pixel/recovery qualification stays separate.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2] ?? 'target/mapgyver-video-status');
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
const pageErrors = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
let token;
async function post(endpoint, body) {
	const response = await fetch(`${native.base}${endpoint}`, {
		method: 'POST',
		signal: AbortSignal.timeout(5000),
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
async function wait(read, predicate, label) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const value = await read();
		if (predicate(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(40);
	}
}
const parameter = (label) =>
	page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
const duration = async (reference) =>
	(await edit({ kind: 'InspectVideoSource', reference })).status.duration_ms;
const clockMilliseconds = (text) =>
	text
		.split(':')
		.map(Number)
		.reduce((total, part) => total * 60 + part, 0) * 1000;
let report;
try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	const label = 'Video runtime status';
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label });
	const fixture = (name) => path.resolve('crates/golden_video/src/decode/tests/fixtures', name);
	await edit({ kind: 'ConfigureVideoFile', reference, file: fixture('h264.mp4') });
	assert.equal(
		await wait(
			() => duration(reference),
			(value) => value != null,
			'short metadata'
		),
		250
	);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias(?::|$)/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	const length = parameter('Length').locator('input.time-field');
	const playing = parameter('Playing').locator('input[type=checkbox]');
	const lengthValue = async () => clockMilliseconds(await length.inputValue());
	await wait(lengthValue, (value) => value === 250, 'short Inspector Length');
	assert.equal(await length.isEditable(), false);
	assert.equal(await playing.isDisabled(), true);
	assert.equal(await playing.isChecked(), false);
	const { reference: longer } = await edit({
		kind: 'AddChannel',
		media: reference.media,
		label: 'Longer variant'
	});
	await edit({
		kind: 'ConfigureVideoFile',
		reference: longer,
		file: fixture('h264-keyframes.mp4')
	});
	const longest = await wait(
		() => duration(longer),
		(value) => value > 250,
		'longest metadata'
	);
	await wait(lengthValue, (value) => Math.abs(value - longest) < 0.001, 'longest Inspector Length');
	await edit({ kind: 'ConfigureVideoFile', reference: longer, file: '' });
	await wait(lengthValue, (value) => value === 250, 'cleared longest Inspector Length');
	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Loop', looping: true } });
	await parameter('Play').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(() => playing.isChecked(), Boolean, 'live Playing');
	await parameter('Pause').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(
		() => playing.isChecked(),
		(value) => !value,
		'live Paused'
	);
	const loadPolicy = parameter('Play at Load').locator('input[type=checkbox]');
	assert.equal(await loadPolicy.isChecked(), false);
	await loadPolicy.check();
	await wait(
		async () =>
			(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot.settings.play_at_load,
		Boolean,
		'authored load policy'
	);
	assert.equal(
		await playing.isChecked(),
		false,
		'changing ready media policy does not play immediately'
	);
	const project = path.join(native.directory, 'video-load-policy.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await post('/api/ui/project-load', { path: project });
	await wait(
		async () => (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot,
		(value) => value.ready && value.playing && value.settings.play_at_load,
		'load policy starts at file readiness'
	);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias(?::|$)/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	await wait(() => playing.isChecked(), Boolean, 'reloaded live Playing');
	await parameter('Stop').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(
		async () => (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot,
		(value) => value.settings.mode === 'Stopped' && value.position_ms === 0,
		'UI Stop trigger'
	);
	await parameter('Play').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(() => playing.isChecked(), Boolean, 'repeat UI Play trigger');
	await parameter('Pause').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(
		() => playing.isChecked(),
		(value) => !value,
		'UI Pause after reload'
	);
	assert.deepEqual(pageErrors, []);
	const source = (await edit({ kind: 'InspectVideoSource', reference })).status;
	report = {
		result: 'pass',
		pid: native.app.pid,
		pageErrors,
		length_ms: [250, longest, 250],
		playing: [false, true, false],
		read_only: { length: true, playing: true },
		transport_triggers: ['Play', 'Pause', 'Stop', 'Play', 'Pause'],
		play_at_load: {
			default: false,
			ready_change_does_not_play: true,
			saved_reload_starts_at_ready: true
		},
		source,
		scope:
			'Generic Inspector status, transport triggers and authored Play at Load with project reload/metadata readiness; no codec pixels, seek, recovery or full M10/5 acceptance',
		normalShutdown: false
	};
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				error: String(error),
				pageErrors,
				body: await page.locator('body').innerText(),
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await native.close();
}
report.normalShutdown = true;
await writeFile(path.join(directory, 'video-status.json'), JSON.stringify(report, null, 2));
console.log(JSON.stringify(report));
