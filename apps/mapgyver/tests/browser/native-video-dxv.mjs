// Bundled DXV playback with authentic FFmpeg FATE profiles and independent Spout pixels.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { observeNativeAdapter } from './native-adapter.mjs';
import { openNativeEditor } from './native-editor.mjs';

const root = path.resolve(process.argv[2] ?? 'target/m103-dxv');
await mkdir(root, { recursive: true });
const directory = await mkdtemp(path.join(root, 'run-'));
const sdk = path.resolve(
	process.env.MAPGYVER_QUALIFICATION_BINARY ?? 'target/debug/native_qualification.exe'
);
const execute = promisify(execFile);
const run = async (file, args) =>
	(await execute(file, args, { windowsHide: true, timeout: 15000 })).stdout;
const samples = [
	['dxv-na.mov', '70151d33af69df482dd8d52ccd851e81ab732a2565c55e94259def6c91b866a1'],
	['dxv-wa.mov', '62b6d0beeb82c36448165503e8a75fa2d22d8d82dbcac0fac1c7ce039270c5e8'],
	['dxv3-nqna.mov', '58623e216687e53d84c3c31d77618c4b498cea706fd0cfc110565bfb1a8bf4b0'],
	['dxv3-nqwa.mov', '47b770ab642eb32c743b8b700943870fae8a5d013fd94f869bc3f600db08b8b5'],
	['dxv3-hqna.mov', '0a9855ce13a58d3861a5626532ba52099b05d4329f8b468c6774c00fd709bc4c'],
	['dxv3-hqwa.mov', '30f2f60095b2ff62702a01d574fc2270a0dd428bf7c08a3456aa30cadb28b243']
];
const fixtures = path.resolve(process.env.MAPGYVER_DXV_FIXTURES ?? path.join(root, 'fixtures'));
await mkdir(fixtures, { recursive: true });
for (const [name, sha256] of samples) {
	const file = path.join(fixtures, name);
	let bytes;
	try {
		bytes = await readFile(file);
	} catch (error) {
		if (error.code !== 'ENOENT') throw error;
		const response = await fetch(`https://fate-suite.ffmpeg.org/dxv/${name}`, {
			signal: AbortSignal.timeout(15000)
		});
		assert(response.ok, `fixture ${name}: ${response.status}`);
		bytes = Buffer.from(await response.arrayBuffer());
		assert.equal(createHash('sha256').update(bytes).digest('hex'), sha256);
		await writeFile(file, bytes, { flag: 'wx' });
	}
	assert.equal(createHash('sha256').update(bytes).digest('hex'), sha256, name);
}
const moving = path.join(directory, 'moving-dxt1.mov');
const alphaFixture = process.env.MAPGYVER_DXV_ALPHA_FIXTURE
	? path.resolve(process.env.MAPGYVER_DXV_ALPHA_FIXTURE)
	: null;
await run('ffmpeg', [
	'-hide_banner',
	'-loglevel',
	'error',
	'-f',
	'lavfi',
	'-i',
	'testsrc2=s=96x64:r=8:d=3',
	'-c:v',
	'dxv',
	'-format',
	'dxt1',
	'-threads',
	'1',
	moving
]);
async function inspect(file) {
	const {
		streams: [stream]
	} = JSON.parse(
		await run('ffprobe', [
			'-v',
			'error',
			'-select_streams',
			'v:0',
			'-show_streams',
			'-of',
			'json',
			file
		])
	);
	assert.equal(stream.codec_name, 'dxv');
	return stream;
}

const native = await openNativeEditor();
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
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
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
const screenEdit = (payload) => edit(payload, 'screens');
async function setSurface(reference, source) {
	const { snapshot } = await screenEdit({ kind: 'InspectSurface', reference });
	await screenEdit({
		kind: 'ConfigureSurface',
		reference,
		definition: { ...snapshot.definition, source }
	});
}
async function wait(read, accept, label) {
	const deadline = Date.now() + 7000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}

