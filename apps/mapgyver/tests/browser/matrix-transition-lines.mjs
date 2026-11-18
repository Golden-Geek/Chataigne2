import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
const native = await openNativeEditor();
const { page } = native;
const errors = [];
const checks = [];
const samples = {};
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
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}
async function edit(payload, namespace = 'media') {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: `mapgyver.${namespace}`,
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
async function wait(read, ready, label) {
	const deadline = Date.now() + 8000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${label}: ${JSON.stringify(value)}`);
		await delay(25);
	}
}
const input = (id, mode = 'Configured') => ({ kind: 'LaunchColumn', column: id, mode });

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Line Test'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 160, height: 120 } }
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const sources = [];
	for (const label of ['First fill', 'Second fill', 'Third fill']) {
		const { reference: source } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: source.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 48 } }
		});
		sources.push(source);
	}
	await matrix({ kind: 'AddLayers', parent: null, count: 40 });
	const { snapshot } = await matrix({ kind: 'Inspect' });
	const columns = snapshot.columns.map((column) => column.id);
	assert.equal(columns.length, 3);
	const [a, b, c] = columns;
	const near = snapshot.items[0].id.id;
	const far = snapshot.items.at(-1).id.id;
	await matrix({
		kind: 'SetCells',
		assignments: [near, far].flatMap((layer) =>
			columns.map((column, index) => ({ layer, column, source: sources[index] }))
		)
	});
	for (const [column, duration_ms, curve] of [
		[b, 2200, 'Linear'],
		[c, 4400, 'Smooth']
	]) {
		await matrix({
			kind: 'ConfigureColumn',
			column,
			definition: { transition: { duration_ms, curve } }
		});
	}
	await page.evaluate(async () => {
		const host = window.__TAURI_INTERNALS__;
		window.__matrixLineStatuses = [];
		await host.invoke('plugin:event|listen', {
			event: 'mapgyver-preview-status',
			target: { kind: 'Any' },
			handler: host.transformCallback((event) => (window.__matrixLineStatuses = event.payload))
		});
	});
	const tab = (name) => page.getByRole('tab').filter({ hasText: name });
	await tab(/^Medias$/).click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Line Test', exact: true })
		.click();
	await tab(/^Matrix(?::|$)/).click();
	const grid = page.locator('[data-panel-id="matrix"]').getByRole('grid');
	const read = (layer) =>
		grid.evaluate(
			(element, { layer, columns, reference }) => {
				const readLines = (container) => {
					const lines = { incoming: 0, outgoing: 0, anchored: true };
					if (!container) return null;
					for (const bar of container.querySelectorAll('[data-matrix-weight]')) {
						const rect = bar.getBoundingClientRect();
						const bounds = container.getBoundingClientRect();
						const side = bar.dataset.matrixWeight;
						const width = bar.offsetWidth;
						lines[side] = width > 0 ? rect.width / width : 0;
						lines.anchored &&=
							Math.abs(side === 'incoming' ? rect.left - bounds.left : rect.right - bounds.right) <=
							2;
					}
					return lines;
				};
				const statuses = window.__matrixLineStatuses;
				const forMatrix = (kind) =>
					statuses.find(
						(status) =>
							status.source.kind === kind && status.source.reference.channel === reference.channel
					);
				return {
					transport: forMatrix('MatrixPlayback')?.matrix?.desired?.playback?.desired_column ?? null,
					final: forMatrix('Channel')?.matrix?.submitted?.playback?.desired_column ?? null,
					headers: columns.map((column) =>
						readLines(element.querySelector(`[role="columnheader"][data-column-id="${column}"]`))
					),
					cells: columns.map((column) =>
						readLines(
							element.querySelector(
								`[data-cell-layer="${layer}"][data-cell-column="${column}"] [role="gridcell"]`
							)
						)
					)
				};
			},
			{ layer, columns, reference }
		);
	const row = (id) => grid.locator(`[role="row"][data-row-id="${id}"]`);
	await row(near).waitFor();
	assert.equal(await row(far).count(), 0, 'far row is unmounted before the transition');
	await matrix(input(a, 'Cut'));
	samples.first = await wait(
		() => read(near),
		(value) =>
			value.transport === a && value.headers[0]?.incoming > 0.98 && value.cells[0]?.incoming > 0.98,
		'initial Cut'
	);
	assert.equal(
		samples.first.final,
		null,
		'a tiled editor observes playback without submitting a Final output'
	);
	checks.push('editor-only playback and full-width Cut');

	await matrix(input(b));
	samples.fade = await wait(
		() => read(near),
		(value) =>
			value.transport === b &&
			value.headers[1]?.incoming > 0.2 &&
			value.headers[1]?.incoming < 0.55,
		'running crossfade'
	);
	const [outgoing, incoming] = samples.fade.headers;
	assert(
		outgoing.anchored && incoming.anchored && samples.fade.cells.every((cell) => cell.anchored)
	);
	assert(Math.abs(outgoing.outgoing + incoming.incoming - 1) < 0.03);
	assert(Math.abs(samples.fade.cells[0].outgoing - outgoing.outgoing) < 0.03);
	assert(Math.abs(samples.fade.cells[1].incoming - incoming.incoming) < 0.03);
	checks.push('left/right anchors and complementary Cell and Column weights');

	await matrix(input(c));
	samples.interrupted = await wait(
		() => read(near),
		(value) =>
			value.transport === c && value.headers[2]?.incoming > 0 && value.headers[1]?.outgoing > 0,
		'interrupted fade'
	);
	const [first, second, third] = samples.interrupted.headers;
	assert(
		first.outgoing > 0.15 &&
			first.outgoing < 0.85 &&
			second.outgoing > 0.15 &&
			second.outgoing < 0.85
	);
	assert(Math.abs(first.outgoing + second.outgoing + third.incoming - 1) < 0.03);
	assert.equal(second.incoming, 0);
	checks.push('interruption starts each outgoing line from its in-flight weight');

	await grid.evaluate((element) => (element.scrollTop = element.scrollHeight));
	await row(far).waitFor();
	assert.equal(await row(near).count(), 0);
	samples.virtual = await wait(
		() => read(far),
		(value) =>
			value.transport === c && value.cells[2]?.incoming > 0.02 && value.cells[2]?.incoming < 0.97,
		'virtual row enters mid-fade'
	);
	assert(samples.virtual.cells[2].anchored);
	assert(Math.abs(samples.virtual.cells[2].incoming - samples.virtual.headers[2].incoming) < 0.03);
	checks.push('newly mounted virtual Cell displays current progress');

	await matrix(input(a, 'Cut'));
	samples.cut = await wait(
		() => read(far),
		(value) =>
			value.transport === a &&
			value.cells[0]?.incoming > 0.98 &&
			value.headers[0]?.incoming > 0.98 &&
			value.headers.slice(1).every((line) => line.outgoing === 0 && line.incoming === 0),
		'Cut clears interrupted lines'
	);
	assert(samples.cut.cells.slice(1).every((line) => line.outgoing === 0 && line.incoming === 0));
	checks.push('Cut clears all interrupted contributions immediately');
	const { screen, surface } = await edit(
		{ kind: 'CreateScreen', label: 'Matrix line output' },
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
			definition: { ...surfaceState.definition, source: reference }
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
					sender: `MapGyver-Matrix-Line-${randomUUID()}`,
					extent: { width: 160, height: 120 }
				}
			}
		},
		'screens'
	);
	await wait(
		() => read(far),
		(value) => value.final === a,
		'Final starts submitting'
	);
	await matrix(input(b));
	samples.submitted = await wait(
		() => read(far),
		(value) =>
			value.final === b && value.headers[1]?.incoming > 0.1 && value.headers[1]?.incoming < 0.9,
		'weights from the submitted Final plan'
	);
	assert(
		Math.abs(samples.submitted.cells[1].incoming - samples.submitted.headers[1].incoming) < 0.03
	);
	samples.settled = await wait(
		() => read(far),
		(value) =>
			value.headers[1]?.incoming > 0.99 &&
			value.cells[1]?.incoming > 0.99 &&
			value.headers[0]?.outgoing < 0.01 &&
			value.cells[0]?.outgoing < 0.01,
		'natural fade completion'
	);
	checks.push(
		'the corresponding submitted Final plan drives the lines with an active Spout output'
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
					samples,
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
