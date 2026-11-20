// Bundled Video subtitle controls and full-image independent SDK Screen captures.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';
import { windowCaptureSource } from './native-latency.mjs';

const requested = path.resolve(process.argv[2] ?? 'target/mapgyver-video-subtitles');
await mkdir(requested, { recursive: true });
const directory = await mkdtemp(path.join(requested, 'run-'));
const moving = process.argv.includes('--moving');
const selectedAdapter = process.env.MAPGYVER_VIDEO_ADAPTER;
const observer = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 12000 })).stdout;
const extent = { width: 160, height: 120 };
const embeddedFile = path.join(directory, 'embedded.srt');
const localFile = path.join(directory, 'local.srt');
const peerFile = path.join(directory, 'peer-embedded.srt');
const clip = path.join(directory, 'subtitle-black.mp4');
const peerClip = path.join(directory, 'subtitle-peer.mp4');
const videoLabel = 'Subtitle source';
const peerLabel = 'Second subtitle channel';
await writeFile(embeddedFile, '1\n00:00:00,500 --> 00:00:01,400\nÉTÉ\n\n', 'utf8');
await writeFile(peerFile, '1\n00:00:01,000 --> 00:00:01,600\nALT\n\n', 'utf8');
for (const [file, subtitle, language] of [
	[clip, embeddedFile, 'fra'],
	[peerClip, peerFile, 'eng']
]) {
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-y',
		'-f',
		'lavfi',
		'-i',
		'color=black:s=160x120:r=20:d=3',
		'-i',
		subtitle,
		'-map',
		'0:v:0',
		'-map',
		'1:s:0',
		'-c:v',
		'libx264',
		'-pix_fmt',
		'yuv420p',
		'-c:s',
		'mov_text',
		'-metadata:s:s:0',
		`language=${language}`,
		file
	]);
}

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
const errors = [],
	captures = [],
	playback = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token, adapter, video, failure, embeddedTrack, peerTrack;
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
const source = async (reference = video) =>
	(await edit({ kind: 'InspectVideoSource', reference })).status;
const inspect = (reference = video) => edit({ kind: 'InspectVideoSubtitles', reference });
const transport = async (operation) =>
	(await edit({ kind: 'Video', media: video.media, edit: operation })).snapshot;
const sample = async () => (await edit({ kind: 'InspectVideo', media: video.media })).snapshot;
const seek = (position_ms) => transport({ kind: 'Seek', position_ms });
async function subtitles(settings, reference = video) {
	const authored = await edit({ kind: 'ConfigureVideoSubtitles', reference, settings });
	assert.equal(authored.kind, 'VideoSubtitles');
	assert.deepEqual(authored.settings, settings);
}
async function subtitleInspector(reference) {
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const channel = nodes.find((node) => node.uuid === reference.channel);
	const parameters = nodes.find(
		(node) => channel.children.includes(node.node_id) && node.decl_id === 'parameters'
	);
	const subtitles = nodes.find(
		(node) =>
			parameters.children.includes(node.node_id) && node.decl_id === 'parameters/video_subtitles'
	);
	const row = page.locator(`.channels-section .node-inspector[data-node-id="${channel.node_id}"]`);
	const channelHeader = row.locator(':scope > .node-header > .node-title');
	if ((await channelHeader.getAttribute('aria-checked')) === 'false') {
		await channelHeader.press('Enter');
	}
	return row.locator(`.node-inspector[data-node-id="${subtitles.node_id}"]`);
}
async function wait(read, accept, label) {
	const deadline = performance.now() + 10000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(performance.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
async function output(reference, name) {
	const { screen, surface } = await edit({ kind: 'CreateScreen', label: name }, 'screens');
	const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
	const sender = `MapGyver-Video-Subtitles-${randomUUID()}`;
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
				target: { kind: 'Spout', sender, extent }
			}
		},
		'screens'
	);
	return sender;
}
const summarize = `
import hashlib, json, sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGBA")
width, height = im.size
pixels = list(im.getdata())
bright = [(i % width, i // width) for i, pixel in enumerate(pixels) if min(pixel[:3]) >= 200]
bounds = [min(x for x, _ in bright), min(y for _, y in bright),
          max(x for x, _ in bright), max(y for _, y in bright)] if bright else None
print(json.dumps(dict(bright=len(bright), bounds=bounds,
                      maximum=max(max(pixel[:3]) for pixel in pixels),
                      opaque=all(pixel[3] == 255 for pixel in pixels),
                      marker=im.getpixel((20, 20))[:3],
                      caption=hashlib.sha256(im.crop((0, 44, width, height)).tobytes()).hexdigest(),
                      sha256=hashlib.sha256(im.tobytes()).hexdigest())))
`;
async function capture(sender, label) {
	const root = path.join(directory, `capture-${String(captures.length + 1).padStart(3, '0')}`);
	const shape = JSON.parse(
		await run(observer, [
			'--role',
			'video-pixels',
			'--adapter',
			String(adapter.index),
			'--name',
			sender,
			'--directory',
			root
		])
	);
	assert.deepEqual(shape, { ...extent, retired: true });
	const file = path.join(root, 'pixels.png');
	const entry = { label, file, ...JSON.parse(await run('python', ['-c', summarize, file])) };
	captures.push(entry);
	return entry;
}
const image = (sender, label, accept) =>
	wait(
		() => capture(sender, label),
		(value) => value.opaque && accept(value),
		label
	);
