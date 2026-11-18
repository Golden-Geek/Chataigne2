// Observe committed Cell playback on the bundled app's live WASAPI virtual outputs.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { clearMatrixScene } from './matrix-scene.mjs';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const video = path.join(directory, 'inherited.mp4');
const override = path.join(directory, 'override.wav');
const audioOnly = path.join(directory, 'audio-only.wav');
const ffmpeg = promisify(execFile);
for (const [file, tone] of [
	[video, '0.012*sin(2*PI*440*t)|0'],
	[override, '0.012*sin(2*PI*640*t)|0'],
	[audioOnly, '0|0.008*sin(2*PI*880*t)']
]) {
	const inputs = file === video ? ['-f', 'lavfi', '-i', 'color=red:s=160x120:r=24:d=16'] : [];
	const codecs = file === video ? ['-c:v', 'libx264', '-c:a', 'aac'] : ['-c:a', 'pcm_s16le'];
	await ffmpeg(
		'ffmpeg',
		[
			'-hide_banner',
			'-loglevel',
			'error',
			'-n',
			...inputs,
			'-f',
			'lavfi',
			'-i',
			`aevalsrc=${tone}:s=44100:d=16`,
			...codecs,
			file
		],
		{ windowsHide: true, timeout: 15000 }
	);
}

const native = await openNativeEditor();
const { page } = native;
const receipts = [];
const errors = [];
let token, failure, device;
page.on('pageerror', (error) => errors.push(String(error)));
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(7000)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}
const operation = (namespace, name, payload) =>
	intent({ kind: 'invokeAppOperation', namespace, operation: name, payload });
const edit = (payload) => operation('mapgyver.media', 'edit', payload);
const inspect = () => operation('mapgyver.audio', 'inspect-output', null);
const graph = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
const set = (node, value) =>
	intent({ kind: 'setParam', node: node.node_id, value, behaviour: 'Append' });

function field(nodes, parent, name) {
	const children = (parent.children ?? []).map((id) => nodes.find((node) => node.node_id === id));
	return (
		children.find((node) => node.decl_id.split('/').at(-1) === name) ??
		children.map((node) => field(nodes, node, name)).find(Boolean)
	);
}
async function wait(read, accept, message) {
	const deadline = Date.now() + 6500;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${message}: ${JSON.stringify(value)}`);
		await delay(30);
	}
}
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
const quiet = (value) => value !== undefined && value < 0.0001;
const record = async (label, predicate) => {
	const levels = await wait(
		async () => {
			const snapshot = await inspect();
			const read = (id) => snapshot.output_levels.find((entry) => entry.channel === id)?.rms_linear;
			return [read(left), read(right)];
		},
		([l, r]) => predicate(l, r),
		label
	);
	receipts.push({ label, levels });
};
const quietBoth = (l, r) => quiet(l) && quiet(r);

try {
	token = (await post('/api/ui/session', {})).token;
	const nodes = await graph();
	await set(
		field(
			nodes,
			nodes.find((node) => node.node_type === 'mapgyver.preferences.audio'),
			'output_enabled'
		),
		{ Bool: true }
	);
	device = (await wait(inspect, (value) => value.readiness === 'ready', 'native device ready'))
		.active_device;
	assert.equal(device.backend, 'wasapi');
	const label = 'Matrix Cell audio check';
	const { reference: matrixRef } = await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label });
	await clearMatrixScene(edit, matrixRef.media);
	const matrix = (value) => edit({ kind: 'Matrix', media: matrixRef.media, edit: value });
	const layer = (await matrix({ kind: 'AddLayers', parent: null, count: 1 })).items[0].id;
	const { columns } = await matrix({ kind: 'AddColumns', parent: null, count: 3 });
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Video',
		label: 'Inherited Video audio'
	});
	await edit({ kind: 'ConfigureVideoFile', reference, file: video });
	await wait(
		() => edit({ kind: 'InspectVideoSource', reference }),
		(row) => row.status.duration_ms >= 16000,
		'prepared Video duration'
	);
	await matrix({
		kind: 'SetCells',
		assignments: columns.slice(0, 2).map((column) => ({ layer, column, source: reference }))
	});
	const cells = [];
	for (const column of columns)
		cells.push((await matrix({ kind: 'EnsureCell', layer, column })).cell);
	const parameter = async (cell, name) => {
		const nodes = await graph();
		return field(
			nodes,
			nodes.find((node) => node.uuid === cell),
			name
		);
	};
	const configure = async (cell, file, lane) => {
		await set(await parameter(cell, 'audio_mode'), { Enum: 'File' });
		await set(await parameter(cell, 'audio_file'), { File: file });
		await set(await parameter(cell, lane === right ? 'audio_left' : 'audio_right'), { Enum: lane });
	};
	await configure(cells[1], override, right);
	await configure(cells[2], audioOnly, left);
	await matrix({
		kind: 'ConfigureColumn',
		column: columns[1],
		definition: { transition: { duration_ms: 1100, curve: 'Linear' } }
	});
	const launch = (column, mode = 'Cut') => matrix({ kind: 'LaunchColumn', column, mode });
	const original = (await matrix({ kind: 'Inspect' })).snapshot.cells;
	assert(original.find((cell) => cell.id === cells[1]).definition.source);
	assert.equal(original.find((cell) => cell.id === cells[2]).definition.source, null);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await page
		.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
		.getByText('audio-only.wav', { exact: true })
		.waitFor();
	await record('configured Cells are silent before launch', quietBoth);
	await launch(columns[0]);
	await record('inherited Video channel uses its left signal', (l, r) => l > 0.006 && quiet(r));
	await launch(columns[1], 'Configured');
	await record(
		'configured transition mixes inherited and independent file',
		(l, r) => l > 0.001 && r > 0.001
	);
	await record(
		'file overrides the Video sound and maps left input to Right',
		(l, r) => quiet(l) && r > 0.006
	);
	await launch(columns[2]);
	await record('audio-only Cell maps its right input to Left', (l, r) => l > 0.004 && quiet(r));
	await matrix({ kind: 'ClearPlayback' });
	await record('Matrix Stop clears both output signals', quietBoth);
	const saved = path.join(directory, 'matrix-audio.mapgyver');
	assert((await post('/api/ui/project-save', { path: saved })).ok);
	assert.equal((await post('/api/ui/project-load', { path: saved })).path, saved);
	assert.deepEqual((await parameter(cells[2], 'audio_file')).data.param.value, { File: audioOnly });
	await launch(columns[2]);
	await record(
		'reloaded audio-only Cell restores the file and virtual route',
		(l, r) => l > 0.004 && quiet(r)
	);
	await matrix({ kind: 'ClearPlayback' });
	await record('reloaded Matrix stops cleanly', quietBoth);
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	await writeFile(path.join(directory, 'app.log'), native.log());
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: failure ? 'fail' : 'pass', failure, device, receipts }, null, 2)
	);
}
console.log(JSON.stringify({ result: 'pass', receipts }));
