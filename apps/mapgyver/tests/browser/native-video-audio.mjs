// Bundled Video transport and per-channel routing observed at Golden Audio's live WASAPI mixer.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const firstFile = path.join(directory, 'left-then-right.mp4');
const secondFile = path.join(directory, 'second-right.mp4');
const left = '8fe83692-442b-4bc0-92b8-87256dfe0001';
const right = '8fe83692-442b-4bc0-92b8-87256dfe0002';
const leftTone = 'if(lt(t\\,6)\\,0.012*sin(2*PI*440*t)\\,0)';
const rightTone = 'if(gte(t\\,6)\\,0.010*sin(2*PI*640*t)\\,0)';
const execute = promisify(execFile);
for (const [file, expressions] of [
	[firstFile, `${leftTone}|${rightTone}`],
	[secondFile, '0|0.006*sin(2*PI*880*t)']
]) {
	await execute(
		'ffmpeg',
		[
			'-hide_banner',
			'-loglevel',
			'error',
			'-n',
			'-f',
			'lavfi',
			'-i',
			'color=black:s=160x120:r=24:d=18',
			'-f',
			'lavfi',
			'-i',
			`aevalsrc=${expressions}:s=44100:d=18`,
			'-c:v',
			'libx264',
			'-pix_fmt',
			'yuv420p',
			'-c:a',
			'aac',
			'-b:a',
			'192k',
			file
		],
		{ windowsHide: true, timeout: 15000 }
	);
}

const native = await openNativeEditor();
const { page } = native;
const errors = [];
const receipts = [];
let token, failure;
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
	assert(response.ok, JSON.stringify(value));
	return value;
}
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack;
}
const operation = async (namespace, name, payload) =>
	(await intent({ kind: 'invokeAppOperation', namespace, operation: name, payload })).result.value;
const edit = (payload) => operation('mapgyver.media', 'edit', payload);
const inspect = () => operation('mapgyver.audio', 'inspect-output', null);
const graph = async () => (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
const set = (node, value) =>
	intent({ kind: 'setParam', node: node.node_id, value, behaviour: 'Append' });

function descendant(nodes, parent, declId) {
	const children = (parent.children ?? []).map((key) => nodes.find((node) => node.node_id === key));
	return (
		children.find((node) => node.decl_id.split('/').at(-1) === declId) ??
		children.map((node) => descendant(nodes, node, declId)).find(Boolean)
	);
}
async function wait(read, accept, label) {
	const deadline = Date.now() + 6500;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(70);
	}
}
const quiet = (value) => value !== undefined && value < 0.0001;

