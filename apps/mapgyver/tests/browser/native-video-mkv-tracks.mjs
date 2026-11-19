// Real MKV playback, Inspector track selection, routed PCM, and independent Spout pixels.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';

const root = path.resolve(process.argv[2] ?? 'target/m103-mkv-tracks');
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const file = path.join(directory, 'tracks.mkv');
const english = path.join(directory, 'english.srt');
const french = path.join(directory, 'french.srt');
await writeFile(english, '1\n00:00:03,250 --> 00:00:04,750\nENG\n\n');
await writeFile(french, '1\n00:00:03,250 --> 00:00:04,750\nTexte français\n\n');
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	'-f',
	'lavfi',
	'-i',
	'color=black:s=160x120:r=20:d=8',
	'-f',
	'lavfi',
	'-i',
	'aevalsrc=0.02*sin(2*PI*440*t)|0:s=48000:d=12',
	'-f',
	'lavfi',
	'-i',
	'aevalsrc=0|0.02*sin(2*PI*640*t):s=48000:d=12',
	'-i',
	english,
	'-i',
	french,
	'-map',
	'0:v:0',
	'-map',
	'1:a:0',
	'-map',
	'2:a:0',
	'-map',
	'3:s:0',
	'-map',
	'4:s:0',
	'-c:v',
	'libx264',
	'-pix_fmt',
	'yuv420p',
	'-c:a',
	'pcm_s16le',
	'-c:s',
	'srt',
	'-metadata:s:a:0',
	'title=Studio left',
	'-metadata:s:a:0',
	'language=eng',
	'-metadata:s:a:1',
	'title=Studio right',
	'-metadata:s:a:1',
	'language=fra',
	'-metadata:s:s:0',
	'title=English captions',
	'-metadata:s:s:0',
	'language=eng',
	'-metadata:s:s:1',
	'title=French captions',
	'-metadata:s:s:1',
	'language=fra',
	'-disposition:a:0',
	'0',
	'-disposition:a:1',
	'default',
	file
]);
const cliMetadata = JSON.parse(
	await run('ffprobe', ['-v', 'error', '-show_streams', '-show_format', '-of', 'json', file])
);
assert.equal(Number(cliMetadata.format.duration), 12);
assert.equal(cliMetadata.streams[0].tags.DURATION, '00:00:08.000000000');

const native = await openNativeEditor();
const { page } = native;
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
const observer = path.resolve('target/debug/native_qualification.exe');
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
	const ack = await post('/api/ui/intent', body);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}
const operation = (namespace, operation, payload) =>
	intent({ kind: 'invokeAppOperation', namespace, operation, payload });
const edit = (payload, domain = 'media') => operation(`mapgyver.${domain}`, 'edit', payload);
const graph = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
const set = (node, value) =>
	intent({ kind: 'setParam', node: node.node_id, value, behaviour: 'Append' });
function field(nodes, parent, name) {
	for (const id of parent?.children ?? []) {
		const child = nodes.find((node) => node.node_id === id);
		if (child?.decl_id.split('/').at(-1) === name) return child;
		const nested = field(nodes, child, name);
		if (nested) return nested;
	}
}
async function wait(read, ready, label) {
	const deadline = Date.now() + 7000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(50);
	}
}
const outputStatus = () => operation('mapgyver.audio', 'inspect-output', null);
const levels = async () =>
	Object.fromEntries(
		(await outputStatus()).output_levels.map(({ channel, rms_linear }) => [channel, rms_linear])
	);
const quiet = (value) => value !== undefined && value < 0.0001;
async function sound(label, ready) {
	const value = await wait(levels, ready, label);
	evidence.push({ label, levels: value });
}