const dark = (value) => value.maximum <= 4;
const centeredAt = (x, bottom) => (value) =>
	value.bright >= 8 &&
	Math.abs((value.bounds[0] + value.bounds[2]) / 2 - x) <= 12 &&
	value.bounds[3] <= bottom + 1 &&
	value.bounds[3] >= bottom - 16;

const markers = {
	head: ([r, g, b]) => r > 180 && g < 90 && b < 90,
	gap: ([r, g, b]) => r < 90 && g > 180 && b < 90,
	tail: ([r, g, b]) => r < 90 && g < 90 && b > 180,
	blend: ([r, g, b]) => r > 90 && g < 90 && b > 90,
	replacement: ([r, g, b]) => r > 180 && g > 180 && b < 90
};

async function movingClip(name, duration, language, headColor, captions) {
	const file = path.join(directory, `${name}.mp4`);
	const text = path.join(directory, `${name}.srt`);
	await writeFile(
		text,
		`1\n00:00:00,000 --> 00:00:02,400\n${captions[0]}\n\n` +
			`2\n00:00:03,400 --> 00:00:0${duration},000\n${captions[1]}\n`
	);
	// A picture marker identifies the source interval independently of the caption.
	const filter = [
		[headColor, 'lt(t,2.4)'],
		['lime', 'gte(t,2.4)*lt(t,3.4)'],
		['blue', 'gte(t,3.4)']
	]
		.map(([color, when]) => `drawbox=x=8:y=8:w=24:h=24:color=${color}:t=fill:enable='${when}'`)
		.join(',');
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-y',
		'-f',
		'lavfi',
		'-i',
		`color=black:s=160x120:r=20:d=${duration}`,
		'-i',
		text,
		'-vf',
		filter,
		'-map',
		'0:v:0',
		'-map',
		'1:s:0',
		'-c:v',
		'libx264',
		'-g',
		'10',
		'-pix_fmt',
		'yuv420p',
		'-c:s',
		'mov_text',
		'-metadata:s:s:0',
		`language=${language}`,
		file
	]);
	return file;
}

async function movingImage(label, [start, end], read, represent, direction = 1) {
	const result = await wait(
		async () => {
			const before = await sample();
			assert(before.playing, `${label}: Video must be playing`);
			if (before.position_ms < start || before.position_ms > end) {
				return { position: before.position_ms };
			}
			const frame = await read(label);
			const after = await sample();
			return {
				before: before.position_ms,
				after: after.position_ms,
				playing: after.playing,
				frame
			};
		},
		(value) =>
			value.playing &&
			value.after >= start &&
			value.after <= end &&
			direction * (value.after - value.before) > 0 &&
			represent(value.frame),
		label
	);
	playback.push({ label, ...result });
	return result;
}

async function nativePreviewReader() {
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview$/ })
		.click();
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
	assert(main?.visible && !main.minimized);
	let views = 0;
	return async () => {
		const box = await viewport.boundingBox();
		assert(
			box && Math.abs(box.width / box.height - 4 / 3) < 0.02,
			'native viewport must match the 4:3 Video'
		);
		const { width, height, dpr } = await page.evaluate(() => ({
			width: innerWidth,
			height: innerHeight,
			dpr: devicePixelRatio
		}));
		const source = windowCaptureSource({
			hwnd: main.hwnd,
			left: Math.round(box.x * dpr),
			top: Math.round(box.y * dpr),
			width: Math.round(box.width * dpr),
			height: Math.round(box.height * dpr),
			viewport: { width, height },
			dpr
		});
		const file = path.join(directory, `preview-${++views}.png`);
		await run('ffmpeg', [
			'-hide_banner',
			'-loglevel',
			'error',
			'-filter_complex',
			`${source},hwdownload,format=bgra,scale=${extent.width}:${extent.height}`,
			'-frames:v',
			'1',
			file
		]);
		return { file, ...JSON.parse(await run('python', ['-c', summarize, file])) };
	};
}

