// Still and animated WebP through a bundled Video channel and independent native output.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';

const root = path.resolve(process.argv[2] ?? 'target/m103-webp');
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const still = path.join(directory, 'still.webp');
const animated = path.join(directory, 'animated.webp');
const lossyStill = path.join(directory, 'still-lossy.webp');
const lossyAnimated = path.join(directory, 'animated-lossy.webp');
const red = [176, 35, 57, 255];
const green = [46, 198, 75, 255];
const blue = [44, 80, 206, 255];
const sdk = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
const lossyReference = JSON.parse(
	await run('python', [
		'-c',
		`
import json, sys
from PIL import Image
def save(colors, still, animation, lossless):
    frames = []
    for color in colors:
        image = Image.new("RGBA", (64,48))
        image.paste(tuple(color), (0,0,32,48))
        frames.append(image)
    frames[0].save(still, lossless=lossless, quality=95)
    frames[0].save(animation, save_all=True, lossless=lossless, quality=95,
                   append_images=frames[1:], duration=[240, 560, 400], loop=0)

save(json.loads(sys.argv[5]), sys.argv[1], sys.argv[2], True)
save([(186,135,33,255),(183,46,170,255),(21,152,165,255)], sys.argv[3], sys.argv[4], False)
with Image.open(sys.argv[3]) as still, Image.open(sys.argv[4]) as animation:
    animation.seek(2)
    print(json.dumps([still.convert("RGBA").getpixel((16,24)),
                      animation.convert("RGBA").getpixel((16,24))]))
`,
		still,
		animated,
		lossyStill,
		lossyAnimated,
		JSON.stringify([red, green, blue])
	])
);

const native = await openNativeEditor();
let token, failure, adapter;
const evidence = [];
async function post(route, body) {
	const result = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(6000)
	});
	const value = await result.json();
	assert(result.ok, JSON.stringify(value));
	return value;
}
async function edit(payload, domain = 'media') {
	const response = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${domain}`,
		operation: 'edit',
		payload
	});
	assert(response.success, JSON.stringify(response));
	return response.result.value;
}
async function wait(read, accept, label) {
	const deadline = Date.now() + 6000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}

const screenEdit = (payload) => edit(payload, 'screens');
async function setSurface(reference, source) {
	const { snapshot } = await screenEdit({ kind: 'InspectSurface', reference });
	await screenEdit({
		kind: 'ConfigureSurface',
		reference,
		definition: { ...snapshot.definition, source }
	});
}
const extent = { width: 64, height: 48 };
const sender = `MapGyver-Video-WebP-${randomUUID()}`;
const yellow = [255, 255, 0, 255];
let captures = 0;
async function pixels(label, color, tolerance = 2) {
	const result = await wait(
		async () => {
			const folder = path.join(directory, `pixels-${++captures}`);
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
			const code =
				'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGBA"); print(json.dumps([im.getpixel((16,24)),im.getpixel((48,24))]))';
			return JSON.parse(await run('python', ['-c', code, path.join(folder, 'pixels.png')]));
		},
		([foreground, background]) =>
			foreground.slice(0, 3).every((value, index) => Math.abs(value - color[index]) <= tolerance) &&
			foreground[3] === 255 &&
			background.every((value, index) => value === yellow[index]),
		label
	);
	evidence.push({ label, pixels: result, referenceForeground: color });
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label: 'WebP' });
	const source = async () => (await edit({ kind: 'InspectVideoSource', reference })).status;
	async function open(file, label, expectedDuration) {
		await edit({ kind: 'ConfigureVideoFile', reference, file });
		const status = await wait(
			source,
			(value) =>
				expectedDuration === undefined
					? value?.error?.message === 'Video stream duration is unavailable'
					: value?.duration_ms === expectedDuration && value.error === null,
			`${label} metadata`
		);
		evidence.push({ label, status });
		return status;
	}
	if (process.argv.includes('--baseline')) {
		for (const [file, label] of [
			[still, 'Still WebP'],
			[animated, 'Animated WebP']
		]) {
			assert.match((await open(file, label)).error?.message ?? '', /duration is unavailable/);
		}
	} else {
		await open(still, 'Still WebP', 40);
		const { reference: background } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Transparency witness'
		});
		await edit({
			kind: 'ConfigureChannel',
			reference: background,
			source: { kind: 'SolidColor', rgba: [1, 1, 0, 1] }
		});
		const { screen, surface } = await screenEdit({ kind: 'CreateScreen', label: 'WebP output' });
		await setSurface(surface, background);
		const { reference: foreground } = await screenEdit({ kind: 'AddSurface', screen });
		await setSurface(foreground, reference);
		await screenEdit({
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled: true, anti_aliasing: 'Raw', target: { kind: 'Spout', sender, extent } }
		});
		adapter = await observeNativeAdapter(
			native.page,
			JSON.parse(await run(sdk, ['--role', 'hardware']))
		);
		await wait(source, (status) => status.state === 'Ready', 'still source ready');
		await pixels('still WebP preserves alpha over the yellow surface', red);

		await open(animated, 'Animated WebP', 1200);
		await wait(source, (status) => status.state === 'Ready', 'animated source ready');
		const transport = async (command) =>
			(await edit({ kind: 'Video', media: reference.media, edit: command })).snapshot;
		const frame = async () => (await edit({ kind: 'InspectVideoFrame', reference })).frame;
		const playback = async () =>
			(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot;
		for (const [position, timestamp, color] of [
			[0, 0, red],
			[239, 0, red],
			[240, 240, green],
			[800, 800, blue],
			[1199, 800, blue],
			[240, 240, green]
		]) {
			await transport({ kind: 'Seek', position_ms: position });
			const result = await wait(
				frame,
				(value) => value?.position_ms === timestamp,
				`frame at ${position} ms`
			);
			evidence.push({ seekMs: position, frameMs: result.position_ms });
			await pixels(`animated WebP at ${position} ms`, color);
		}
		await transport({ kind: 'Loop', looping: true });
		await transport({ kind: 'Speed', speed: 0.25 });
		await transport({ kind: 'Seek', position_ms: 1100 });
		await pixels('last frame before wrap', blue);
		await transport({ kind: 'Play' });
		await wait(playback, (value) => value.playing && value.position_ms < 200, 'natural loop wrap');
		await pixels('first frame after natural wrap', red);
		await transport({ kind: 'Pause' });

		await open(lossyStill, 'Lossy still WebP', 40);
		await pixels(
			'lossy still matches the reference decoder over the yellow surface',
			lossyReference[0],
			3
		);
		await open(lossyAnimated, 'Lossy animated WebP', 1200);
		await transport({ kind: 'Seek', position_ms: 800 });
		await wait(frame, (value) => value?.position_ms === 800, 'lossy seek frame');
		await pixels(
			'lossy animated matches the reference decoder after seeking',
			lossyReference[1],
			3
		);
	}
} catch (error) {
	failure = String(error.stack ?? error);
	await writeFile(path.join(directory, 'app.log'), native.log());
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: failure ? 'fail' : 'pass', failure, adapter, evidence }, null, 2)
	);
}
console.log(JSON.stringify({ directory, evidence }));