try {
	token = (await post('/api/ui/session', {})).token;
	const label = 'MKV tracks';
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label });
	await edit({ kind: 'ConfigureVideoFile', reference, file });
	const source = async () => (await edit({ kind: 'InspectVideoSource', reference })).status;
	if (process.argv.includes('--baseline')) {
		const status = await wait(
			source,
			(status) => status && (status.duration_ms !== null || status.error),
			'settled baseline MKV probe'
		);
		assert.match(status.error?.message ?? '', /duration is unavailable/);
		evidence.push({ baseline: status, metadata: { containerSeconds: 12, videoSeconds: 8 } });
	} else {
		const status = await wait(
			source,
			(status) => status?.duration_ms === 8000,
			'8-second picture duration'
		);
		evidence.push({ durationMs: status.duration_ms, containerSeconds: 12 });
		const { reference: second } = await edit({
			kind: 'AddChannel',
			media: reference.media,
			label: 'Second File'
		});
		await edit({ kind: 'ConfigureVideoFile', reference: second, file });
		const nodes = await graph();
		const owner = nodes.find((node) => node.uuid === reference.channel);
		const secondOwner = nodes.find((node) => node.uuid === second.channel);
		const track = field(nodes, owner, 'video_audio_track');
		const secondTrack = field(nodes, secondOwner, 'video_audio_track');
		await set(field(nodes, secondOwner, 'audio_gain'), { Float: -120 });
		await set(secondTrack, { Enum: 'Embedded:2' });
		const prefs = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
		await set(field(nodes, prefs, 'output_enabled'), { Bool: true });
		const output = await wait(
			outputStatus,
			(state) => state.readiness === 'ready',
			'native audio endpoint'
		);
		assert.equal(output.active_device.backend, 'wasapi');

		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: label, exact: true })
			.click();
		for (const node of [owner, field(nodes, owner, 'video_audio')]) {
			const header = page.locator(
				`.channels-section .node-inspector[data-node-id="${node.node_id}"] > .node-header > .node-title`
			);
			if ((await header.getAttribute('aria-checked')) === 'false') await header.press('Enter');
		}
		const choices = page.locator(`.parameter-inspector[data-node-id="${track.node_id}"] select`);
		const audioLabels = [
			'Automatic',
			'Studio left · eng · Track 1',
			'Studio right · fra · Track 2'
		];
		await wait(
			async () => {
				const labels = await choices.getByRole('option').allTextContents();
				if (JSON.stringify(labels) === JSON.stringify(audioLabels)) return { labels };
				const parameter = (await graph()).find((node) => node.node_id === track.node_id);
				return {
					labels,
					authored: parameter.data.param.constraints.enum_options.map(({ label }) => label),
					subtitles: (await edit({ kind: 'InspectVideoSubtitles', reference })).tracks
				};
			},
			({ labels }) => JSON.stringify(labels) === JSON.stringify(audioLabels),
			'named audio choices'
		);
		assert.equal(await choices.inputValue(), 'Automatic');
		const transport = (editValue) =>
			edit({ kind: 'Video', media: reference.media, edit: editValue });
		const position = async () =>
			(await edit({ kind: 'InspectVideo', media: reference.media })).snapshot;
		await transport({ kind: 'Seek', position_ms: 1800 });
		await transport({ kind: 'Play' });
		await sound(
			'Automatic plays the default-tagged right track',
			(value) => quiet(value[left]) && value[right] > 0.004
		);
		const before = await position();
		await choices.selectOption({ label: audioLabels[1] });
		await sound(
			'selecting the left track replaces PCM in place',
			(value) => value[left] > 0.004 && quiet(value[right])
		);
		const after = await position();
		assert(after.playing && after.position_ms > before.position_ms && before.position_ms >= 1800);
		evidence.push({
			switchedAt: { before: before.position_ms, after: after.position_ms },
			audioLabels
		});
		await set(track, { Enum: 'Embedded:99' });
		await sound(
			'a missing explicit track is silent',
			(value) => quiet(value[left]) && quiet(value[right])
		);
		assert((await position()).playing);
		await choices
			.getByRole('option', { name: /Embedded:99.*unavailable/ })
			.waitFor({ state: 'attached' });
		await choices.selectOption({ label: audioLabels[1] });
		await sound(
			'an available explicit track resumes without restarting Video',
			(value) => value[left] > 0.004 && quiet(value[right])
		);
		await transport({ kind: 'Pause' });

		const extent = { width: 160, height: 120 };
		async function screenOutput(source, label) {
			const sender = `MapGyver-Video-MKV-${randomUUID()}`;
			const { screen, surface } = await edit({ kind: 'CreateScreen', label }, 'screens');
			const { snapshot } = await edit({ kind: 'InspectSurface', reference: surface }, 'screens');
			await edit(
				{
					kind: 'ConfigureSurface',
					reference: surface,
					definition: { ...snapshot.definition, source }
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
		const sender = await screenOutput(reference, 'MKV first channel');
		const peerSender = await screenOutput(second, 'MKV second channel');
		adapter = await observeNativeAdapter(
			page,
			JSON.parse(await run(observer, ['--role', 'hardware']))
		);
		await wait(source, (status) => status.state === 'Ready', 'MKV decoded and displayed');
		const subtitleInspect = (channel = reference) =>
			edit({ kind: 'InspectVideoSubtitles', reference: channel });
		const discovered = await wait(
			subtitleInspect,
			({ tracks }) => tracks.length === 2,
			'two independently named captions'
		);
		assert.deepEqual(
			discovered.tracks.map(({ label }) => label),
			['English captions · eng · Track 3', 'French captions · fra · Track 4']
		);
		await transport({ kind: 'Crop', start_ms: 3300, end_ms: 5300 });
		await transport({ kind: 'Seek', position_ms: 200 });
		const subtitles = (index, channel = reference) =>
			edit({
				kind: 'ConfigureVideoSubtitles',
				reference: channel,
				settings: {
					...discovered.settings,
					enabled: true,
					size: 12,
					track: { kind: 'Embedded', index }
				}
			});
		let capture = 0;
		async function pixels(label, accept, output = sender) {
			const result = await wait(
				async () => {
					const dest = path.join(directory, `pixels-${++capture}`);
					const received = JSON.parse(
						await run(observer, [
							'--role',
							'video-pixels',
							'--adapter',
							String(adapter.index),
							'--name',
							output,
							'--directory',
							dest
						])
					);
					assert.deepEqual(received, { ...extent, retired: true });
					const code =
						'import hashlib,json,sys;from PIL import Image; im=Image.open(sys.argv[1]).convert("RGB"); print(json.dumps(dict(sha256=hashlib.sha256(im.tobytes()).hexdigest(),bright=sum(min(px)>=200 for px in im.getdata()))))';
					return JSON.parse(await run('python', ['-c', code, path.join(dest, 'pixels.png')]));
				},
				accept,
				label
			);
			evidence.push({ label, ...result });
			return result;
		}
		await pixels('held cropped MKV frame before enabling captions', (image) => image.bright === 0);
		await subtitles(3);
		const first = await pixels(
			'English subtitle appears at source time after a cropped seek',
			(image) => image.bright > 10
		);
		await subtitles(4);
		const translated = await pixels(
			'French track replaces the caption at the same held time',
			(image) => image.bright > first.bright && image.sha256 !== first.sha256
		);
		await pixels(
			'selecting a track leaves the same-file second channel off',
			(image) => image.bright === 0,
			peerSender
		);
		await subtitles(3, second);
		await pixels(
			'the same file shows English on the second channel',
			(image) => image.sha256 === first.sha256,
			peerSender
		);
		await pixels(
			'editing the second channel preserves French on the first channel',
			(image) => image.sha256 === translated.sha256
		);
		await transport({ kind: 'Seek', position_ms: 1800 });
		await pixels(
			'cropped seek past both subtitle cue ends clears the caption',
			(image) => image.bright === 0
		);
		const save = path.join(directory, 'tracks.mapgyver');
		assert((await post('/api/ui/project-save', { path: save })).ok);
		assert.equal((await post('/api/ui/project-load', { path: save })).path, save);
		await wait(source, (status) => status?.duration_ms === 8000, 'reloaded MKV');
		const restored = await graph();
		for (const [channel, index] of [
			[reference.channel, 1],
			[second.channel, 2]
		]) {
			const track = field(
				restored,
				restored.find((node) => node.uuid === channel),
				'video_audio_track'
			);
			assert.deepEqual(track.data.param.value, { Enum: `Embedded:${index}` });
		}
		assert.deepEqual((await subtitleInspect()).settings.track, { kind: 'Embedded', index: 4 });
		assert.deepEqual((await subtitleInspect(second)).settings.track, {
			kind: 'Embedded',
			index: 3
		});
		await transport({ kind: 'Seek', position_ms: 200 });
		await pixels(
			'reload preserves the French subtitle selection',
			(image) => image.sha256 === translated.sha256
		);
		await pixels(
			'reload preserves the second channel’s English subtitle selection',
			(image) => image.sha256 === first.sha256,
			peerSender
		);
		await transport({ kind: 'Play' });
		await sound(
			'reload preserves the independently authored left track',
			(value) => value[left] > 0.004 && quiet(value[right])
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
console.log(JSON.stringify({ result: 'pass', directory, evidence }));
