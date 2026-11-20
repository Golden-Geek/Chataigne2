// Bundled native app and independent SDK captures. Run after the normal MapGyver build.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { copyFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { windowCaptureSource } from './native-latency.mjs';

const directory = path.resolve(process.argv[2] ?? 'target/mapgyver-video-fade-loop');
const moving = process.argv.includes('--moving');
const selectedAdapter = process.env.MAPGYVER_VIDEO_ADAPTER;
const observer = path.resolve('target/debug/native_qualification.exe');
const execute = promisify(execFile);
const run = async (file, args, timeout = 12000) =>
	(await execute(file, args, { windowsHide: true, timeout })).stdout;
const extent = { width: 160, height: 120 };
const overlapMs = 500;
const durationMs = 2000;
const clip = path.join(directory, 'fade-loop-red-blue.mp4');
await mkdir(directory, { recursive: true });
async function makeClip(file, first, second) {
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-y',
		'-f',
		'lavfi',
		'-i',
		`color=${first[0]}:s=160x120:r=30:d=${first[1]}`,
		'-f',
		'lavfi',
		'-i',
		`color=${second[0]}:s=160x120:r=30:d=${second[1]}`,
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
		file
	]);
}
await makeClip(clip, ['red', 1], ['blue', 1]);

const hardware = JSON.parse(await run(observer, ['--role', 'hardware']));
const logTails = { stdout: '', stderr: '' };
let retiredSessions = 0;
const native = await openNativeEditor({
	onLogData(data, stream) {
		const lines = (logTails[stream] + data).split('\n');
		logTails[stream] = lines.pop().slice(-4096);
		for (const line of lines) {
			if (line.trim() === '[mapgyver-output] native session retired') retiredSessions++;
		}
	},
	prepareProfile: selectedAdapter
		? async (root) => {
				const profile = path.join(root, 'MapGyver');
				await mkdir(profile, { recursive: true });
				await run(observer, [
					'--role',
					'gpu-preferences',
					'--file',
					path.join(profile, 'preferences.json'),
					'--selected',
					selectedAdapter
				]);
			}
		: undefined
});
const { page } = native;
const pageErrors = [];
page.on('pageerror', (error) => pageErrors.push(String(error)));
const captures = [],
	seams = [],
	playback = [];
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
const source = async (reference = video) =>
	(await edit({ kind: 'InspectVideoSource', reference })).status;
const control = (request) =>
	page.evaluate(
		async (request) =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify(request)
				})
			),
		request
	);
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
async function pixels(file, label) {
	const value = JSON.parse(
		await run('python', ['apps/mapgyver/tests/native/video_policy_pixels.py', file])
	);
	assert.equal(value.rgba.length, 3, 'independent interior samples');
	const entry = { label, ...value, file };
	captures.push(entry);
	return entry;
}
async function spout(sender, label) {
	const folder = path.join(directory, `capture-${String(captures.length + 1).padStart(3, '0')}`);
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
	return pixels(path.join(folder, 'pixels.png'), label);
}
const allPixels = (value, predicate) =>
	value.rgba.every((sample, i) => sample[3] === 255 && predicate(sample, i));
const captured = (sender, label, predicate) =>
	wait(
		() => spout(sender, label),
		(value) => allPixels(value, predicate),
		label
	);
const linear = (byte) => {
	const encoded = byte / 255;
	return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4;
};
const smoothstep = (t) => t * t * (3 - 2 * t);
const betweenMix = (outgoing, incoming, low, high) => (sample, index) =>
	sample.slice(0, 3).every((value, channel) => {
		const first = linear(outgoing.rgba[index][channel]);
		const difference = linear(incoming.rgba[index][channel]) - first;
		const a = first + low * difference;
		const b = first + high * difference;
		const observed = linear(value);
		return observed >= Math.min(a, b) - 0.035 && observed <= Math.max(a, b) + 0.035;
	});
const nearMix = (outgoing, incoming, weight) => betweenMix(outgoing, incoming, weight, weight);

async function previewSampler() {
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	await tab('Medias').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Auto-Smooth Loop smoke', exact: true })
		.click();
	await tab('Media Preview').click();
	await page.getByRole('combobox', { name: 'Preview channel' }).selectOption(video.channel);
	const viewport = page.locator(
		'.media-preview-panel .native-preview[data-preview-installed="true"]'
	);
	await viewport.waitFor();
	const [main] = JSON.parse(
		await run('python', [
			'apps/mapgyver/tests/native_output/window_state.py',
			String(native.app.pid),
			'--main'
		])
	);
	assert(
		main?.visible && !main.minimized,
		'owned test window must be available to Windows Graphics Capture'
	);
	return async (label) => {
		const box = await viewport.boundingBox();
		assert(box && Math.abs(box.width / box.height - 4 / 3) < 0.02);
		const layout = await page.evaluate(() => ({
			width: innerWidth,
			height: innerHeight,
			dpr: devicePixelRatio
		}));
		const { dpr } = layout;
		const capture = windowCaptureSource({
			hwnd: main.hwnd,
			left: Math.round(box.x * dpr),
			top: Math.round(box.y * dpr),
			width: Math.round(box.width * dpr),
			height: Math.round(box.height * dpr),
			viewport: layout,
			dpr
		});
		const file = path.join(directory, `preview-${captures.length + 1}.png`);
		await run('ffmpeg', [
			'-hide_banner',
			'-loglevel',
			'error',
			'-filter_complex',
			`${capture},hwdownload,format=bgra,scale=${extent.width}:${extent.height}`,
			'-frames:v',
			'1',
			file
		]);
		return pixels(file, label);
	};
}