const compare = `
import json, sys
from PIL import Image
reference, actual = [Image.open(path).convert("RGBA") for path in sys.argv[1:3]]
assert reference.size == actual.size
width, height = actual.size
def linear(value):
    value /= 255
    return value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4
def srgb(value):
    return round(255 * (12.92 * value if value <= 0.0031308 else 1.055 * value ** (1 / 2.4) - 0.055))
errors = []
translucent_errors = []
transparent = 0
for y in range(1, height, 3):
    for x in range(1, width, 3):
        r,g,b,a = reference.getpixel((x,y))
        opacity = a / 255
        transparent += a == 0
        expected = [srgb(linear(component) * opacity + bg * (1 - opacity))
                    for component,bg in zip((r,g,b),(1,1,0))] + [255]
        error = max(abs(p-q) for p,q in zip(actual.getpixel((x,y)), expected))
        errors.append(error)
        if 0 < a < 255:
            translucent_errors.append(error)
ordered = sorted(errors)
translucent_errors.sort()
count = len(translucent_errors)
print(json.dumps(dict(samples=len(errors), transparent=transparent, translucent=count,
                      p99=ordered[int(len(errors) * .99)], maximum=ordered[-1],
                      translucentP99=translucent_errors[int(count * .99)] if count else None)))
`;
let captures = 0;
async function pixels(file, sender, extent, hq, timestamp, baseline) {
	const n = ++captures;
	const reference = path.join(directory, `reference-${n}.png`);
	const folder = path.join(directory, `output-${n}`);
	await run('ffmpeg', [
		'-hide_banner',
		'-loglevel',
		'error',
		'-ss',
		String(timestamp / 1000),
		'-i',
		file,
		'-frames:v',
		'1',
		'-vf',
		hq
			? 'zscale=matrixin=ycgco:rangein=full:matrix=gbr:range=full:chromalin=center:filter=bilinear,format=gbrap,format=rgba'
			: 'format=rgba',
		'-threads',
		'1',
		reference
	]);
	const measure = async () => {
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
			await run('python', ['-c', compare, reference, path.join(folder, 'pixels.png')])
		);
	};
	return baseline
		? measure()
		: wait(
				measure,
				({ p99, translucentP99 }) =>
					p99 <= 5 && (file !== alphaFixture || (translucentP99 !== null && translucentP99 <= 5)),
				`${file} at ${timestamp} ms`
			);
}

try {
	token = (await post('/api/ui/session', {})).token;
	const baseline = process.argv.includes('--baseline');
	const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label: 'DXV' });
	const { reference: background } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'DXV alpha witness'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference: background,
		source: { kind: 'SolidColor', rgba: [1, 1, 0, 1] }
	});
	const { screen, surface } = await screenEdit({ kind: 'CreateScreen', label: 'DXV output' });
	await setSurface(surface, background);
	const { reference: foreground } = await screenEdit({ kind: 'AddSurface', screen });
	await setSurface(foreground, reference);
	const source = async () => (await edit({ kind: 'InspectVideoSource', reference })).status;
	const frame = async () => (await edit({ kind: 'InspectVideoFrame', reference })).frame;
	const transport = async (command) =>
		(await edit({ kind: 'Video', media: reference.media, edit: command })).snapshot;
	const files = [...samples.map(([name]) => path.join(fixtures, name)), moving];
	if (alphaFixture) files.push(alphaFixture);
	for (const file of files) {
		const name = path.basename(file);
		const stream = await inspect(file);
		const hq = stream.color_space === 'ycgco';
		const extent = { width: stream.width, height: stream.height };
		const sender = `MapGyver-Video-DXV-${randomUUID()}`;
		await edit({ kind: 'ConfigureVideoFile', reference, file });
		await screenEdit({
			kind: 'ConfigureScreen',
			screen,
			definition: { enabled: true, anti_aliasing: 'Raw', target: { kind: 'Spout', sender, extent } }
		});
		adapter ??= await observeNativeAdapter(
			native.page,
			JSON.parse(await run(sdk, ['--role', 'hardware']))
		);
		const status = await wait(
			source,
			(value) =>
				value?.state === 'Ready' ||
				(baseline && hq && /color matrix/i.test(value?.error?.message ?? '')),
			`${name} source`
		);
		evidence.push({ name, sample: stream.codec_tag_string, format: stream.pix_fmt, status });
		if (baseline && hq) {
			assert.match(status.error?.message ?? '', /color matrix is not yet supported/);
			continue;
		}
		const held = await pixels(file, sender, extent, hq, 0, baseline);
		evidence.push({ name, timestamp: 0, ...held });
		if (file === moving && !baseline) {
			for (const position of [1250, 0]) {
				await transport({ kind: 'Seek', position_ms: position });
				await wait(frame, (value) => value?.position_ms === position, `DXV frame ${position}`);
				evidence.push({
					name,
					timestamp: position,
					...(await pixels(file, sender, extent, hq, position, false))
				});
			}
			await transport({ kind: 'Play' });
			await wait(
				async () => (await edit({ kind: 'InspectVideo', media: reference.media })).snapshot,
				(value) => value.playing && value.position_ms >= 500,
				'DXV transport advances'
			);
			await transport({ kind: 'Pause' });
		}
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