try {
	token = (await post('/api/ui/session', {})).token;
	const nodes = await graph();
	const prefs = nodes.find((node) => node.node_type === 'mapgyver.preferences.audio');
	await set(descendant(nodes, prefs, 'output_enabled'), { Bool: true });
	const device = descendant(nodes, prefs, 'device');
	const auxiliary = await intent({
		kind: 'createUserItem',
		parent: prefs.node_id,
		node_type: 'mapgyver.preferences.audio.output',
		label: 'Audio fixture auxiliary',
		initial_params: []
	});
	const withAux = await graph();
	const aux = withAux.find((node) => node.node_id === auxiliary.created_node.node_id);
	await set(descendant(withAux, aux, 'physical'), { Enum: 'output:0' });
	const output = await wait(
		inspect,
		(value) => value.readiness === 'ready' && value.routes.length === 3,
		'WASAPI stereo plus auxiliary'
	);
	assert.equal(output.active_device.backend, 'wasapi');
	const auxKey = output.routes.find(([id]) => id !== left && id !== right)[0];
	const audio = async () =>
		Object.fromEntries(
			(await inspect()).output_levels.map(({ channel, rms_linear }) => [channel, rms_linear])
		);
	const record = async (label, accept) => {
		const levels = await wait(audio, accept, label);
		receipts.push({ label, levels });
		return levels;
	};
	const label = 'Video audio fixture';
	const { reference: first } = await edit({ kind: 'CreateMedia', media_kind: 'Video', label });
	const { reference: second } = await edit({
		kind: 'AddChannel',
		media: first.media,
		label: 'Second Audio channel'
	});
	await edit({ kind: 'ConfigureVideoFile', reference: first, file: firstFile });
	await edit({ kind: 'ConfigureVideoFile', reference: second, file: secondFile });
	const authored = await graph();
	const firstNode = authored.find((node) => node.uuid === first.channel);
	const secondNode = authored.find((node) => node.uuid === second.channel);
	const gain = (node) => descendant(authored, node, 'audio_gain');
	const firstLeft = descendant(authored, firstNode, 'audio_left');
	await set(descendant(authored, secondNode, 'audio_right'), { Enum: auxKey });
	await set(gain(secondNode), { Float: -120 });
	for (const reference of [first, second]) {
		await wait(
			() => edit({ kind: 'InspectVideoSource', reference }),
			(entry) => entry.status.duration_ms > 17000,
			'Video metadata available'
		);
	}
	const transport = (value) => edit({ kind: 'Video', media: first.media, edit: value });
	await record(
		'paused transport is silent',
		(value) => quiet(value[left]) && quiet(value[right]) && quiet(value[auxKey])
	);
	await transport({ kind: 'Play' });
	const baseline = await record(
		'first embedded tone reaches only the authored left output',
		(value) => value[left] > 0.005 && quiet(value[right]) && quiet(value[auxKey])
	);
	await set(gain(firstNode), { Float: -6.0206 });
	const reduced = await record(
		'Video volume change attenuates only that channel by 6 dB',
		(value) =>
			value[left] / baseline[left] >= 0.4 &&
			value[left] / baseline[left] <= 0.62 &&
			quiet(value[right]) &&
			quiet(value[auxKey])
	);

	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: label, exact: true })
		.click();
	const firstAudio = descendant(authored, firstNode, 'video_audio');
	for (const node of [firstNode, firstAudio]) {
		const header = page.locator(
			`.channels-section .node-inspector[data-node-id="${node.node_id}"] > .node-header > .node-title`
		);
		if ((await header.getAttribute('aria-checked')) === 'false') await header.press('Enter');
	}
	const leftChoice = page.locator(
		`.parameter-inspector[data-node-id="${firstLeft.node_id}"] select`
	);
	await leftChoice
		.locator(`option[value=${JSON.stringify(auxKey)}]`)
		.waitFor({ state: 'attached' });
	await leftChoice.selectOption(auxKey);
	await record(
		'Inspector remaps the playing channel to the named auxiliary output',
		(value) => quiet(value[left]) && quiet(value[right]) && value[auxKey] > 0.002
	);
	const enabled = page
		.locator(
			`.channels-section .node-inspector[data-node-id="${firstAudio.node_id}"] > .node-header`
		)
		.getByRole('switch', { name: 'Toggle node enabled', exact: true });
	await enabled.click();
	await wait(
		() => enabled.getAttribute('aria-checked'),
		(value) => value === 'false',
		'Audio folder unchecked'
	);
	await record(
		'disabling the Audio folder mutes that playing channel',
		(value) => quiet(value[left]) && quiet(value[right]) && quiet(value[auxKey])
	);
	await enabled.click();
	await wait(
		() => enabled.getAttribute('aria-checked'),
		(value) => value === 'true',
		'Audio folder checked'
	);
	await record(
		're-enabling Audio resumes its authored route',
		(value) => quiet(value[left]) && quiet(value[right]) && value[auxKey] > 0.002
	);
	await transport({ kind: 'Pause' });
	await record(
		'pausing clears every pending channel sample',
		(value) => quiet(value[left]) && quiet(value[right]) && quiet(value[auxKey])
	);
	await transport({ kind: 'Seek', position_ms: 9000 });
	await record(
		'seeking while paused keeps all routes silent',
		(value) => quiet(value[left]) && quiet(value[right]) && quiet(value[auxKey])
	);
	await transport({ kind: 'Play' });
	await record(
		'seeking resumes at the right-only section of the same file',
		(value) => quiet(value[left]) && value[right] > 0.002 && quiet(value[auxKey])
	);
	await set(gain(secondNode), { Float: 0 });
	const together = await record(
		'second embedded file has independent gain and adds an auxiliary signal',
		(value) => quiet(value[left]) && value[right] > 0.002 && value[auxKey] > 0.002
	);
	await set(device, { Enum: 'wasapi/mapgyver-owned-audio-absent' });
	await wait(
		inspect,
		(value) => value.readiness === 'missing' && value.active_device === null,
		'missing selection retires the native stream while Video is playing'
	);
	await set(device, { Enum: 'System Default' });
	await record(
		'native output reopening resumes both signals at the current transport time',
		(value) => quiet(value[left]) && value[right] > 0.002 && value[auxKey] > 0.002
	);
	await transport({ kind: 'Stop' });
	await record(
		'stopping the shared transport retires both audio sources',
		(value) => quiet(value[left]) && quiet(value[right]) && quiet(value[auxKey])
	);
	const saved = path.join(directory, 'video-audio.mapgyver');
	assert((await post('/api/ui/project-save', { path: saved })).ok);
	assert.equal((await post('/api/ui/project-load', { path: saved })).path, saved);
	await wait(
		() => edit({ kind: 'InspectVideoSource', reference: first }),
		(entry) => entry.status.duration_ms > 17000,
		'reloaded Video metadata'
	);
	await transport({ kind: 'Play' });
	const restoredRms = Math.hypot(reduced[left], together[auxKey]);
	await record(
		'saved project restores both authored routes and measured gains',
		(value) =>
			quiet(value[left]) && quiet(value[right]) && Math.abs(value[auxKey] / restoredRms - 1) < 0.15
	);
	await transport({ kind: 'Stop' });
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	await writeFile(path.join(directory, 'app.log'), native.log());
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: failure ? 'fail' : 'pass', failure, receipts }, null, 2)
	);
}
console.log(JSON.stringify({ result: 'pass', cases: receipts.map(({ label }) => label) }));
