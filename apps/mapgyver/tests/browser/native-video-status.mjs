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
	const position = parameter('Current Position').locator('input.time-field');
	const slider = parameter('Current Position').getByRole('slider');
	const positionValue = async () => clockMilliseconds(await position.inputValue());
	await wait(
		() => slider.getAttribute('aria-valuemax'),
		(value) => value === '0',
		'unknown duration range'
	);
	assert.equal(await positionValue(), 0);
	assert.equal(await position.isEditable(), true);
	assert.equal(await parameter('Seek to').count(), 0);
	const unknown = await edit({
		kind: 'Video',
		media: reference.media,
		edit: { kind: 'Seek', position_ms: 999 }
	});
	assert.equal(unknown.snapshot.position_ms, 0);
	await edit({ kind: 'ConfigureVideoFile', reference, file: fixture('h264.mp4') });
	assert.equal(
		await wait(
			() => duration(reference),
			(value) => value != null,
			'short metadata'
		),
		250
	);

	await wait(lengthValue, (value) => value === 250, 'short Inspector Length');
	await wait(
		() => slider.getAttribute('aria-valuemax'),
		(value) => Number(value) === 250,
		'short position range'
	);
	await position.fill('00:00:00.125');
	await position.press('Enter');
	await wait(
		async () => (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot.position_ms,
		(value) => value === 125,
		'Inspector position seek'
	);
	await position.fill('00:00:01.000');
	await position.press('Enter');
	await wait(positionValue, (value) => value === 250, 'Inspector seek clamps to duration');

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
	await wait(
		() => slider.getAttribute('aria-valuemax'),
		(value) => Number(value) === longest,
		'longest position range'
	);
	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms: 400 } });
	await wait(positionValue, (value) => value === 400, 'seek within longest variant');

	await edit({ kind: 'ConfigureVideoFile', reference: longer, file: '' });
	await wait(lengthValue, (value) => value === 250, 'cleared longest Inspector Length');
	await wait(
		() => slider.getAttribute('aria-valuemax'),
		(value) => Number(value) === 250,
		'shrinking position range'
	);
	await wait(positionValue, (value) => value === 250, 'shrinking duration clamps position');
	assert.equal(
		(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot.position_ms,
		250
	);

	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Loop', looping: true } });
	await parameter('Play').getByRole('button', { name: 'Trigger', exact: true }).click();
	await wait(() => playing.isChecked(), Boolean, 'live Playing');
	const livePositions = [];
	for (let sample = 0; sample < 7; sample++) {
		livePositions.push(await positionValue());
		await delay(75);
	}
	assert(
		new Set(livePositions.map(Math.round)).size >= 3,
		`live position did not advance: ${livePositions}`
	);
	await edit({ kind: 'Video', media: reference.media, edit: { kind: 'Seek', position_ms: 0 } });
	await wait(positionValue, (value) => value < 25, 'fresh seek before drag');
	const box = await slider.boundingBox();
	assert(box && box.width > 8 && box.height > 0, 'visible position slider');
	await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.5);
	await page.mouse.down();
	let heldPosition, acceptedSeek;
	try {
		await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.5);
		acceptedSeek = await wait(
			async () => (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot,
			(value) => value.settings.position_ms > 0,
			'active slider seek accepted'
		);
		await wait(
			positionValue,
			(value) => Math.abs(value - acceptedSeek.settings.position_ms) <= 1,
			'active seek presentation'
		);
		heldPosition = await positionValue();
		await delay(350);
		assert.equal(
			await positionValue(),
			heldPosition,
			'clock feedback must not fight the held seek'
		);
		assert.equal(
			(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot.settings.mode,
			'Playing'
		);
	} finally {
		await page.mouse.up();
	}
	await wait(
		positionValue,
		(value) => Math.abs(value - heldPosition) > 1,
		'feedback resumes after release'
	);
	assert.equal(
		(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot.settings.position_ms,
		acceptedSeek.settings.position_ms,
		'feedback must not become another seek'
	);

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
		live_positions_ms: livePositions,
		seek_feedback: {
			held_ms: heldPosition,
			accepted_ms: acceptedSeek.settings.position_ms,
			frozen_for_350ms: true,
			resumed_after_release: true,
			transport_mode_preserved: true
		},
		read_only: { length: true, playing: true },
		transport_triggers: ['Play', 'Pause', 'Stop', 'Play', 'Pause'],
		position: {
			widget: 'time_slider',
			editable: true,
			ranges_ms: [0, 250, longest, 250],
			manual_seek_ms: 125,
			clamped_seek_ms: 250,
			longest_seek_ms: 400
		},
		play_at_load: {
			default: false,
			ready_change_does_not_play: true,
			saved_reload_starts_at_ready: true
		},
		source,
		scope:
			'Generic Inspector status, bounded Current Position, transport triggers and authored Play at Load with project reload/metadata readiness; live clock feedback and held-seek suspension/resume verified; no codec pixels, recovery or full M10/5 acceptance',
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
