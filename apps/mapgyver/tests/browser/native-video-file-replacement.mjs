// Active Video input loss, replacement at the same path, and extended-format relink.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, rename, symlink, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';

const root = path.resolve(process.argv[2] ?? 'target/m103-file-replacement');
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
const clip = 'active.mkv';
const extent = { width: 160, height: 120 };
const sources = {};
for (const [name, color, audio, caption, language] of [
	['Original', 'red', '0.04*sin(2*PI*440*t)|0', 'A', 'eng'],
	['Replacement', 'blue', '0|0.04*sin(2*PI*640*t)', 'NEW CAPTION', 'fra']
]) {
	const folder = path.join(directory, name);
	await mkdir(folder);
	const subtitle = path.join(folder, 'cue.srt');
	await writeFile(subtitle, `1\n00:00:00,000 --> 00:00:12,000\n${caption}\n\n`);
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-f',
		'lavfi',
		'-i',
		`color=${color}:s=160x120:r=20:d=12`,
		'-f',
		'lavfi',
		'-i',
		`aevalsrc=${audio}:s=48000:d=12`,
		'-i',
		subtitle,
		'-map',
		'0:v:0',
		'-map',
		'1:a:0',
		'-map',
		'2:s:0',
		'-c:v',
		'libx264',
		'-pix_fmt',
		'yuv420p',
		'-preset',
		'ultrafast',
		'-c:a',
		'pcm_s16le',
		'-c:s',
		'srt',
		'-metadata:s:a:0',
		`title=${name} audio`,
		'-metadata:s:a:0',
		`language=${language}`,
		'-metadata:s:s:0',
		`title=${name} captions`,
		'-metadata:s:s:0',
		`language=${language}`,
		path.join(folder, clip)
	]);
	sources[name] = folder;
}
const damaged = path.join(directory, 'Damaged');
await mkdir(damaged);
await writeFile(path.join(damaged, clip), 'Deliberately malformed media');
const animated = path.join(directory, 'relinked.webp');
await run('python', [
	'-c',
	'from PIL import Image; import sys; a=Image.new("RGB",(160,120),"yellow"); b=Image.new("RGB",(160,120),"green"); a.save(sys.argv[1],save_all=True,lossless=True,append_images=[b],duration=[1500,1500],loop=0)',
	animated
]);
const dxv = path.join(directory, 'relinked-dxv.mov');
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	'-f',
	'lavfi',
	'-i',
	'color=lime:s=160x120:r=20:d=3',
	'-c:v',
	'dxv',
	'-format',
	'dxt1',
	dxv
]);
// Junctions model a live folder swap without requiring Windows to rename an open decoder handle.
const live = path.join(directory, 'Live');
await symlink(sources.Original, live, 'junction');
const file = path.join(live, clip);
const native = await openNativeEditor();
const sdk = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const sender = `MapGyver-Video-replacement-${randomUUID()}`;
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
const evidence = [];
let token, failure, adapter;
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
const graph = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
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
async function wait(read, ready, label) {
	const deadline = Date.now() + 7500;
	for (;;) {
		const result = await read();
		if (ready(result)) return result;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(result)}`);
		await delay(35);
	}
}
const readLevels = async () => {
	const values = (await audio()).output_levels;
	return [left, right].map(
		(id) => values.find(({ channel }) => channel === id)?.rms_linear ?? null
	);
};
const quiet = (value) => value !== null && value < 0.0001;
const audible = (value) => value !== null && value > 0.005;
const red = ([r, g, b]) => r > 190 && g < 60 && b < 60;
const blue = ([r, g, b]) => b > 190 && r < 60 && g < 60;
const yellow = ([r, g, b]) => r > 190 && g > 190 && b < 60;
const green = ([r, g, b]) => g > 100 && r < 60 && b < 60;
let captures = 0;
async function image() {
	const dest = path.join(directory, `image-${++captures}`);
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
	const code =
		'import json,sys; from PIL import Image; im=Image.open(sys.argv[1]).convert("RGB"); print(json.dumps(dict(color=im.getpixel((80,25)),bright=sum(min(p)>=200 for p in im.crop((0,60,160,120)).getdata()))))';
	return JSON.parse(await run('python', ['-c', code, path.join(dest, 'pixels.png')]));
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'Replacing Video'
	});
	const configureFile = (file) => edit({ kind: 'ConfigureVideoFile', reference, file });
	await configureFile(file);
	const nodes = await graph();
	const prefs = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
	const channel = nodes.find((node) => node.uuid === reference.channel);
	const track = field(nodes, channel, 'video_audio_track');
	await set(field(nodes, prefs, 'output_enabled'), { Bool: true });
	const route = await wait(audio, (value) => value.readiness === 'ready', 'WASAPI routes');
	assert.equal(route.active_device.backend, 'wasapi');
	const { screen, surface } = await edit(
		{ kind: 'CreateScreen', label: 'Video relink output' },
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
			definition: { enabled: true, anti_aliasing: 'Raw', target: { kind: 'Spout', sender, extent } }
		},
		'screens'
	);
	adapter = await observeNativeAdapter(
		native.page,
		JSON.parse(await run(sdk, ['--role', 'hardware']))
	);
	const source = async () => (await edit({ kind: 'InspectVideoSource', reference })).status;
	const subtitles = () => edit({ kind: 'InspectVideoSubtitles', reference });
	const transport = async (command) =>
		(await edit({ kind: 'Video', media: reference.media, edit: command })).snapshot;
	const observe = async () =>
		(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot;
	async function catalogs() {
		const param = (await graph()).find((node) => node.node_id === track.node_id).data.param;
		const captions = await subtitles();
		return {
			audio: param.constraints.enum_options.map(({ label }) => label),
			subtitles: captions.tracks.map(({ label }) => label),
			audioSelection: param.value,
			subtitleSelection: captions.settings.track
		};
	}
	const ready = () =>
		wait(source, (value) => value?.state === 'Ready' && !value.error, 'Video Ready');
	const expectedAudio = (name) =>
		`${name} audio · ${name === 'Original' ? 'eng' : 'fra'} · Track 1`;
	const expectedCaption = (name) =>
		`${name} captions · ${name === 'Original' ? 'eng' : 'fra'} · Track 2`;
	async function verify(label, name, picture, bright, lanes) {
		const status = await ready();
		const tracks = await wait(
			catalogs,
			(value) =>
				JSON.stringify(value.audio) ===
					JSON.stringify(['Automatic', name ? expectedAudio(name) : 'Embedded:1 (unavailable)']) &&
				JSON.stringify(value.subtitles) === JSON.stringify(name ? [expectedCaption(name)] : []),
			`${label} track catalogs`
		);
		assert.deepEqual(tracks.audioSelection, { Enum: 'Embedded:1' });
		assert.deepEqual(tracks.subtitleSelection, { kind: 'Embedded', index: 2 });
		const pixels = await wait(
			image,
			(value) => picture(value.color) && bright(value.bright),
			`${label} pixels`
		);
		const levels = await wait(readLevels, lanes, `${label} audio`);
		const playback = await observe();
		assert(playback.playing, label);
		evidence.push({
			label,
			source: status.state,
			position: playback.position_ms,
			tracks,
			pixels,
			levels
		});
	}
	await ready();
	const first = await wait(
		subtitles,
		(value) => value.tracks.length === 1,
		'original caption track'
	);
	await set(track, { Enum: 'Embedded:1' });
	await edit({
		kind: 'ConfigureVideoSubtitles',
		reference,
		settings: { ...first.settings, enabled: true, size: 14, track: { kind: 'Embedded', index: 2 } }
	});
	await transport({ kind: 'Loop', looping: true });
	await transport({ kind: 'Play' });
	const leftOnly = ([l, r]) => audible(l) && quiet(r);
	const rightOnly = ([l, r]) => quiet(l) && audible(r);
	const silent = ([l, r]) => quiet(l) && quiet(r);
	const noCaption = (pixels) => pixels === 0;
	await verify('original active MKV', 'Original', red, (px) => px >= 10 && px < 100, leftOnly);
	await rename(live, path.join(directory, 'Original-link'));
	const missingLevels = await wait(readLevels, silent, 'missing MKV silences audio');
	if (process.argv.includes('--baseline')) {
		const status = await source();
		const tracks = await catalogs();
		assert.equal(status.state, 'Ready', JSON.stringify(status));
		assert.deepEqual(tracks.subtitles, [expectedCaption('Original')]);
		evidence.push({
			label: 'baseline retains stale Ready and track catalogs',
			status,
			tracks,
			missingLevels
		});
	} else {
		async function unavailable(label, kind) {
			const status = await wait(
				source,
				(value) => value.state !== 'Ready' && value.error?.kind === kind && value.retrying,
				label
			);
			const tracks = await wait(
				catalogs,
				(value) =>
					value.subtitles.length === 0 &&
					value.audio.length === 2 &&
					value.audio[1] === 'Embedded:1 (unavailable)',
				`${label} clears streams`
			);
			assert.deepEqual(tracks.audioSelection, { Enum: 'Embedded:1' });
			assert.deepEqual(tracks.subtitleSelection, { kind: 'Embedded', index: 2 });
			const levels = await wait(readLevels, silent, `${label} stays silent`);
			evidence.push({ label, status, tracks, levels });
		}
		await unavailable('active MKV disappears', 'Missing');
		await symlink(damaged, live, 'junction');
		await unavailable('malformed replacement at same path', 'Decode');
		await rename(live, path.join(directory, 'Damaged-link'));
		await symlink(sources.Replacement, live, 'junction');
		await verify(
			'same-path recovery without another Play command',
			'Replacement',
			blue,
			(px) => px > 150,
			rightOnly
		);

		// Swap while Ready without waiting for a missing-state observation between the two roots.
		await rename(live, path.join(directory, 'Replacement-link'));
		await symlink(sources.Original, live, 'junction');
		await verify(
			'same-path swap from a ready MKV',
			'Original',
			red,
			(px) => px >= 10 && px < 100,
			leftOnly
		);
		await configureFile(animated);
		await verify(
			'authored relink to animated WebP',
			null,
			(color) => yellow(color) || green(color),
			noCaption,
			silent
		);
		await configureFile(dxv);
		await verify('authored relink to DXV', null, green, noCaption, silent);
		await transport({ kind: 'Stop' });
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
console.log(JSON.stringify({ directory, adapter, cases: evidence.length }));