async function movingPixels(label, speed, [start, end], sample, predicate) {
	const witness = await wait(
		async () => {
			const before = await inspect();
			assert(before.playing && before.settings.speed === speed, label);
			if (before.position_ms < start || before.position_ms > end)
				return { positionMs: before.position_ms };
			const image = await sample(label);
			const after = await inspect();
			return {
				beforeMs: before.position_ms,
				afterMs: after.position_ms,
				playing: after.playing,
				image
			};
		},
		(value) =>
			value.playing &&
			value.afterMs >= start &&
			value.afterMs <= end &&
			Math.sign(speed) * (value.afterMs - value.beforeMs) > 0 &&
			allPixels(value.image, predicate),
		label,
		9000
	);
	playback.push({ label, speed, ...witness });
	return witness;
}

async function observedWrap(speed, overlap, previous) {
	const sign = Math.sign(speed);
	const label = speed < 0 ? 'reverse mixed-duration wrap' : 'forward mixed-duration wrap';
	await wait(
		inspect,
		(sample) => {
			assert(sample.playing && sample.settings.speed === speed, label);
			const current = sample.position_ms;
			if ((previous - current) * sign > durationMs / 2) {
				const restart = sign < 0 ? durationMs - overlap : overlap;
				assert(previous > (sign < 0 ? -1 : durationMs - 200));
				if (sign < 0) assert(previous < 200);
				assert(
					(current - restart) * sign >= -0.01 && (current - restart) * sign < 160,
					`${label} should skip the consumed overlap: ${previous} → ${current}`
				);
				playback.push({ label, speed, beforeMs: previous, afterMs: current, restart });
				return true;
			}
			previous = current;
			return false;
		},
		label,
		9000
	);
}

