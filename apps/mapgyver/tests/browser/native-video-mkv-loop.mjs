// A moving HEVC/Opus/SubRip MKV through live track changes and native cropped loops.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';

const root = path.resolve(process.argv[2] ?? 'target/m103-mkv-loop');
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const file = path.join(directory, 'loop.mkv');
const english = path.join(directory, 'english.srt');
const french = path.join(directory, 'french.srt');
await Promise.all([
	writeFile(
		english,
		'1\n00:00:01,000 --> 00:00:02,600\nA\n\n2\n00:00:03,600 --> 00:00:05,400\nB\n\n'
	),
	writeFile(
		french,
		'1\n00:00:01,000 --> 00:00:02,600\nDEBUT DU FILM\n\n2\n00:00:03,600 --> 00:00:05,400\nFIN DU FILM\n\n'
	)
]);
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
const tone = (frequency, start, end) =>
	`0.04*sin(2*PI*${frequency}*t)*gte(t\\,${start})*lt(t\\,${end})`;
const head = tone(440, 1, 2.6);
const tail = tone(640, 3.6, 5.4);
const tones = (left, right) => `aevalsrc=${left}|${right}:s=48000:d=6`;
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	...[
		['black', 1],
		['red', 1.6],
		['lime', 1],
		['blue', 1.8],
		['black', 0.6]
	].flatMap(([color, duration]) => [
		'-f',
		'lavfi',
		'-i',
		`color=${color}:s=160x120:r=20:d=${duration}`
	]),
	'-f',
	'lavfi',
	'-i',
	tones(head, tail),
	'-f',
	'lavfi',
	'-i',
	tones(tail, head),
	'-i',
	english,
	'-i',
	french,
	'-filter_complex',
	'[0:v][1:v][2:v][3:v][4:v]concat=n=5:v=1:a=0[picture]',
	'-map',
	'[picture]',
	'-map',
	'5:a:0',
	'-map',
	'6:a:0',
	'-map',
	'7:s:0',
	'-map',
	'8:s:0',
	'-c:v',
	'libx265',
	'-pix_fmt',
	'yuv420p',
	'-preset',
	'ultrafast',
	'-x265-params',
	'pools=1:frame-threads=1:keyint=20:min-keyint=20:log-level=error',
	'-c:a',
	'libopus',
	'-b:a',
	'128k',
	'-c:s',
	'srt',
	'-metadata:s:a:0',
	'title=Head left',
	'-metadata:s:a:0',
	'language=eng',
	'-metadata:s:a:1',
	'title=Head right',
	'-metadata:s:a:1',
	'language=fra',
	'-metadata:s:s:0',
	'language=eng',
	'-metadata:s:s:1',
	'language=fra',
	'-disposition:a:0',
	'default',
	'-disposition:a:1',
	'0',
	'-output_ts_offset',
	'1.8',
	file
]);
const streams = JSON.parse(
	await run('ffprobe', ['-v', 'error', '-show_streams', '-of', 'json', file])
).streams;
assert.deepEqual(
	streams.map(({ codec_name }) => codec_name),
	['hevc', 'opus', 'opus', 'subrip', 'subrip']
);
assert.equal(streams[0].profile, 'Main');
assert(Number(streams[0].start_time) >= 1.8, 'exercise a nonzero Video PTS origin');

const native = await openNativeEditor();
const sdk = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const sender = `MapGyver-Video-MKV-Loop-${randomUUID()}`;
const extent = { width: 160, height: 120 };
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
let token, failure, adapter;
const evidence = [];
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
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function intent(body) {
	const result = await post('/api/ui/intent', body);
	assert(result.success, JSON.stringify(result));
	return result.result?.value;
}
const operation = (namespace, operation, payload) =>
	intent({ kind: 'invokeAppOperation', namespace, operation, payload });
const edit = (payload, domain = 'media') => operation(`mapgyver.${domain}`, 'edit', payload);
const audio = () => operation('mapgyver.audio', 'inspect-output', null);
const levels = async () => {
	const values = (await audio()).output_levels;
	return [left, right].map((id) => values.find(({ channel }) => channel === id)?.rms_linear);
};
const set = (node, value) =>
	intent({ kind: 'setParam', node: node.node_id, value, behaviour: 'Append' });
