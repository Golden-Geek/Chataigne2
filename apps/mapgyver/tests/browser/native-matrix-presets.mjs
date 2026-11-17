// Focused normal-app preset workflow; hardware qualification is separate.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
let token;
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(10000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	assert(response.ok);
	return response.json();
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Preset check'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Preset check', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const configurations = page.getByRole('button', { name: 'Matrix configurations', exact: true });
	await configurations.click();
	await page.getByRole('menuitem', { name: 'Capture configuration', exact: true }).click();
	const deadline = Date.now() + 10000;
	let presets;
	for (;;) {
		const { snapshot } = await matrix({
			kind: 'InspectViewport',
			selection: null,
			viewport: { row_start: 0, row_count: 10, axis_start: 0, axis_count: 10 }
		});
		presets = snapshot?.presets;
		if (presets?.length) break;
		assert(Date.now() < deadline, 'Preset missing from Matrix viewport');
		await delay(50);
	}
	const preset = presets[0];
	const { snapshot: original } = await matrix({ kind: 'Inspect' });
	const column = original.columns[0].id;
	const row = original.items.find((item) => item.id.kind === 'Layer').id;
	await matrix({ kind: 'MoveItem', item: row, parent: null, after: original.items.at(-1).id });
	await configurations.click();
	await page.getByRole('menuitem', { name: preset.label, exact: true }).hover();
	await page.getByRole('menuitem', { name: 'Recall configuration', exact: true }).click();
	const { snapshot: recalled } = await matrix({ kind: 'Inspect' });
	assert.deepEqual(
		recalled.items.map((item) => item.id),
		original.items.map((item) => item.id)
	);
	assert.deepEqual(recalled.cells, original.cells);
	assert.equal(recalled.columns[0].id, column);
	const expectedRows = original.items.map((item) => item.id.id);
	async function visibleRowsMatch() {
		const deadline = Date.now() + 10000;
		for (;;) {
			const rows = await page
				.locator('[role="rowheader"][data-row-id]')
				.evaluateAll((nodes) => nodes.map((node) => node.getAttribute('data-row-id')));
			if (JSON.stringify(rows) === JSON.stringify(expectedRows)) return;
			assert(Date.now() < deadline, `Visible rows did not follow recall: ${JSON.stringify(rows)}`);
			await delay(50);
		}
	}
	await visibleRowsMatch();
	const project = path.join(directory, 'presets.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	await post('/api/ui/project-load', { path: project });
	await page.getByText('presets.mapgyver', { exact: true }).waitFor();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Preset check', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	await visibleRowsMatch();
	const result = {
		result: 'pass',
		pid: native.app.pid,
		preset,
		checks: [
			'contextual capture',
			'contextual recall and visible row order',
			'stable Cells',
			'save/reload and reselect Matrix'
		],
		qualification: 'focused normal-use only'
	};
	await page.screenshot({ path: path.join(directory, 'matrix-presets.png') });
	await writeFile(path.join(directory, 'result.json'), JSON.stringify(result, null, 2));
	console.log(JSON.stringify(result));
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failed-native-check.png') }).catch(() => {});
	throw error;
} finally {
	await native.close();
}
