// Independent Screen pixels and live WASAPI mixer levels from one Video clock.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const clip = path.join(directory, 'intro-tail.mp4');
const sdk = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const extent = { width: 160, height: 120 };
const execute = promisify(execFile);
const run = async (program, args) =>
	(await execute(program, args, { windowsHide: true, timeout: 15000 })).stdout;
const tone = 'if(lt(t\\,3)\\,0.012*sin(2*PI*440*t)\\,0)|if(gte(t\\,3)\\,0.012*sin(2*PI*640*t)\\,0)';
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	'-n',
	'-f',
	'lavfi',
	'-i',
	'color=black:s=160x120:r=24:d=6',
	'-f',
	'lavfi',
	'-i',
	`aevalsrc=${tone}:s=44100:d=6`,
	'-vf',
	[
		"drawbox=color=red:t=fill:enable='lt(t,3)'",
		"drawbox=color=blue:t=fill:enable='between(t,3,4.49)'",
		"drawbox=color=lime:t=fill:enable='gte(t,4.5)'"
	].join(','),
	'-c:v',
	'libx264',
	'-pix_fmt',
	'yuv420p',
	'-c:a',
	'aac',
	'-b:a',
	'192k',
	clip
]);

const hardware = JSON.parse(await run(sdk, ['--role', 'hardware']));
const native = await openNativeEditor();
const { page } = native;
const errors = [],
	captures = [],
	wraps = [];
let failure, token, adapter, video, audioIntro, audioTail;
page.on('pageerror', (error) => errors.push(String(error)));

async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(6000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}
const op = (namespace, operation, payload) =>
	intent({ kind: 'invokeAppOperation', namespace, operation, payload });
const edit = (payload, owner = 'media') => op(`mapgyver.${owner}`, 'edit', payload);
const inspect = async () => (await edit({ kind: 'InspectVideo', media: video.media })).snapshot;
const transport = async (editValue) =>
	(await edit({ kind: 'Video', media: video.media, edit: editValue })).snapshot;
const audio = () => op('mapgyver.audio', 'inspect-output', null);
async function wait(read, accept, label, timeout = 7000) {
	const deadline = Date.now() + timeout;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
const levels = async () =>
	Object.fromEntries(
		(await audio()).output_levels.map(({ channel, rms_linear }) => [channel, rms_linear])
	);
const silent = (level) => level < 0.0001;
const dominant = (pixel, axis) =>
	pixel[3] === 255 &&
	pixel[axis] > 205 &&
	pixel.every((value, index) => index === 3 || index === axis || value < 60);
const sender = `MapGyver-Video-Intro-Loop-${randomUUID()}`;
let frameCount = 0;
async function capture(label, predicate) {
	const pixel = await wait(
		async () => {
			const folder = path.join(directory, `frame-${String(++frameCount).padStart(2, '0')}`);
			const shape = JSON.parse(
				await run(sdk, [
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
			return JSON.parse(
				await run('python', [
					'-c',
					'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGBA"); print(json.dumps(im.getpixel((80,60))))',
					path.join(folder, 'pixels.png')
				])
			);
		},
		predicate,
		label
	);
	captures.push({ label, pixel });
	return pixel;
}

try {
	token = (await post('/api/ui/session', {})).token;
	const nodes = (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
	const prefs = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
	const enabled = nodes.find(
		(node) => prefs.children.includes(node.node_id) && node.decl_id === 'output_enabled'
	);
	await intent({
		kind: 'setParam',
		node: enabled.node_id,
		value: { Bool: true },
		behaviour: 'Append'
	});
	await wait(
		audio,
		(value) => value.readiness === 'ready' && value.routes.length === 2,
		'WASAPI output ready'
	);
	const label = 'Video intro and tail';
	({ reference: video } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label }));
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: clip });
	const { screen, surface } = await edit({ kind: 'CreateScreen', label: 'Tail output' }, 'screens');
	const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...snapshot.definition, source: video }
		},
		'screens'
	);
	await edit(
		{
			kind: 'ConfigureScreen',
			screen,
			definition: {
				enabled: true,
				anti_aliasing: 'Raw',
				frame_rate: { kind: 'Explicit', fps: 60 },
				target: { kind: 'Spout', sender, extent }
			}
		},
		'screens'
	);
	adapter = await observeNativeAdapter(page, hardware);
	await wait(
		() => edit({ kind: 'InspectVideoSource', reference: video }),
		(value) => value.status.state === 'Ready',
		'Video played by an actual Screen'
	);
	assert.equal((await transport({ kind: 'Crop', start_ms: 500, end_ms: 5500 })).length_ms, 5000);
	await transport({ kind: 'Loop', looping: true });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	const loopStart = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Loop Start', { exact: true }) })
		.locator('input.time-field');
	await loopStart.fill('00:00:03.500');
	await loopStart.press('Enter');
	await wait(
		inspect,
		(value) => value.settings.loop_start_ms === 3500,
		'Inspector authors source-time Loop Start'
	);
	await transport({ kind: 'Stop' });
	await capture('held cropped intro is red', (pixel) => dominant(pixel, 0));
	await wait(
		levels,
		(value) => silent(value[left]) && silent(value[right]),
		'held intro is silent'
	);

	let before = (await transport({ kind: 'Play' })).position_ms;
	audioIntro = await wait(
		levels,
		(value) => value[left] > 0.005 && silent(value[right]),
		'intro audio is left-only'
	);
	await capture('one-time moving intro is red', (pixel) => dominant(pixel, 0));
	await wait(
		inspect,
		(value) => {
			assert(value.playing);
			const after = value.position_ms;
			if (before - after > 1000) {
				assert(
					before > 4800 && after >= 3000 && after < 3200,
					`unexpected hard seam ${before} → ${after}`
				);
				wraps.push({ before, after });
			}
			if (wraps.length) assert(after >= 3000, 'the intro cannot replay after the first seam');
			before = after;
			return wraps.length === 2;
		},
		'two hard seams enter the authored tail',
		10000
	);
	audioTail = await wait(
		levels,
		(value) => silent(value[left]) && value[right] > 0.005,
		'loop audio is right-only'
	);
	await capture(
		'repeat picture uses only the blue or green tail',
		(pixel) => dominant(pixel, 1) || dominant(pixel, 2)
	);

	await transport({ kind: 'Pause' });
	await transport({ kind: 'AutoSmoothLoop', duration_ms: 500 });
	await transport({ kind: 'Seek', position_ms: 4750 });
	await capture(
		'held smooth seam blends green with blue from Loop Start',
		([r, g, b, a]) => a === 255 && r < 35 && g > 110 && b > 110
	);
	assert.equal((await inspect()).position_ms, 4750);
	await transport({ kind: 'Stop' });
	await capture('Stop restores the red intro', (pixel) => dominant(pixel, 0));
	await wait(
		levels,
		(value) => silent(value[left]) && silent(value[right]),
		'Stop retires the audio signal'
	);
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	await writeFile(path.join(directory, 'app.log'), native.log());
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: failure ? 'fail' : 'pass',
				failure,
				adapter: adapter?.description,
				captures,
				wraps,
				audioIntro,
				audioTail
			},
			null,
			2
		)
	);
}
console.log(JSON.stringify({ result: 'pass', captures, wraps }));
