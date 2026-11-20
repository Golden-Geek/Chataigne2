// Bundled Inspector and transport with independently decoded Screen Spout pixels.
import assert from 'node:assert/strict';
import { execFile, spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';
import { windowCaptureSource } from './native-latency.mjs';

const directory = path.resolve(process.argv[2] ?? `target/mapgyver-video-time-crop-${Date.now()}`);
const moving = process.argv.includes('--moving');
const selectedAdapter = process.env.MAPGYVER_VIDEO_ADAPTER;
const observer = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 12000 })).stdout;
const extent = { width: 160, height: 120 };
const frameMs = 100;
const frameCount = 16;
const start = 400;
const end = 1200;
const length = end - start;
const overlap = 200;
const firstCode = start / frameMs;
const lastCode = end / frameMs - 1;
const marker = { bits: 4, origin: [24, 20], step: 24, sentinel: [4, 20], threshold: 128 };
const videoLabel = 'Numbered Video time crop';
const clip = path.join(directory, 'numbered-time-crop.mp4');
await mkdir(directory, { recursive: true });
const filters = [
	"geq=lum='16+14*N':cb=128:cr=128",
	'drawbox=x=0:y=0:w=160:h=28:color=black:t=fill',
	'drawbox=x=0:y=12:w=8:h=16:color=white:t=fill',
	...Array.from(
		{ length: marker.bits },
		(_, bit) =>
			`drawbox=x=${16 + marker.step * bit}:y=12:w=16:h=16:color=white:t=fill:enable='bitand(n,${1 << bit})'`
	)
];
async function makeClip(file, count) {
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-y',
		'-f',
		'lavfi',
		'-i',
		'nullsrc=s=160x120:r=10',
		'-vf',
		filters.join(','),
		'-frames:v',
		String(count),
		'-an',
		'-c:v',
		'libx264',
		'-crf',
		'12',
		'-g',
		'10',
		'-pix_fmt',
		'yuv420p',
		'-color_range',
		'tv',
		'-colorspace',
		'bt709',
		'-color_trc',
		'bt709',
		file
	]);
}
await makeClip(clip, frameCount);