async function verifyMoving({
	direct,
	shared,
	peer,
	peerOutput,
	peerPixels,
	peerSettings,
	common
}) {
	const clip = await movingClip('moving', 6, 'deu', 'red', ['HEAD', 'TAIL']);
	const replacement = await movingClip('replacement', 8, 'ita', 'yellow', ['REPLACED', 'NEW TAIL']);
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: clip });
	await wait(
		source,
		(status) => status?.state === 'Ready' && status.duration_ms === 6000,
		'moving file'
	);
	await subtitles({ ...common, track: { kind: 'Embedded', index: 1 } });
	await transport({ kind: 'Crop', start_ms: 0, end_ms: null });
	const expected = {};
	for (const [phase, position] of [
		['head', 750],
		['gap', 2850],
		['tail', 4150]
	]) {
		await seek(position);
		expected[phase] = await image(
			direct,
			`held moving ${phase}`,
			(frame) =>
				markers[phase](frame.marker) &&
				(phase === 'gap' ? frame.bright === 0 : centeredAt(80, 108)(frame))
		);
	}
	assert.equal(new Set(Object.values(expected).map((frame) => frame.caption)).size, 3);
	const onDirect = (label) => capture(direct, label);
	const match =
		(phase, marker = phase) =>
		(frame) =>
			frame.opaque && markers[marker](frame.marker) && frame.caption === expected[phase].caption;
	for (const [range, start, end, length] of [
		['full', 0, null, 6000],
		['crop', 600, 5000, 4400]
	]) {
		await transport({ kind: 'Pause' });
		assert.equal(
			(await transport({ kind: 'Crop', start_ms: start, end_ms: end })).length_ms,
			length
		);
		await transport({ kind: 'Loop', looping: true });
		for (const overlap of [0, 1000]) {
			await transport({ kind: 'Pause' });
			const speed = overlap ? 1 : 1.25;
			await transport({ kind: 'Speed', speed });
			await transport({ kind: 'AutoSmoothLoop', duration_ms: overlap });
			await seek(length - 1600);
			await transport({ kind: 'Play' });
			const label = `${range} ${overlap ? 'smooth' : 'hard'} loop`;
			const tail = await movingImage(
				`${label} tail before wrap`,
				[length - (overlap ? 900 : 1350), length - 100],
				onDirect,
				match('tail', overlap ? 'blend' : 'tail')
			);
			const head = await movingImage(
				`${label} head after wrap`,
				[overlap + 100, overlap + 725],
				onDirect,
				match('head')
			);
			assert(head.after < tail.before, `${label} must cross the loop point`);
		}
	}
	await transport({ kind: 'Pause' });
	await transport({ kind: 'Crop', start_ms: 0, end_ms: null });
	await transport({ kind: 'Loop', looping: false });
	await transport({ kind: 'Speed', speed: -1.25 });
	await seek(5000);
	await transport({ kind: 'Play' });
	for (const [phase, range] of [
		['tail', [3600, 4900]],
		['gap', [2450, 3310]],
		['head', [900, 2280]]
	]) {
		await movingImage(`reverse ${phase}`, range, onDirect, match(phase), -1);
	}

	await transport({ kind: 'Pause' });
	await transport({ kind: 'Speed', speed: 0.75 });
	await transport({ kind: 'Loop', looping: true });
	await transport({ kind: 'AutoSmoothLoop', duration_ms: 0 });
	await seek(225);
	await transport({ kind: 'Play' });
	const before = await movingImage(
		'moving Reference shows the original cue',
		[225, 1900],
		(label) => capture(shared, label),
		match('head')
	);
	const beforeRelink = await sample();
	await edit({ kind: 'ConfigureVideoFile', reference: video, file: replacement });
	const afterRelink = await sample();
	assert(afterRelink.playing && afterRelink.position_ms >= beforeRelink.position_ms);
	playback.push({
		label: 'relink transport',
		before: beforeRelink.position_ms,
		after: afterRelink.position_ms
	});
	const after = await movingImage(
		'moving replacement uses its own embedded cue',
		[225, 2220],
		onDirect,
		(frame) =>
			markers.replacement(frame.marker) &&
			centeredAt(80, 108)(frame) &&
			frame.caption !== expected.head.caption
	);
	assert(after.before >= before.after, 'live file replacement must preserve play position');
	assert.equal((await source()).duration_ms, 8000);
	assert.match((await inspect()).tracks[0].label, /^ita\b/);
	assert.deepEqual((await inspect(peer)).settings, peerSettings);
	await movingImage(
		'shorter sibling keeps its own cue after live replacement',
		[1010, 1580],
		(label) => capture(peerOutput, label),
		(frame) => frame.sha256 === peerPixels.sha256
	);
	await transport({ kind: 'Pause' });
	if (process.argv.includes('--native-preview')) {
		const readPreview = await nativePreviewReader();
		await seek(600);
		await transport({ kind: 'Play' });
		await movingImage(
			'moving native Media Preview caption',
			[650, 2100],
			readPreview,
			(frame) => markers.replacement(frame.marker) && centeredAt(80, 108)(frame)
		);
		await movingImage(
			'moving native Media Preview cue gap',
			[2540, 3270],
			readPreview,
			(frame) => markers.gap(frame.marker) && frame.bright === 0
		);
		await transport({ kind: 'Pause' });
	}
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
	const direct = await output(video, 'Subtitle direct output');
	adapter = await observeNativeAdapter(page, hardware);
	if (selectedAdapter) assert.equal(adapter.key, selectedAdapter);
	await wait(source, (status) => status?.state === 'Ready', 'black Video file is decoded');
	await image(direct, 'subtitles disabled by default', dark);
	const initial = await inspect();
	assert.equal(initial.kind, 'VideoSubtitles');
	assert.equal(initial.settings.enabled, false);
	assert.deepEqual(initial.settings.track, { kind: 'None' });
	({
		tracks: [embeddedTrack]
	} = await wait(
		inspect,
		(result) => result.tracks.length === 1,
		'embedded text track is discovered'
	));
	assert.equal(embeddedTrack.index, 1, 'exact muxed FFmpeg stream index');
	assert(embeddedTrack.label.length > 0);

	const crop = await transport({ kind: 'Crop', start_ms: 500, end_ms: 2500 });
	assert.equal(crop.length_ms, 2000);
	const common = { enabled: true, file: '', size: 18, font: 'Segoe UI', position: [0.5, 0.9] };
	const standardPlacement = centeredAt(80, 108);
	await subtitles({ ...common, track: { kind: 'Embedded', index: embeddedTrack.index } });
	await seek(0);
	const embedded = await image(
		direct,
		'cropped first frame uses the same source-time cue',
		standardPlacement
	);
	await seek(900);
	await image(direct, 'embedded cue end is exclusive at cropped position 900', dark);
	await seek(600);
	await image(
		direct,
		'held embedded cue restores after seek',
		(value) => value.sha256 === embedded.sha256
	);

	const local = { ...common, track: { kind: 'File' }, file: localFile };
	await subtitles(local);
	await image(direct, 'missing local file clears the former embedded cue', dark);
	assert.equal((await source()).state, 'Ready', 'missing captions preserve the Video');
	await writeFile(localFile, '1\n00:00:00,900 --> 00:00:02,100\nÉTÉ LOCAL\n\n', 'utf8');
	const recovered = await image(
		direct,
		'missing SRT recovers while held, without reselecting',
		standardPlacement
	);
	assert.notEqual(recovered.sha256, embedded.sha256);
	await seek(399);
	await image(direct, 'local cue absent immediately before its source start', dark);
	await seek(400);
	await image(
		direct,
		'local cue included exactly at its source start',
		(value) => value.sha256 === recovered.sha256
	);
	await seek(1600);
	await image(direct, 'local cue excluded exactly at its source end', dark);
	await seek(600);
	await image(
		direct,
		'local cue restores after boundary seeks',
		(value) => value.sha256 === recovered.sha256
	);

	const positioned = { ...local, size: 12, font: 'Courier New', position: [0.32, 0.55] };
	await subtitles(positioned);
	const changed = await image(
		direct,
		'authored font, smaller size and upper-left position',
		centeredAt(51, 66)
	);
	assert(changed.bounds[3] - changed.bounds[1] < recovered.bounds[3] - recovered.bounds[1]);

	const { reference: peer } = await edit({
		kind: 'AddChannel',
		media: video.media,
		label: peerLabel
	});
	await edit({ kind: 'ConfigureVideoFile', reference: peer, file: peerClip });
	const peerOutput = await output(peer, 'Second channel subtitle output');
	await wait(
		() => source(peer),
		(status) => status?.state === 'Ready',
		'second Video file is decoded'
	);
	const peerInitial = await wait(
		() => inspect(peer),
		(result) => result.tracks.length === 1,
		'second channel discovers its own embedded track'
	);
	[peerTrack] = peerInitial.tracks;
	assert.equal(peerTrack.index, 1);
	assert.notEqual(peerTrack.label, embeddedTrack.label);
	assert.deepEqual((await inspect()).tracks, [embeddedTrack]);
	assert.equal(peerInitial.settings.enabled, false);
	assert.deepEqual(peerInitial.settings.track, { kind: 'None' });
	await image(peerOutput, 'second channel is dark until its own track is enabled', dark);
	const peerSettings = {
		...common,
		track: { kind: 'Embedded', index: peerTrack.index },
		size: 16,
		position: [0.7, 0.75]
	};
	await subtitles({ ...peerSettings, track: { kind: 'None' } }, peer);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: videoLabel, exact: true })
		.click();
	const peerInspector = await subtitleInspector(peer);
	const peerHeader = peerInspector.locator(':scope > .node-header > .node-title');
	if ((await peerHeader.getAttribute('aria-checked')) === 'false') {
		await peerHeader.press('Enter');
	}
	const peerTrackChoice = peerInspector
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('Track', { exact: true }) })
		.getByRole('combobox');
	assert.equal(await peerTrackChoice.inputValue(), 'None');
	assert.equal(await peerTrackChoice.getByRole('option', { name: embeddedTrack.label }).count(), 0);
	await peerTrackChoice.selectOption({ label: peerTrack.label });
	await wait(
		() => inspect(peer),
		(value) => value.settings.track.kind === 'Embedded',
		'Inspector selects the second channel embedded track'
	);
	assert.deepEqual((await inspect(peer)).settings, peerSettings);
	const peerPixels = await image(
		peerOutput,
		'second channel uses the caption and placement of its own embedded file',
		centeredAt(112, 90)
	);
	assert.notEqual(peerPixels.sha256, changed.sha256);
	assert.deepEqual((await inspect()).settings, positioned);
	await image(
		direct,
		'editing the second channel preserves the first channel pixels',
		(value) => value.sha256 === changed.sha256
	);

	const { media: mirror } = await edit({
		kind: 'CreateReference',
		target: video.media,
		label: 'Subtitled Video Reference'
	});
	await edit({ kind: 'SelectReferenceChannel', media: mirror, channel: video.channel });
	const channels = (
		await edit({ kind: 'InspectChannels', query: { media: mirror, start: 0, count: 1 } })
	).page.channels;
	assert.equal(channels.length, 1);
	assert.deepEqual(channels[0].definition.source, { kind: 'Reference', target: video });
	const shared = await output(channels[0].reference, 'Subtitle Reference output');
	await image(
		shared,
		'Reference sees the exact same subtitle pixels',
		(value) => value.sha256 === changed.sha256
	);

	const firstInspector = await subtitleInspector(video);
	const switchControl = firstInspector
		.locator(':scope > .node-header')
		.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	await wait(
		() => switchControl.getAttribute('aria-checked'),
		(value) => value === 'true',
		'Inspector folder enabled'
	);
	await switchControl.click();
	await wait(inspect, (value) => !value.settings.enabled, 'generic Inspector disabled Subtitles');
	await image(direct, 'Inspector disable bypasses subtitle overlay', dark);
	await image(shared, 'Inspector disable also clears the Reference overlay', dark);
	assert.deepEqual((await inspect(peer)).settings, peerSettings);
	await image(
		peerOutput,
		'disabling the first channel preserves the second channel caption',
		(value) => value.sha256 === peerPixels.sha256
	);
	await switchControl.click();
	await wait(inspect, (value) => value.settings.enabled, 'generic Inspector re-enabled Subtitles');
	await image(
		direct,
		'Inspector re-enable restores the same held cue',
		(value) => value.sha256 === changed.sha256
	);
	assert.deepEqual((await inspect()).settings, positioned);
	if (moving)
		await verifyMoving({ direct, shared, peer, peerOutput, peerPixels, peerSettings, common });
	assert.deepEqual(errors, []);
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
					appLog: failure ? native.log() : undefined,
					adapter,
					embeddedTrack,
					peerTrack,
					captures,
					playback,
					errors
				},
				null,
				2
			)
		);
	}
}
console.log(
	`Video subtitle smoke passed on ${adapter.description}. ${path.join(directory, 'result.json')}`
);
