// Bundled native app and independent SDK captures. Run after the normal MapGyver build.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';

const directory = path.resolve(process.argv[2] ?? 'target/mapgyver-video-fade-loop');
const observer = path.resolve('target/debug/native_qualification.exe');
const execute = promisify(execFile);
const run = async (file, args, timeout = 12000) =>
	(await execute(file, args, { windowsHide: true, timeout })).stdout;
const extent = { width: 160, height: 120 };
const overlapMs = 500;
const durationMs = 2000;
const clip = path.join(directory, 'fade-loop-red-blue.mp4');
await mkdir(directory, { recursive: true });
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	'-y',
	'-f',
	'lavfi',
	'-i',
	'color=red:s=160x120:r=30:d=1',
	'-f',
	'lavfi',
	'-i',
	'color=blue:s=160x120:r=30:d=1',
	'-filter_complex',
	'[0:v][1:v]concat=n=2:v=1:a=0[v]',
	'-map',
	'[v]',
	'-an',
	'-c:v',
	'libx264',
	'-crf',
	'12',
	'-pix_fmt',
	'yuv420p',
	clip
]);

const hardware = JSON.parse(await run(observer, ['--role', 'hardware']));
const native = await openNativeEditor();
const { page } = native;
const pageErrors = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
const captures = [],
	seams = [];