const hardware = JSON.parse(await run(observer, ['--role', 'hardware']));
const native = await openNativeEditor({
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
	seams = {},
	playback = [];
let token, adapter, video, failure, preview;
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
const source = async (reference = video) =>
	(await edit({ kind: 'InspectVideoSource', reference })).status;
const frame = async () => (await edit({ kind: 'InspectVideoFrame', reference: video })).frame;
const transport = async (operation) =>
	(await edit({ kind: 'Video', media: video.media, edit: operation })).snapshot;
async function wait(read, accept, label, milliseconds = 8000) {
	const deadline = performance.now() + milliseconds;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(performance.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
const parameter = (label) =>
	page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
async function setTime(label, text, expected) {
	const input = parameter(label).locator('input.time-field');
	await input.fill(text);
	await input.press('Enter');
	await wait(inspect, expected, `generic Inspector ${label} ${text}`);
}
async function output(reference, name) {
	const { screen, surface } = await edit({ kind: 'CreateScreen', label: name }, 'screens');
	const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
	const sender = `MapGyver-Video-Time-Crop-${randomUUID()}`;
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
				anti_aliasing: 'Raw',
				frame_rate: { kind: 'Explicit', fps: 60 },
				target: { kind: 'Spout', sender, extent }
			}
		},
		'screens'
	);
	return sender;
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
	const file = path.join(folder, 'pixels.png');
	const observation = JSON.parse(
		await run('python', [
			'-c',
			[
				'import json,sys',
				'from PIL import Image',
				'im=Image.open(sys.argv[1]).convert("RGBA")',
				'm=json.loads(sys.argv[2])',
				'opaque=lambda p:p[3]==255 and sum(p[:3])/3>m["threshold"]',
				'x,y=m["origin"]',
				'valid=opaque(im.getpixel(tuple(m["sentinel"])))',
				'code=sum(opaque(im.getpixel((x+m["step"]*i,y)))<<i for i in range(m["bits"])) if valid else None',
				'print(json.dumps(dict(code=code,rgba=[im.getpixel(p) for p in [(40,30),(80,60),(120,90)]])))'
			].join(';'),
			file,
			JSON.stringify(marker)
		])
	);
	const entry = { label, ...observation, file };
	captures.push(entry);
	return entry;
}
const captured = (sender, label, predicate) =>
	wait(
		() => spout(sender, label),
		(value) => value.rgba.every((sample) => sample[3] === 255) && predicate(value),
		label
	);
const number = (code) => (value) => value.code === code;
const linear = (byte) => {
	const encoded = byte / 255;
	return encoded <= 0.04045 ? encoded / 12.92 : ((encoded + 0.055) / 1.055) ** 2.4;
};
const smoothstep = (x) => x * x * (3 - 2 * x);
const matchesBlend =
	(tail, head, lower, upper = lower) =>
	(value) =>
		value.code !== null &&
		value.rgba.every((sample, i) =>
			sample.slice(0, 3).every((channel, c) => {
				const outgoing = linear(tail.rgba[i][c]);
				const difference = linear(head.rgba[i][c]) - outgoing;
				const a = outgoing + lower * difference;
				const b = outgoing + upper * difference;
				const observed = linear(channel);
				return observed >= Math.min(a, b) - 0.025 && observed <= Math.max(a, b) + 0.025;
			})
		);
async function step(direction, expected) {
	await wait(frame, (value) => value?.previous_ready, 'step neighbour window');
	const next = await transport({ kind: 'Step', channel: video.channel, direction });
	assert.equal(next.playing, false);
	assert.equal(next.position_ms, expected);
}
async function wraps(expectedResume, label, count = 2, direction = 1) {
	const previous = [];
	let before = (await transport({ kind: 'Play' })).position_ms;
	const deadline = performance.now() + 6500;
	while (previous.length < count) {
		const next = await inspect();
		assert(next.playing && next.settings.looping);
		assert(
			next.position_ms >= 0 &&
				(direction > 0 ? next.position_ms < length : next.position_ms <= length)
		);
		if (direction * (before - next.position_ms) > length / 2) {
			assert(
				direction > 0 ? before >= length - frameMs : before <= frameMs,
				`${label}: early wrap at ${before} ms`
			);
			const advanced = direction * (next.position_ms - expectedResume);
			assert(advanced >= 0 && advanced < frameMs, `${label}: resumed at ${next.position_ms} ms`);
			previous.push({ before, after: next.position_ms });
		}
		before = next.position_ms;
		assert(performance.now() < deadline, `${label}: ${count} wraps required`);
		await delay(15);
	}
	return previous;
}
function assertCycle(codes, direction, label) {
	const cycle = Array.from({ length: lastCode - firstCode + 1 }, (_, index) =>
		direction > 0 ? firstCode + index : lastCode - index
	);
	assert(
		codes.length >= cycle.length * 2 + 1,
		`${label}: two complete visible hard loops: ${codes}`
	);
	const offset = cycle.indexOf(codes[0]);
	assert(offset >= 0, `${label}: initial observed frame is inside the crop`);
	assert.deepEqual(
		codes,
		Array.from({ length: codes.length }, (_, index) => cycle[(offset + index) % cycle.length]),
		label
	);
}
async function hardLoop(sender, direction = 1) {
	const label = direction > 0 ? 'hard' : 'reverse';
	const root = path.join(directory, `${label}-loop-sdk`);
	const trace = path.join(root, 'consumer.jsonl');
	await mkdir(root);
	const child = spawn(
		observer,
		[
			'--role',
			'consumer',
			'--adapter',
			String(adapter.index),
			'--name',
			sender,
			'--width',
			String(extent.width),
			'--height',
			String(extent.height),
			'--instance',
			randomUUID(),
			'--directory',
			root,
			'--max-seconds',
			'30',
			'--max-frames',
			'1024',
			'--binary-frame-code',
			JSON.stringify(marker)
		],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	let stdout = '',
		stderr = '';
	child.stdout.on('data', (value) => (stdout += value));
	child.stderr.on('data', (value) => (stderr = (stderr + value).slice(-8192)));
	child.stdin.on('error', () => {});
	const closed = new Promise((resolve) => {
		child.once('error', (error) => {
			stderr += String(error);
			resolve(-1);
		});
		child.once('close', resolve);
	});
	try {
		await wait(
			async () => {
				assert(child.pid && child.exitCode === null, stderr);
				return stdout;
			},
			(value) => value.includes('"kind":"ready"'),
			'independent SDK receiver'
		);
		// SDK registration precedes the first acquired image. Allow one partial
		// attachment cycle and require two complete cycles in admitted pixels below.
		preview?.startSequence();
		seams[label] = await wraps(direction > 0 ? 0 : length, `${label} loop`, 3, direction);
		// Ensure the head of the final cycle reaches the independent SDK consumer.
		await delay(120);
		await transport({ kind: 'Pause' });
		if (preview) {
			const previewCodes = preview.finishSequence();
			seams[`${label}PreviewCodes`] = previewCodes;
			assertCycle(previewCodes, direction, `${label} Media Preview`);
		}
	} finally {
		if (child.exitCode === null) child.stdin.end('stop\n');
		let timeout;
		try {
			assert.equal(
				await Promise.race([
					closed,
					new Promise((resolve) => {
						timeout = setTimeout(() => resolve(-1), 8000);
					})
				]),
				0,
				stderr
			);
		} finally {
			clearTimeout(timeout);
			if (child.exitCode === null) child.kill();
		}
	}
	const rows = (await readFile(trace, 'utf8')).trim().split('\n').map(JSON.parse);
	const final = rows.find((row) => row.kind === 'consumer_final').consumer;
	for (const field of ['failed_receives', 'mismatch', 'overflow'])
		assert.equal(final[field], 0, field);
	const codes = [];
	for (const { frame } of rows.filter((row) => row.kind === 'consumer_frame')) {
		const code = frame.binary_frame_code.Ok;
		assert(Number.isInteger(code) && frame.spout_image, JSON.stringify(frame));
		if (codes.at(-1) !== code) codes.push(code);
	}
	seams[`${label}Codes`] = codes;
	assertCycle(codes, direction, `${label} Spout output`);
}

async function observePreview() {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: videoLabel, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview$/ })
		.click();
	const viewport = page.locator(
		'.media-preview-panel .native-preview[data-preview-installed="true"]'
	);
	await viewport.waitFor();
	const box = await viewport.boundingBox();
	assert(box && Math.abs(box.width / box.height - extent.width / extent.height) < 0.02);
	const [owner] = JSON.parse(
		await run('python', [
			'apps/mapgyver/tests/native_output/window_state.py',
			String(native.app.pid),
			'--main'
		])
	);
	assert(owner?.visible && !owner.minimized);
	const layout = await page.evaluate(() => ({
		width: innerWidth,
		height: innerHeight,
		dpr: devicePixelRatio
	}));
	const { dpr } = layout;
	const input = windowCaptureSource({
		hwnd: owner.hwnd,
		left: Math.round(box.x * dpr),
		top: Math.round(box.y * dpr),
		width: Math.round(box.width * dpr),
		height: Math.round(box.height * dpr),
		viewport: layout,
		dpr
	});
	const child = spawn(
		'ffmpeg',
		[
			'-hide_banner',
			'-loglevel',
			'error',
			'-filter_complex',
			`${input},hwdownload,format=bgra,scale=${extent.width}:${extent.height}`,
			'-fps_mode',
			'passthrough',
			'-frames:v',
			'4096',
			'-f',
			'rawvideo',
			'-pix_fmt',
			'rgba',
			'pipe:1'
		],
		{ windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] }
	);
	let serial = 0,
		latest,
		sequence,
		stderr = '',
		buffer = Buffer.alloc(0);
	const complete = new Promise((resolve) => child.once('close', resolve));
	child.on('error', (error) => (stderr += String(error)));
	child.stdin.on('error', () => {});
	child.stderr.on('data', (data) => (stderr = (stderr + data).slice(-8192)));
	const frameBytes = extent.width * extent.height * 4;
	child.stdout.on('data', (data) => {
		buffer = Buffer.concat([buffer, data]);
		while (buffer.length >= frameBytes) {
			const read = (x, y) =>
				Array.from(buffer.subarray((y * extent.width + x) * 4, (y * extent.width + x) * 4 + 4));
			const white = (x, y) => {
				const [r, g, b, a] = read(x, y);
				return a === 255 && (r + g + b) / 3 > marker.threshold;
			};
			const [x, y] = marker.origin;
			let code = null;
			if (white(...marker.sentinel)) {
				code = 0;
				for (let bit = 0; bit < marker.bits; bit++)
					if (white(x + marker.step * bit, y)) code |= 1 << bit;
			}
			latest = { code, serial: ++serial, rgba: [read(40, 30), read(80, 60), read(120, 90)] };
			if (sequence && sequence.at(-1) !== code) sequence.push(code);
			buffer = buffer.subarray(frameBytes);
		}
	});
	return {
		latest() {
			assert(child.pid && child.exitCode === null, `preview capture exited: ${stderr}`);
			return latest;
		},
		startSequence() {
			sequence = [this.latest()?.code ?? null];
		},
		finishSequence() {
			const result = sequence;
			sequence = null;
			return result;
		},
		async close() {
			if (child.exitCode === null) child.stdin.end('q\n');
			const deadline = setTimeout(() => child.kill(), 4500);
			try {
				assert.equal(await complete, 0, stderr);
			} finally {
				clearTimeout(deadline);
			}
		}
	};
}

