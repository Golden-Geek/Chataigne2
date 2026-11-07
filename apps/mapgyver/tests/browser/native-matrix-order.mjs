// Actual app hierarchy edits, independently received complete overlapping output images.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';
import { adapterKey, observeNativeAdapter } from './native-adapter.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

const directory = path.resolve(process.argv[2]);
const index = Number(process.argv[3]);
assert(Number.isInteger(index) && index >= 0);
await mkdir(directory);
const probe = path.resolve('target/debug/native_qualification.exe');
const hardware = JSON.parse(
	execFileSync(probe, ['--role', 'hardware'], {
		encoding: 'utf8',
		windowsHide: true
	})
);
assert(hardware.adapters[index] && !hardware.adapters[index].software);
const key = adapterKey(hardware.adapters[index].id);
const native = await openNativeEditor({
	prepareProfile: async (root) => {
		const preferences = path.join(root, 'MapGyver');
		await mkdir(preferences, { recursive: true });
		execFileSync(
			probe,
			[
				'--role',
				'gpu-preferences',
				'--file',
				path.join(preferences, 'preferences.json'),
				'--selected',
				key
			],
			{ windowsHide: true }
		);
	}
});
const page = native.page;
page.setDefaultTimeout(15000);
let token,
	output,
	stage = 'setup';