function field(nodes, owner, name) {
	for (const id of owner.children ?? []) {
		const child = nodes.find((node) => node.node_id === id);
		if (child?.decl_id.split('/').at(-1) === name) return child;
		const found = child && field(nodes, child, name);
		if (found) return found;
	}
}
async function wait(read, ready, label, timeout = 7000) {
	const deadline = Date.now() + timeout;
	for (;;) {
		const result = await read();
		if (ready(result)) return result;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(result)}`);
		await delay(25);
	}
}
const quiet = (value) => value !== undefined && value < 0.0006;
const audible = (value) => value !== undefined && value > 0.005;
const routed = {
	left: ([l, r]) => audible(l) && quiet(r),
	right: ([l, r]) => quiet(l) && audible(r),
	silent: ([l, r]) => quiet(l) && quiet(r),
	mixed: ([l, r]) => audible(l) && audible(r)
};
const summarize = `
import hashlib, json, sys
from PIL import Image
image = Image.open(sys.argv[1]).convert("RGB")
r,g,b = image.getpixel((80,25))
phase = "blend" if r > 90 and b > 90 and g < 45 else (
    "head" if r > 190 and g < 45 and b < 45 else (
    "gap" if g > 190 and r < 45 and b < 45 else (
    "tail" if b > 190 and r < 45 and g < 45 else "other")))
mask = bytes(min(rgb) >= 200 for rgb in image.crop((0,65,160,120)).getdata())
print(json.dumps(dict(phase=phase, rgb=[r,g,b], bright=sum(mask),
                      caption=hashlib.sha256(mask).hexdigest() if any(mask) else None)))
`;
let captures = 0;
async function capture() {
	const dest = path.join(directory, `pixels-${++captures}`);
	const info = JSON.parse(
		await run(sdk, [
			'--role',
			'video-pixels',
			'--adapter',
			String(adapter.index),
			'--name',
			sender,
			'--directory',
			dest
		])
	);
	assert.deepEqual(info, { ...extent, retired: true });
	return path.join(dest, 'pixels.png');
}
const readPixels = async (file) => JSON.parse(await run('python', ['-c', summarize, file]));
const pixels = async () => readPixels(await capture());

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'MKV loop markers'
	});
	await edit({ kind: 'ConfigureVideoFile', reference, file });
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const prefs = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
	const channel = nodes.find((node) => node.uuid === reference.channel);
	const track = field(nodes, channel, 'video_audio_track');
	await set(field(nodes, prefs, 'output_enabled'), { Bool: true });
	const route = await wait(audio, (value) => value.readiness === 'ready', 'WASAPI routes');
	assert.equal(route.active_device.backend, 'wasapi');
	const { screen, surface } = await edit(
		{ kind: 'CreateScreen', label: 'MKV moving output' },
		'screens'
	);
	const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
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
	adapter = await observeNativeAdapter(
		native.page,
		JSON.parse(await run(sdk, ['--role', 'hardware']))
	);
	const status = await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference })).status,
		(value) => value?.state === 'Ready',
		'ready HEVC/Opus MKV'
	);
	assert(Math.abs(status.duration_ms - 6000) <= 1, JSON.stringify(status));
	evidence.push({
		pictureOriginMs: Number(streams[0].start_time) * 1000,
		durationMs: status.duration_ms
	});
	const found = await wait(
		() => edit({ kind: 'InspectVideoSubtitles', reference }),
		(value) => value.tracks.length === 2,
		'embedded SubRip tracks'
	);
	assert.deepEqual(
		found.tracks.map(({ index }) => index),
		[3, 4]
	);
	const subtitles = (index) =>
		edit({
			kind: 'ConfigureVideoSubtitles',
			reference,
			settings: { ...found.settings, enabled: true, size: 14, track: { kind: 'Embedded', index } }
		});
	const transport = async (command) =>
		(await edit({ kind: 'Video', media: reference.media, edit: command })).snapshot;
	const observe = async () =>
		(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot;
	const crop = await transport({ kind: 'Crop', start_ms: 1200, end_ms: 5200 });
	assert.equal(crop.length_ms, 4000);
	await transport({ kind: 'Loop', looping: true });
	const signatures = {};
	// Held images establish exact caption masks. Moving checks below sample the independent output.
	for (const [index, language] of [
		[3, 'short'],
		[4, 'long']
	]) {
		await subtitles(index);
		for (const [phase, position, minimum, maximum] of [
			['head', 350, language === 'long' ? 100 : 10, language === 'long' ? 2000 : 95],
			['tail', 3000, language === 'long' ? 100 : 10, language === 'long' ? 2000 : 95]
		]) {
			await transport({ kind: 'Seek', position_ms: position });
			const image = await wait(
				pixels,
				(value) => value.phase === phase && value.bright >= minimum && value.bright <= maximum,
				`${language} ${phase} caption`
			);
			signatures[`${language}-${phase}`] = image.caption;
			evidence.push({ label: `held ${language} ${phase}`, position, image });
		}
	}
	assert.equal(new Set(Object.values(signatures)).size, 4);
	await subtitles(3);
	await transport({ kind: 'Seek', position_ms: 100 });
	await transport({ kind: 'Play' });

	async function moving(label, [start, end], phase, caption, route) {
		const witness = await wait(
			async () => {
				const before = await observe();
				assert(before.playing, label);
				if (before.position_ms < start || before.position_ms >= end - 120)
					return { position: before.position_ms };
				const first = await levels();
				if (!routed[route](first)) return { position: before.position_ms, first };
				const file = await capture();
				const after = await observe();
				const last = await levels();
				const image = await readPixels(file);
				return {
					before: before.position_ms,
					after: after.position_ms,
					playing: after.playing,
					first,
					last,
					image
				};
			},
			(value) =>
				value.playing &&
				value.after >= value.before &&
				value.after <= end &&
				value.image.phase === phase &&
				value.image.caption === caption &&
				routed[route](value.last),
			label
		);
		evidence.push({ label, ...witness });
		return witness;
	}

	const initial = await moving(
		'moving head with default Opus and short captions',
		[150, 800],
		'head',
		signatures['short-head'],
		'left'
	);
	const switchStarted = performance.now();
	await set(track, { Enum: 'Embedded:2' });
	await subtitles(4);
	const switched = await moving(
		'moving head after live audio and subtitle changes',
		[250, 1350],
		'head',
		signatures['long-head'],
		'right'
	);
	const switchWindowMs = performance.now() - switchStarted;
	assert(
		switched.before >= initial.after && switchWindowMs < crop.length_ms,
		'track changes keep the running transport position within the first loop'
	);
	evidence.push({ switchWindowMs });
	await moving(
		'cue gap keeps green picture, no caption and quiet audio',
		[1550, 2250],
		'gap',
		null,
		'silent'
	);
	await moving(
		'moving cropped tail uses long caption and left Opus',
		[2550, 3600],
		'tail',
		signatures['long-tail'],
		'left'
	);
	async function wrap(label, minimum, maximum) {
		let previous = (await observe()).position_ms;
		await wait(
			observe,
			(sample) => {
				assert(sample.playing);
				const wrapped =
					previous > 3700 && sample.position_ms >= minimum && sample.position_ms < maximum;
				if (wrapped) evidence.push({ label, before: previous, after: sample.position_ms });
				previous = sample.position_ms;
				return wrapped;
			},
			label
		);
	}
	await wrap('hard crop wrap', 0, 200);
	await moving(
		'first moving head after hard wrap',
		[100, 1000],
		'head',
		signatures['long-head'],
		'right'
	);

	await transport({ kind: 'Pause' });
	await transport({ kind: 'AutoSmoothLoop', duration_ms: 800 });
	await transport({ kind: 'Seek', position_ms: 3050 });
	await transport({ kind: 'Play' });
	await moving(
		'smooth cropped seam blends picture and both audio markers',
		[3450, 3900],
		'blend',
		signatures['long-tail'],
		'mixed'
	);
	await wrap('smooth crop wrap skips the overlapped head', 795, 910);
	await moving(
		'smooth wrap resumes after the overlapped head',
		[825, 1390],
		'head',
		signatures['long-head'],
		'right'
	);
	await transport({ kind: 'Stop' });
	await wait(levels, routed.silent, 'Stop clears routed Opus audio');
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
console.log(JSON.stringify({ directory, adapter, cases: evidence.length }));