async function verifyMixed(direct, shared, head, tail) {
	const shortDuration = 1200;
	const clamped = shortDuration / 2;
	const shortFile = path.join(directory, 'fade-loop-short.mp4');
	await makeClip(shortFile, ['lime', 0.9], ['yellow', 0.3]);
	const short = (await edit({ kind: 'AddChannel', media: video.media, label: 'Shorter file' }))
		.reference;
	await edit({ kind: 'ConfigureVideoFile', reference: short, file: shortFile });
	const shortOutput = await output(short, 'Fade-loop shorter output');
	await wait(
		() => source(short),
		(value) => value.state === 'Ready' && value.duration_ms === shortDuration,
		'actual shorter Video channel'
	);
	const unknown = (await edit({ kind: 'AddChannel', media: video.media, label: 'No file yet' }))
		.reference;
	assert.equal((await source(unknown)).state, 'Unconfigured');
	const authored = await transport({ kind: 'AutoSmoothLoop', duration_ms: 900 });
	assert.equal(authored.length_ms, durationMs);
	assert.equal(authored.settings.auto_smooth_loop_ms, 900);
	const seek = (position_ms) => transport({ kind: 'Seek', position_ms });
	await seek(250);
	const shortHead = await captured(
		shortOutput,
		'short head',
		([r, g, b]) => r < 120 && g > 190 && b < 120
	);
	await seek(1300);
	const shortTail = await captured(
		shortOutput,
		'short tail held past end',
		([r, g, b]) => r > 190 && g > 190 && b < 120
	);
	await captured(direct, 'before the clamped overlap', nearMix(tail, head, 0));
	await seek(durationMs - clamped / 2);
	await captured(
		direct,
		'halfway through 600 ms effective forward overlap',
		nearMix(tail, head, 0.5)
	);
	await captured(
		shortOutput,
		'short held tail blends into the same source head',
		nearMix(shortTail, shortHead, 0.5)
	);

	const onPreview = await previewSampler();
	const onDirect = (label) => spout(direct, label);
	const onReference = (label) => spout(shared, label);
	const onShort = (label) => spout(shortOutput, label);
	for (const [speed, label, outgoing, incoming] of [
		[0.2, 'forward', tail, head],
		[-0.2, 'reverse', head, tail]
	]) {
		await transport({ kind: 'Pause' });
		await transport({ kind: 'Speed', speed });
		const offset = speed > 0 ? durationMs - clamped : clamped;
		const positionAt = (progress) => offset + Math.sign(speed) * clamped * progress;
		await seek(positionAt(-0.2));
		await transport({ kind: 'Play' });
		let last;
		for (const [name, sample, from, to] of [
			['direct output', onDirect, 0.13, 0.36],
			['Reference output', onReference, 0.38, 0.62],
			['native Media Preview', onPreview, 0.64, 0.9]
		]) {
			const range = [positionAt(from), positionAt(to)].sort((a, b) => a - b);
			const expected = betweenMix(
				outgoing,
				incoming,
				smoothstep(from - 0.04),
				smoothstep(to + 0.04)
			);
			const witness = await movingPixels(
				`${label} overlap ${name}`,
				speed,
				range,
				sample,
				expected
			);
			if (last)
				assert((witness.beforeMs - last.afterMs) * Math.sign(speed) >= 0, 'same advancing seam');
			last = witness;
		}
		await observedWrap(speed, clamped, last.afterMs);
		const endState = speed > 0 ? shortHead : shortTail;
		const range = speed > 0 ? [clamped + 15, 835] : [shortDuration + 25, durationMs - clamped - 15];
		await movingPixels(
			`${label} shorter file after wrap`,
			speed,
			range,
			onShort,
			nearMix(endState, endState, 0)
		);
	}

	// The render session can remain active for an empty editor's native backing surface.
	// Release preview consumers before asserting that replacement of the last outputs drains it.
	for (const name of [
		'Screen Warping',
		'Surface Source',
		'Composition',
		'Matrix',
		'Media Preview'
	]) {
		const tab = page.getByRole('tab').filter({ hasText: new RegExp(`^${name}(?::|$)`) });
		while (await tab.count()) {
			await tab.first().getByRole('button', { name: 'Close panel', exact: true }).click();
		}
	}
	await page.waitForFunction(() => !document.querySelector('.native-preview'));

	await transport({ kind: 'Pause' });
	await transport({ kind: 'Speed', speed: 0.2 });
	await seek(durationMs - clamped / 2);
	await transport({ kind: 'Play' });
	await movingPixels(
		'moving overlap before relink',
		0.2,
		[1690, 1850],
		onDirect,
		betweenMix(tail, head, 0.44, 0.9)
	);
	const relink = path.join(directory, 'recovered-after-overlap.mp4');
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: relink });
	await wait(
		source,
		(value) => value.state === 'Unavailable' && value.error?.kind === 'Missing',
		'missing relink'
	);
	const shorterTransport = await wait(
		inspect,
		(value) => value.playing && value.length_ms === shortDuration,
		'shorter channel keeps advancing after the other source is removed'
	);
	const cleared = 'missing source clears previous overlap';
	const empty = await wait(
		() => spout(direct, cleared),
		(value) => value.rgba.every((rgba) => rgba.every((byte) => byte === 0)),
		cleared
	);
	playback.push({ label: cleared, image: empty });
	await copyFile(clip, relink);
	await wait(
		source,
		(value) => value.state === 'Ready' && value.duration_ms === durationMs,
		'live file recovery'
	);
	const recoveredTransport = await wait(
		inspect,
		(value) => value.playing && value.length_ms === durationMs,
		'no extra Play required after recovery'
	);
	playback.push({
		label: 'live file recovery restored range',
		shortMs: shorterTransport.length_ms,
		restoredMs: recoveredTransport.length_ms
	});
	await movingPixels(
		'recovered source renders a moving blend',
		0.2,
		[1620, 1790],
		onDirect,
		betweenMix(tail, head, 0.2, 0.78)
	);
	const [beforeClose, active] = await Promise.all([
		inspect(),
		control({ kind: 'mapgyverRuntimeStatus' })
	]);
	assert(
		beforeClose.playing &&
			beforeClose.position_ms > durationMs - clamped &&
			beforeClose.position_ms < durationMs
	);
	assert.equal(active.state, 'Running');
	assert(
		active.resources.used_bytes > 0 && active.resources.used_bytes <= active.resources.limit_bytes
	);
	const previousRetirements = retiredSessions;
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	const retired = await wait(
		() => control({ kind: 'mapgyverRuntimeStatus' }),
		(value) =>
			value.sequence > active.sequence &&
			retiredSessions > previousRetirements &&
			value.state === 'Idle' &&
			value.gpu_adapter === null &&
			value.resources === null &&
			value.screens.length === 0 &&
			!value.error,
		'New retires the decoder and GPU session during overlap'
	);
	playback.push({
		label: 'project replacement retires live overlap',
		positionMs: beforeClose.position_ms,
		activeResources: active.resources,
		retired,
		completedSessions: retiredSessions - previousRetirements
	});
}

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
	if (selectedAdapter) assert.equal(adapter.key, selectedAdapter);
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
	if (moving) await verifyMixed(direct, shared, head, tail);
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
					playback,
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