async function movingWitness(label, [first, last], observe, accepts) {
	const witness = await wait(
		async () => {
			const before = await inspect();
			assert(before.playing, label);
			if (before.position_ms < first || before.position_ms > last)
				return { positionMs: before.position_ms };
			const image = await observe(label);
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
			value.afterMs > value.beforeMs &&
			value.afterMs <= last &&
			accepts(value.image),
		label
	);
	playback.push({ label, ...witness });
}

async function movingSmoothPreview(direct, tail, head) {
	const playOverlap = async () => {
		await transport({ kind: 'Pause' });
		await transport({ kind: 'Seek', position_ms: 715 });
		await transport({ kind: 'Play' });
	};
	// A single SDK capture opens a receiver and performs a CPU readback. Give it
	// enough wall time to finish within the same pair of 100 ms media frames.
	await transport({ kind: 'Speed', speed: 0.1 });
	await playOverlap();
	const inOverlap = matchesBlend(tail, head, 0.56, 0.99);
	const consumedHead = matchesBlend(tail, head, 1);
	const blended = (image) => inOverlap(image) && !consumedHead(image);
	await movingWitness(
		'native preview uses moving cropped overlap',
		[725, 792],
		async () => {
			const previous = preview.latest()?.serial ?? 0;
			return wait(
				() => preview.latest(),
				(value) => value?.serial > previous,
				'new window capture'
			);
		},
		blended
	);
	await playOverlap();
	await movingWitness(
		'Spout uses moving cropped overlap',
		[725, 792],
		(label) => spout(direct, label),
		blended
	);
	await transport({ kind: 'Speed', speed: 0.5 });
}

async function mixedAndRelink(direct, shared) {
	const shorterFile = path.join(directory, 'shorter-cropped-channel.mp4');
	const endsBeforeStart = path.join(directory, 'ends-before-start.mp4');
	await makeClip(shorterFile, 9);
	await makeClip(endsBeforeStart, 3);
	const { reference: shorter } = await edit({
		kind: 'AddChannel',
		media: video.media,
		label: 'Nine frames'
	});
	await edit({ kind: 'ConfigureVideoFile', reference: shorter, file: shorterFile });
	const peer = await output(shorter, 'Time crop short channel output');
	await wait(
		() => source(shorter),
		(value) => value.state === 'Ready' && value.duration_ms === 900,
		'short peer ready'
	);
	assert.equal((await inspect()).length_ms, length);
	await transport({ kind: 'AutoSmoothLoop', duration_ms: 0 });
	await transport({ kind: 'Speed', speed: 0.25 });
	await transport({ kind: 'Seek', position_ms: length });
	await captured(peer, 'short channel holds its own last frame at crop End', number(8));
	await captured(direct, 'long channel keeps its own last in-crop frame', number(lastCode));
	await transport({ kind: 'Seek', position_ms: 535 });
	await transport({ kind: 'Play' });
	await movingWitness(
		'shorter native channel holds while the shared clock advances',
		[515, 775],
		(label) => spout(peer, label),
		number(8)
	);
	await edit({ kind: 'RemoveChannel', reference: shorter });
	const playing = await inspect();
	assert(playing.playing && playing.length_ms === length);
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: endsBeforeStart });
	await wait(
		source,
		(value) => value.duration_ms === 300,
		'replacement metadata ends before Start'
	);
	const empty = await wait(
		inspect,
		(value) => !value.ready && !value.playing && value.position_ms === 0 && value.length_ms === 0,
		'relink outside the authored range suspends playback'
	);
	assert.equal(empty.settings.start_ms, start);
	assert.equal(empty.settings.end_ms, end);
	playback.push({ label: 'replacement outside crop preserves authored bounds', state: empty });
	for (const [route, sender] of [
		['direct', direct],
		['Reference', shared]
	]) {
		const label = `${route} clears the invalid source range`;
		await wait(
			() => spout(sender, label),
			(value) =>
				value.code === null && value.rgba.every((rgba) => rgba.every((byte) => byte === 0)),
			label
		);
	}
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: clip });
	await wait(source, (value) => value.state === 'Ready', 'restored decoder');
	const restored = await wait(
		inspect,
		(value) => value.length_ms === length && value.ready,
		'original range restored'
	);
	assert.equal(restored.settings.start_ms, start);
	assert.equal(restored.settings.end_ms, end);
	await transport({ kind: 'Stop' });
	await captured(direct, 'restored direct output starts at the authored Start', number(firstCode));
	await captured(shared, 'restored Reference starts at the authored Start', number(firstCode));
}