let token, adapter, video, failure;
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function edit(payload, owner = 'media') {
	const result = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${owner}`,
		operation: 'edit',
		payload
	});
	assert(result.success, JSON.stringify(result));
	return result.result.value;
}
const inspect = async () => (await edit({ kind: 'InspectVideo', media: video.media })).snapshot;
const transport = async (operation) =>
	(await edit({ kind: 'Video', media: video.media, edit: operation })).snapshot;
async function wait(read, accept, label, milliseconds = 6000) {
	const deadline = performance.now() + milliseconds;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(performance.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
async function output(reference, name) {
	const { screen, surface } = await edit({ kind: 'CreateScreen', label: name }, 'screens');
	const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
	const sender = `MapGyver-Video-Fade-Loop-${randomUUID()}`;
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...snapshot.definition, source: reference }
		},
		'screens'
	);
	await edit(
		{
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				target: { kind: 'Spout', sender, extent }
			}
		},
		'screens'
	);
	return sender;
}
async function captured(sender, label, predicate) {
	return wait(
		async () => {
			const folder = path.join(
				directory,
				`capture-${String(captures.length + 1).padStart(3, '0')}`
			);
			await mkdir(folder, { recursive: true });
			const shape = JSON.parse(
				await run(observer, [
					'--role',
					'video-pixels',
					'--adapter',
					String(adapter.index),
					'--name',
					sender,
					'--directory',
					folder
				])
			);
			assert.deepEqual(shape, { ...extent, retired: true });
			const pixels = JSON.parse(
				await run('python', [
					'apps/mapgyver/tests/native/video_policy_pixels.py',
					path.join(folder, 'pixels.png')
				])
			);
			assert.equal(pixels.rgba.length, 3, 'independent interior samples');
			const entry = { label, ...pixels, file: path.join(folder, 'pixels.png') };
			captures.push(entry);
			return entry;
		},
		(value) => value.rgba.every((sample, i) => sample[3] === 255 && predicate(sample, i)),
		label
	);
}
const linear = (byte) => {
	const encoded = byte / 255;
	return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4;
};
const smoothstep = (t) => t * t * (3 - 2 * t);
const nearMix = (blue, red, weight) => (sample, index) =>
	sample
		.slice(0, 3)
		.every(
			(value, channel) =>
				Math.abs(
					linear(value) -
						((1 - weight) * linear(blue.rgba[index][channel]) +
							weight * linear(red.rgba[index][channel]))
				) <= 0.035
		);

try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	({ reference: video } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'Auto-Smooth Loop smoke'
	}));
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: clip });
	const direct = await output(video, 'Fade-loop Video output');
	adapter = await observeNativeAdapter(page, hardware);
	await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference: video })).status,
		(status) => status.state === 'Ready' && status.duration_ms === durationMs,
		'exact two-second decoded source'
	);
	assert.equal((await inspect()).settings.auto_smooth_loop_ms, 0);
	await transport({ kind: 'Pause' });
	await transport({ kind: 'Seek', position_ms: 250 });
	const head = await captured(direct, 'plain red head', ([r, g, b]) => r > 210 && g < 65 && b < 65);
	await transport({ kind: 'Seek', position_ms: 1750 });
	const tail = await captured(
		direct,
		'plain blue tail',
		([r, g, b]) => r < 65 && g < 65 && b > 210
	);
	assert.equal((await transport({ kind: 'Loop', looping: true })).settings.auto_smooth_loop_ms, 0);
	await captured(direct, 'zero overlap uses plain tail', nearMix(tail, head, 0));
	assert.equal(
		(await transport({ kind: 'AutoSmoothLoop', duration_ms: overlapMs })).settings
			.auto_smooth_loop_ms,
		overlapMs
	);

	const { media } = await edit({
		kind: 'CreateReference',
		target: video.media,
		label: 'Fade-loop shared Reference'
	});
	const mirrors = (await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 1 } }))
		.page.channels;
	assert.equal(mirrors.length, 1);
	const shared = await output(mirrors[0].reference, 'Fade-loop Reference output');
	for (const phase of [0.25, 0.5, 0.75]) {
		const positionMs = durationMs - overlapMs + phase * overlapMs;
		const settled = await transport({ kind: 'Seek', position_ms: positionMs });
		assert.equal(settled.playing, false);
		assert(Math.abs(settled.position_ms - positionMs) < 0.01);
		const expected = nearMix(tail, head, smoothstep(phase));
		const sample = await captured(direct, `smoothstep ${phase}`, expected);
		if (phase === 0.5) {
			const mirror = await captured(shared, 'shared Reference at the same held mix', expected);
			assert.deepEqual(mirror.rgba, sample.rgba);
		}
	}
	await transport({ kind: 'Seek', position_ms: 1750 });
	const off = await transport({ kind: 'Loop', looping: false });
	assert.equal(off.settings.auto_smooth_loop_ms, overlapMs);
	await captured(
		direct,
		'Loop off retains overlap but shows the plain tail',
		nearMix(tail, head, 0)
	);
	await transport({ kind: 'Loop', looping: true });
	await captured(direct, 'Loop on restores the same midpoint mix', nearMix(tail, head, 0.5));

	await transport({ kind: 'Seek', position_ms: 1800 });
	await transport({ kind: 'Speed', speed: 0.75 });
	const initial = await transport({ kind: 'Play' });
	let before = initial.position_ms;
	const deadline = performance.now() + 6000;
	while (seams.length < 2) {
		const next = await inspect();
		assert(next.playing && next.settings.looping);
		if (before - next.position_ms > 700) {
			assert(before > 1850, `seam departed early at ${before} ms`);
			assert(
				next.position_ms >= overlapMs && next.position_ms < overlapMs + 180,
				`seam did not continue from its consumed head: ${next.position_ms} ms`
			);
			seams.push({ beforeMs: before, afterMs: next.position_ms });
		}
		before = next.position_ms;
		assert(performance.now() < deadline, `repeated live seams absent: ${JSON.stringify(seams)}`);
		await delay(15);
	}
	await transport({ kind: 'Pause' });
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error);
	throw error;
} finally {
	try {
		await native.close();
	} catch (error) {
		failure ??= String(error);
		throw error;
	} finally {
		await writeFile(
			path.join(directory, 'result.json'),
			JSON.stringify(
				{
					result: failure ? 'fail' : 'pass',
					failure,
					pid: native.app.pid,
					adapter,
					durationMs,
					overlapMs,
					captures,
					seams,
					pageErrors
				},
				null,
				2
			)
		);
	}
}
console.log(
	`Video fade-loop passed on ${adapter.description}: ${seams.length} wraps. ${path.join(directory, 'result.json')}`
);
