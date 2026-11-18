// Owns its app profile and Video clip. Observe normal Inspector controls and transport only.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { promisify } from 'node:util';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const clip = path.join(directory, 'source.mp4');
await promisify(execFile)(
	'ffmpeg',
	[
		'-hide_banner',
		'-loglevel',
		'error',
		'-f',
		'lavfi',
		'-i',
		'testsrc2=s=160x120:r=30:d=4',
		'-c:v',
		'libx264',
		'-pix_fmt',
		'yuv420p',
		clip
	],
	{ windowsHide: true, timeout: 15000 }
);

const native = await openNativeEditor();
const { page } = native;
const errors = [];
const checks = [];
page.on('pageerror', (error) => errors.push(String(error)));
let token, failure;

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
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack.result?.value;
}
const edit = (payload, namespace = 'media') =>
	intent({
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
const graph = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
const create = async (media_kind, label) =>
	(await edit({ kind: 'CreateMedia', media_kind, label })).reference;
async function wait(read, ready, label) {
	const deadline = Date.now() + 10000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}

try {
	token = (await post('/api/ui/session', {})).token;
	const source = await create('Video', 'Auto Next Video');
	const fill = await create('SolidColor', 'Auto Next destination');
	const owner = await create('Matrix', 'Auto Next Matrix');
	const matrix = (operation) => edit({ kind: 'Matrix', media: owner.media, edit: operation });
	const playback = async () => (await matrix({ kind: 'InspectPlayback' })).snapshot;
	const video = async () => (await edit({ kind: 'InspectVideo', media: source.media })).snapshot;
	const initial = (await matrix({ kind: 'Inspect' })).snapshot;
	const row = initial.items[0].id.id;
	const [first, next] = initial.columns.map(({ id }) => id);
	await edit({ kind: 'ConfigureVideoFile', reference: source, file: clip });
	await wait(video, (state) => state.ready, 'Video timing ready');
	await edit({
		kind: 'Video',
		media: source.media,
		edit: { kind: 'Crop', start_ms: 1000, end_ms: 2300 }
	});
	assert.equal((await video()).length_ms, 1300);
	await matrix({
		kind: 'SetCells',
		assignments: [
			{ layer: row, column: first, source },
			{ layer: row, column: next, source: fill }
		]
	});
	const { screen, surface } = await edit(
		{ kind: 'CreateScreen', label: 'Auto Next output' },
		'screens'
	);
	const { snapshot: surfaceState } = await edit(
		{ kind: 'InspectSurface', reference: surface },
		'screens'
	);
	await edit(
		{
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...surfaceState.definition, source: owner }
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
				target: {
					kind: 'Spout',
					sender: `MapGyver-Auto-Next-${randomUUID()}`,
					extent: { width: 160, height: 120 }
				}
			}
		},
		'screens'
	);
	await matrix({
		kind: 'ConfigureColumn',
		column: next,
		definition: { transition: { duration_ms: 1800, curve: 'Linear' } }
	});
	const { cells } = (await matrix({ kind: 'Inspect' })).snapshot;
	const cell = cells.find((entry) => entry.layer === row && entry.definition.column === first);
	const nodes = (await graph()).nodes;
	const byId = new Map(nodes.map((node) => [node.node_id, node]));
	const parameters = nodes
		.find((node) => node.uuid === cell.id)
		.children.map((id) => byId.get(id))
		.find((node) => node.decl_id === 'parameters');
	const id = (field) =>
		parameters.children
			.map((id) => byId.get(id))
			.find((node) => node.decl_id === `parameters/${field}`).node_id;
	const inspector = (field) => page.locator(`.parameter-inspector[data-node-id="${id(field)}"]`);
	const set = (field, value) =>
		intent({ kind: 'setParam', node: id(field), value, behaviour: 'Append' });
	const mode = inspector('auto_next').locator('select');
	const choose = async (value) => {
		await mode.selectOption(value);
		await wait(
			graph,
			({ nodes }) =>
				nodes.find((node) => node.node_id === id('auto_next')).data.param.value.Enum === value,
			'authored mode'
		);
	};
	const tab = (name) => page.getByRole('tab').filter({ hasText: name });
	await tab(/^Medias$/).click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Auto Next Matrix', exact: true })
		.click();
	await tab(/^Matrix(?::|$)/).click();
	await page.locator(`[data-cell-layer="${row}"][data-cell-column="${first}"]`).click();
	await mode.waitFor();
	assert.equal(await mode.inputValue(), 'None');
	assert.equal(await inspector('auto_next_time_ms').count(), 0);
	assert.equal(await inspector('auto_next_lead').count(), 0);
	await choose('Specific Time');
	await inspector('auto_next_time_ms').waitFor();
	assert.equal(await inspector('auto_next_lead').count(), 0);
	await set('auto_next_time_ms', { Float: 120 });
	const launch = () => matrix({ kind: 'LaunchColumn', column: first, mode: 'Cut' });
	const settledHistory = async () =>
		(await wait(graph, ({ history }) => !history.active_edit_session, 'Inspector edit committed'))
			.history;
	const advance = async (trigger = launch) => {
		const history = await settledHistory();
		await trigger();
		const advanced = await wait(
			playback,
			(state) => state.layers.some((request) => request.layer === row && request.column === next),
			'automatic Layer advance'
		);
		assert.equal(advanced.desired_column, first);
		assert.equal(
			advanced.layers.find((request) => request.layer === row).transition.duration_ms,
			1800
		);
		assert.deepEqual(await settledHistory(), history);
		return video();
	};
	await advance();
	checks.push('Inspector defaults, conditional inputs and Specific Time advance without history');
	await matrix({ kind: 'ClearPlayback' });
	await choose('End of Media');
	await inspector('auto_next_lead').waitFor();
	assert.equal(await inspector('auto_next_time_ms').count(), 0);
	const atEnd = await advance();
	assert.equal(atEnd.position_ms, 1300);
	assert.equal(atEnd.playing, false);
	checks.push('End of Media follows the effective 1300 ms crop');
	await matrix({ kind: 'ClearPlayback' });
	await choose('None');
	await set('auto_next_lead', { Bool: true });
	await launch();
	await wait(
		async () => (await edit({ kind: 'InspectVideoSource', reference: source })).status,
		(status) => status.state === 'Ready',
		'actively decoded outgoing Video'
	);
	const beforeLead = await wait(
		video,
		(state) => state.playing && state.position_ms >= 100,
		'outgoing Video starts'
	);
	await set('auto_next', { Enum: 'End of Media' });
	const atLead = await advance(async () => {});
	assert(
		atLead.playing && atLead.position_ms >= beforeLead.position_ms && atLead.position_ms < 1000,
		JSON.stringify(atLead)
	);
	const moved = await wait(
		video,
		(state) => state.playing && state.position_ms > atLead.position_ms + 100,
		'outgoing Video still moves after early advance'
	);
	assert(moved.position_ms < 1300);
	await wait(
		video,
		(state) => state.settings.mode === 'Stopped',
		'outgoing Video stops at retirement'
	);
	checks.push(
		'Transition longer than the crop advances immediately and retains the moving outgoing Video'
	);
	assert.deepEqual(errors, []);
} catch (error) {
	failure = String(error.stack ?? error);
	throw error;
} finally {
	try {
		await native.close();
	} finally {
		await writeFile(
			path.join(directory, 'result.json'),
			JSON.stringify(
				{
					result: failure ? 'fail' : 'pass',
					pid: native.app.pid,
					checks,
					errors,
					failure,
					appLog: failure ? native.log() : undefined
				},
				null,
				2
			)
		);
	}
}
console.log(JSON.stringify({ result: 'pass', checks }));