try {
	token = (await post('/api/ui/session', {})).token;
	await page.waitForURL(`${native.base}/**`);
	({ reference: video } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: videoLabel
	}));
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: clip });
	const direct = await output(video, 'Time crop direct output');
	adapter = await observeNativeAdapter(page, hardware);
	if (selectedAdapter) assert.equal(adapter.key, selectedAdapter);
	await wait(
		source,
		(status) => status.state === 'Ready' && status.duration_ms === frameMs * frameCount,
		'numbered source ready'
	);
	await wait(inspect, (value) => value.length_ms === frameMs * frameCount, 'default full source');
	await captured(direct, 'uncropped frame zero', number(0));
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: videoLabel, exact: true })
		.click();
	await setTime('Start', '00:00:00.400', (value) => value.settings.start_ms === start);
	await setTime('End', '00:00:01.200', (value) => value.settings.end_ms === end);
	await wait(
		() => parameter('Length').locator('input.time-field').getAttribute('readonly'),
		(value) => value !== null,
		'read-only generic Inspector Length'
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(inspect, (value) => value.length_ms === frameCount * frameMs - start, 'Undo crop End');
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(inspect, (value) => value.length_ms === length, 'Redo crop End');

	assert.equal((await transport({ kind: 'Stop' })).position_ms, 0);
	await captured(direct, 'crop Stop is first in-range frame', number(firstCode));
	await setTime('Current Position', '00:00:00.900', (value) => value.position_ms === length);
	const tail = await captured(direct, 'exact end is last in-range frame', number(lastCode));
	assert.equal((await inspect()).position_ms, length);
	await transport({ kind: 'Seek', position_ms: 0 });
	await captured(direct, 'crop-relative zero', number(firstCode));
	await step('Previous', 0);
	await step('Next', frameMs);
	const head = await captured(direct, 'step into second crop frame', number(firstCode + 1));
	await transport({ kind: 'Seek', position_ms: length - frameMs });
	await captured(direct, 'last frame stays below exclusive End', number(lastCode));
	await step('Next', length - frameMs);
	await step('Previous', length - 2 * frameMs);

	await transport({ kind: 'Stop' });
	await transport({ kind: 'Play' });
	await wait(
		inspect,
		(value) => !value.playing && !value.settings.looping && value.position_ms === length,
		'Loop off stops at the crop end'
	);
	await captured(direct, 'non-looping final image cannot show excluded frame', number(lastCode));

	const { media } = await edit({
		kind: 'CreateReference',
		target: video.media,
		label: 'Time crop Reference'
	});
	const mirrors = (await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 1 } }))
		.page.channels;
	assert.equal(mirrors.length, 1);
	const shared = await output(mirrors[0].reference, 'Time crop Reference output');
	const mirrorTail = await captured(shared, 'crop endpoint through Reference', number(lastCode));
	assert.deepEqual(mirrorTail.rgba, tail.rgba);

	// Natural completion can leave Play requested. Hold explicitly before enabling Loop.
	await transport({ kind: 'Pause' });
	await transport({ kind: 'Loop', looping: true });
	await transport({ kind: 'Speed', speed: 0.5 });
	await transport({ kind: 'Seek', position_ms: 0 });
	await captured(direct, 'hard-loop warmup starts at cropped head', number(firstCode));
	if (moving) {
		preview = await observePreview();
		await wait(
			() => preview.latest(),
			(value) => value?.code === firstCode,
			'native preview shows cropped head'
		);
	}
	await hardLoop(direct);

	await transport({ kind: 'AutoSmoothLoop', duration_ms: overlap });
	const phase = 0.625;
	const nearBlend = matchesBlend(tail, head, smoothstep(phase));
	await transport({ kind: 'Seek', position_ms: length - overlap + phase * overlap });
	const blend = await captured(direct, 'held smooth mix uses cropped head and tail', nearBlend);
	const sharedBlend = await captured(
		shared,
		'Reference observes the identical held smooth mix',
		nearBlend
	);
	assert.deepEqual(sharedBlend.rgba, blend.rgba);
	if (moving) await movingSmoothPreview(direct, tail, head);
	seams.smooth = await wraps(overlap, 'smooth loop');
	if (moving) {
		await transport({ kind: 'Pause' });
		await transport({ kind: 'AutoSmoothLoop', duration_ms: 0 });
		await transport({ kind: 'Speed', speed: -0.5 });
		await transport({ kind: 'Seek', position_ms: length });
		await wait(
			() => preview.latest(),
			(value) => value?.code === lastCode,
			'reverse starts from the last in-range frame'
		);
		await hardLoop(direct, -1);
		const finishedPreview = preview;
		preview = null;
		await finishedPreview.close();
		await transport({ kind: 'Speed', speed: 0.5 });
		await transport({ kind: 'Seek', position_ms: 250 });
		await transport({ kind: 'Play' });
	}

	const edited = await transport({ kind: 'Crop', start_ms: 500, end_ms: 1000 });
	assert(edited.playing && edited.position_ms < 500);
	assert.equal(edited.length_ms, 500);
	await transport({ kind: 'Stop' });
	await captured(direct, 'live typed crop moves Stop to its new start', number(5));
	await transport({ kind: 'Crop', start_ms: 300, end_ms: null });
	await wait(inspect, (value) => value.length_ms === 1300, 'open End uses source duration');
	await transport({ kind: 'Stop' });
	await captured(direct, 'open End crop moves Stop to its source Start', number(3));

	await transport({ kind: 'Crop', start_ms: start, end_ms: end });
	const project = path.join(directory, 'cropped.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	const loaded = await post('/api/ui/project-load', { path: project });
	assert.equal(path.resolve(loaded.path), path.resolve(project));
	await wait(inspect, (value) => value.length_ms === length, 'reload restores the crop range');
	const reloaded = await inspect();
	assert.equal(reloaded.settings.start_ms, start);
	assert.equal(reloaded.settings.end_ms, end);
	assert.equal(reloaded.position_ms, 0);
	await captured(shared, 'reloaded Reference displays the crop start', number(firstCode));
	if (moving) await mixedAndRelink(direct, shared);
	assert.deepEqual(pageErrors, []);
} catch (error) {
	failure = String(error);
	throw error;
} finally {
	try {
		try {
			await preview?.close();
		} finally {
			await native.close();
		}
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
					adapter,
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
	`Video time crop passed on ${adapter.description}. ${path.join(directory, 'result.json')}`
);
