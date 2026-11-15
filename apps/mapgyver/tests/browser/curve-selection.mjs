// Production Curve UI against the ordinary app's real transport, graph and history.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';
import { openInitialScreenHeadless } from '../headless/initial-screen-host.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory);
assert(process.env.MAPGYVER_TEST_BINARY);
const host = await openInitialScreenHeadless(directory, { frontend: true });
let browser,
	page,
	token,
	stage = 'setup';
const checks = [],
	errors = [],
	receipts = [];
async function post(route, body) {
	const response = await fetch(host.base + route, {
		method: 'POST',
		signal: AbortSignal.timeout(6000),
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body)
	});
	const value = await response.json();
	assert(response.ok, `${route}: ${JSON.stringify(value)}`);
	return value;
}
const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
const subtree = (root, max_depth = 8) =>
	post('/api/ui/snapshot', { scope: { subtree: { root, max_depth } } });
async function intent(value) {
	const ack = await post('/api/ui/intent', value);
	assert(ack.success, JSON.stringify(ack));
	return ack;
}
async function operation(payload) {
	return (
		await intent({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload
		})
	).result.value;
}
async function wait(read, test) {
	const deadline = Date.now() + 7000;
	for (;;) {
		const value = await read();
		if (test(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(20);
	}
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference } = await operation({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Curve acceptance'
	});
	const matrix = (
		await operation({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } })
	).snapshot;
	const layerId = matrix.items.find((item) => item.id.kind === 'Layer').id.id;
	const layer = (await snapshot()).nodes.find((node) => node.uuid === layerId);
	assert(layer);
	const fields = (await subtree(layer.node_id)).nodes;
	const opacity = fields.find((node) => node.decl_id === 'parameters/opacity');
	assert(opacity, JSON.stringify(fields.map((node) => [node.decl_id, node.node_type])));
	await intent({
		kind: 'setParamControlState',
		node: opacity.node_id,
		state: { mode: 'animation', spec: { mode: 'animation' } }
	});
	const animationFields = (await subtree(opacity.node_id)).nodes;
	const waveform = animationFields.find((node) => node.decl_id === 'waveform');
	assert(waveform);
	await intent({
		kind: 'setParam',
		node: waveform.node_id,
		value: { Enum: 'curve' },
		behaviour: 'Append'
	});
	const graph = await wait(
		() => subtree(opacity.node_id),
		(value) => value.nodes.some((node) => node.node_type === 'animation_curve')
	);
	const curve = graph.nodes.find((node) => node.node_type === 'animation_curve');
	const curveState = async () => {
		const nodes = (await subtree(curve.node_id)).nodes;
		const identities = new Map(nodes.map((node) => [node.node_id, node.uuid]));
		return nodes
			.map((node) => ({
				uuid: node.uuid,
				type: node.node_type,
				decl: node.decl_id,
				label: node.meta.label,
				children: (node.children ?? []).map((id) => identities.get(id)),
				value: node.data?.param?.value
			}))
			.sort((a, b) => a.uuid.localeCompare(b.uuid));
	};
	const keys = async () =>
		(await subtree(curve.node_id)).nodes.filter((node) => node.node_type === 'animation_curve_key');
	browser = await chromium.launch({ headless: true });
	page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(7000);
	page.on('pageerror', (error) => errors.push(String(error)));
	page.on('websocket', (socket) =>
		socket.on('framereceived', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				const ack = frame.update?.acknowledgement;
				if (ack && (!ack.result || !ack.success || ack.created_node)) receipts.push(ack);
			} catch {}
		})
	);
	await page.goto(host.base);
	const media = page.locator('section[aria-label="Medias"]');
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias(?::|$)/ })
		.click();
	await media.getByRole('button', { name: 'Curve acceptance', exact: true }).click();
	const matrixRow = media.locator(
		`[role="treeitem"][data-tree-node-id="${(await snapshot()).nodes.find((node) => node.uuid === reference.media).node_id}"]`
	);
	if (await matrixRow.count()) {
		if ((await matrixRow.getAttribute('aria-expanded')) === 'false')
			await matrixRow.press('ArrowRight');
	}
	await media.getByRole('button', { name: layer.meta.label, exact: true }).click();
	stage = 'open typed animation Curve Inspector';
	const current = await snapshot();
	const chain = [curve];
	for (let child = curve; child.node_id !== layer.node_id;) {
		child = current.nodes.find((node) => node.children?.includes(child.node_id));
		assert(child, 'Curve must remain inside the authored Layer');
		chain.unshift(child);
	}
	for (const node of chain) {
		const section = page
			.locator(`.inspector .node-inspector[data-node-id="${node.node_id}"]`)
			.first();
		if (await section.count()) {
			const title = section.locator('.node-title').first();
			if ((await title.getAttribute('aria-checked')) === 'false') await title.click();
		}
	}
	const canvas = page.locator('.inspector canvas.curve-canvas').first();
	await canvas.waitFor();
	await canvas.scrollIntoViewIfNeeded();
	const owned = page.locator(`.inspector .node-inspector[data-node-id="${curve.node_id}"]`).first();
	await owned.getByTitle('Select Node in Outliner', { exact: true }).first().click();
	await canvas.waitFor();
	await canvas.scrollIntoViewIfNeeded();
	await wait(
		() => page.locator('.curve-bounds-readout').first().textContent(),
		(text) => !text.includes('min: -]')
	);
	await page.evaluate(
		() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
	);
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	const history = async () => (await snapshot()).history;
	const before = await curveState(),
		beforeKeys = await keys(),
		beforeHistory = await history();
	stage = 'real key creation and exact history';
	const box = await canvas.boundingBox();
	assert(box);
	await canvas.click({
		position: { x: box.width * 0.6, y: box.height * 0.6 },
		modifiers: ['Control']
	});
	await wait(keys, (value) => value.length === beforeKeys.length + 1);
	await wait(history, (value) => !value.active_edit_session);
	const after = await curveState();
	assert.equal(
		(await history()).undo_len,
		beforeHistory.undo_len + 1,
		'key initialization is one actual history step'
	);
	const created = (await keys()).find((node) => !beforeKeys.some((old) => old.uuid === node.uuid));
	assert(created);
	const creationReceipts = receipts.filter((ack) => ack.created_node);
	assert.equal(creationReceipts.length, 1);
	assert.deepEqual(creationReceipts[0].created_node, {
		node_id: created.node_id,
		uuid: created.uuid
	});
	await page
		.locator(`.selected-key-node-inspector [data-node-id="${created.node_id}"]`)
		.first()
		.waitFor();
	checks.push({
		stage,
		created: { uuid: created.uuid, node_id: created.node_id },
		one_history_step: true,
		typed_inspector: true
	});
	stage = 'primary-down shared selection, live drag and owner Escape';
	await canvas.click({ position: { x: box.width * 0.3, y: box.height * 0.85 } });
	await page.locator('.selected-key-node-inspector').waitFor({ state: 'detached' });
	const original = await canvas.elementHandle();
	await canvas.scrollIntoViewIfNeeded();
	const currentBox = await canvas.boundingBox();
	const x = currentBox.x + currentBox.width * 0.6,
		y = currentBox.y + currentBox.height * 0.6;
	await page.mouse.move(x, y);
	await page.mouse.down();
	await page
		.locator(`.selected-key-node-inspector [data-node-id="${created.node_id}"]`)
		.first()
		.waitFor();
	assert(
		await canvas.evaluate((node, previous) => node === previous, original),
		'shared selection retains the captured canvas'
	);
	await page.mouse.move(x + 24, y - 12, { steps: 4 });
	await wait(curveState, (value) => JSON.stringify(value) !== JSON.stringify(after));
	assert(
		(await history()).active_edit_session,
		'real Parameter changes arrive before pointer release'
	);
	await page.mouse.up();
	await wait(history, (value) => !value.active_edit_session);
	const dragged = await curveState();
	assert.equal(
		(await history()).undo_len,
		beforeHistory.undo_len + 2,
		'one history item per real drag'
	);
	await canvas.focus();
	await page.keyboard.press('Escape');
	await page.locator('.selected-key-node-inspector').waitFor({ state: 'detached' });
	await page
		.locator('.inspector-header .title-text')
		.filter({ hasText: /^Curve$/ })
		.waitFor();
	checks.push({
		stage,
		selection_before_release: true,
		canvas_retained: true,
		live_backend_write: true,
		one_history_step: true,
		owner_inspector: true
	});
	stage = 'exact creation and drag Undo/Redo';
	await undo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(after));
	await redo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(dragged));
	await undo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(after));
	await undo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(before));
	await redo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(after));
	await redo();
	await wait(curveState, (value) => JSON.stringify(value) === JSON.stringify(dragged));
	checks.push({
		stage,
		exact_undo_redo: true
	});
	assert.deepEqual(errors, []);
	assert.deepEqual(
		receipts.filter((ack) => !ack.success),
		[]
	);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: 'pass',
				pid: host.app.pid,
				checks,
				errors,
				receipt_count: receipts.length,
				native_pixels: false
			},
			null,
			2
		)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, error: String(error), checks, errors, receipts }, null, 2)
	);
	if (page)
		await writeFile(
			path.join(directory, 'page.txt'),
			await page
				.locator('body')
				.innerText()
				.catch(() => 'unavailable')
		);
	throw error;
} finally {
	await browser?.close();
	await host.close();
	await writeFile(path.join(directory, 'host.log'), host.log());
}