const errors = [];
page.on('pageerror', (error) => errors.push(String(error)));
const receipts = await observeNativeRejectedRequests(page, () => stage);
async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(10000)
	});
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
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
async function wait(read, ready) {
	const deadline = Date.now() + 15000;
	for (;;) {
		const value = await read();
		if (ready(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(40);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Front first Matrix'
	});
	const matrix = (value) => edit({ kind: 'Matrix', media: reference.media, edit: value });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const rows = initial.items.map((item) => item.id);
	const column = initial.columns[0].id;
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 16, height: 16 } }
	});
	async function assign(row, rgba) {
		const { reference: source } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: `Order ${rgba.join('-')}`
		});
		await edit({
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba }
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: source.media,
			size: { kind: 'Fixed', extent: { width: 16, height: 16 } }
		});
		await matrix({
			kind: 'ConfigureLayer',
			layer: row.id,
			settings: { size: { width: 16, height: 16 }, position: [0, 0], duration_ms: 250 }
		});
		await matrix({ kind: 'SetCells', assignments: [{ layer: row.id, column, source }] });
	}
	for (const [i, rgba] of [
		[1, 0, 0, 1],
		[0, 0, 1, 1],
		[0, 1, 0, 1]
	].entries())
		await assign(rows[i], rgba);
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}(?::|$)`) });
	async function show() {
		await tab('Medias').click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Front first Matrix', exact: true })
			.click();
		await tab('Matrix').click();
	}
	await show();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	const uiOrder = () =>
		grid
			.locator('.row-label')
			.evaluateAll((elements) => elements.map((element) => element.dataset.rowId));
	async function order(ids) {
		await wait(uiOrder, (actual) => JSON.stringify(actual) === JSON.stringify(ids));
	}
	async function pickFront(expected) {
		await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
		const panel = page.locator(
			'.workbench-editor[data-panel-id="composition"][data-spatial-kind="Matrix"]'
		);
		await wait(
			() => panel.getAttribute('aria-busy'),
			(value) => value === 'false'
		);
		const rect = panel.locator(`[data-layer-id="${expected}"]`);
		await rect.waitFor();
		const bounds = await rect.boundingBox();
		assert(bounds);
		await page.mouse.click(bounds.x + bounds.width * 0.43, bounds.y + bounds.height * 0.43);
		await wait(
			() => panel.getAttribute('data-spatial-selected'),
			(value) => value === expected
		);
		await tab('Matrix').click();
	}
	await matrix({ kind: 'LaunchColumn', column, mode: 'Cut' });
	output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	const { screen } = await output.demand(reference);
	assert.equal((await observeNativeAdapter(page, hardware)).key, key);
	async function pixels(label, color) {
		stage = label;
		// Settling only; no frame-rate, latency or presentation-budget claim.
		await delay(700);
		await output.observePixels(label, color, 2);
	}
	await order(rows.map((row) => row.id));
	await pixels('root-first-red', [255, 0, 0, 255]);
	await pickFront(rows[0].id);
	await matrix({ kind: 'MoveItem', item: rows[0], parent: null, after: rows[1] });
	await order([rows[1].id, rows[0].id, rows[2].id]);
	await pixels('move-first-blue', [0, 0, 255, 255]);
	await pickFront(rows[1].id);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await order(rows.map((row) => row.id));
	await pixels('undo-red', [255, 0, 0, 255]);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await order([rows[1].id, rows[0].id, rows[2].id]);
	await pixels('redo-blue', [0, 0, 255, 255]);

	const { items: groups } = await matrix({ kind: 'AddGroup', parent: null, label: 'Front Group' });
	const group = groups[0];
	const { items: children } = await matrix({ kind: 'AddLayers', parent: group.id, count: 2 });
	await assign(children[0], [1, 1, 0, 1]);
	await assign(children[1], [0, 1, 1, 1]);
	await matrix({ kind: 'DropItem', item: group, target: { kind: 'Before', item: rows[1] } });
	await matrix({ kind: 'LaunchColumn', column, mode: 'Cut' });
	await order([group.id, ...children.map((row) => row.id), rows[1].id, rows[0].id, rows[2].id]);
	await pixels('group-first-yellow', [255, 255, 0, 255]);
	await matrix({ kind: 'MoveItem', item: children[1], parent: group.id, after: null });
	await order([group.id, children[1].id, children[0].id, rows[1].id, rows[0].id, rows[2].id]);
	await pixels('group-reordered-cyan', [0, 255, 255, 255]);
	await matrix({ kind: 'DropItem', item: children[0], target: { kind: 'Before', item: group } });
	await order([children[0].id, group.id, children[1].id, rows[1].id, rows[0].id, rows[2].id]);
	await pixels('reparent-front-yellow', [255, 255, 0, 255]);
	await matrix({
		kind: 'ConfigureItem',
		item: children[0],
		settings: { expose_channel: false, enabled: true, opacity: 0.5, blend: 'Normal' }
	});
	await pixels('front-half-yellow-over-cyan', [188, 255, 188, 255]);
	await matrix({
		kind: 'ConfigureItem',
		item: children[0],
		settings: { expose_channel: false, enabled: false, opacity: 0.5, blend: 'Normal' }
	});
	await pixels('disabled-front-cyan', [0, 255, 255, 255]);
	const authored = await inspect();
	const project = path.join(directory, 'Front-first.mapgyver');
	assert((await post('/api/ui/project-save', { path: project })).ok);
	const loaded = await post('/api/ui/project-load', { path: project });
	assert.equal(path.resolve(loaded.path), project);
	await show();
	assert.deepEqual(await inspect(), authored);
	await order(authored.items.map((item) => item.id.id));
	await matrix({ kind: 'LaunchColumn', column, mode: 'Cut' });
	await pixels('reloaded-cyan', [0, 255, 255, 255]);
	await matrix({
		kind: 'ConfigureItem',
		item: children[0],
		settings: { expose_channel: false, enabled: true, opacity: 1, blend: 'Normal' }
	});
	await pixels('reloaded-front-enabled-yellow', [255, 255, 0, 255]);
	await page.screenshot({ path: path.join(directory, 'front-first.png') });
	assert.deepEqual(errors, []);
	assert.deepEqual(receipts.rejected, []);
	await output.finish();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				pid: native.app.pid,
				adapter: output.adapter,
				screen,
				observations: output.observations,
				errors,
				receipts
			},
			null,
			2
		)
	);
	console.log(
		JSON.stringify({
			passed: true,
			pid: native.app.pid,
			gpu: key,
			stages: output.observations.length
		})
	);
} catch (error) {
	await page.screenshot({ path: path.join(directory, 'failure.png') }).catch(() => {});
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify(
			{
				stage,
				error: String(error),
				stack: error.stack,
				observations: output?.observations,
				errors,
				receipts,
				log: native.log()
			},
			null,
			2
		)
	);
	throw error;
} finally {
	await output?.finish();
	await native.close();
}
