// Verify actual desktop selection after UI-created authored changes.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2] ?? 'target/mapgyver-creation-selection');
await mkdir(directory, { recursive: true });
const native = await openNativeEditor();
const { page } = native;
const errors = [],
	checks = [];
let token;
let stage = 'setup';
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

async function wait(read, accepts) {
	const deadline = Date.now() + 7000;
	for (;;) {
		const value = await read();
		if (accepts(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(40);
	}
}

const manager = page.locator('section[data-panel-id="media-manager"]');
const mediaRow = (name) => manager.getByRole('button', { name, exact: true });
const menu = (name) => page.getByRole('menuitem', { name, exact: true });
const inspector = page.locator('.inspector[data-inspected-uuid]');
const inspected = (id) =>
	inspector.locator(`xpath=self::*[@data-inspected-uuid="${id}"]`).waitFor();
const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });

try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Selection source'
	});
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Selection Matrix'
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await mediaRow('Selection source').click();
	await inspected(source.media);
	const channels = page.getByRole('region', { name: 'Channels', exact: true });

	stage = 'keep the selected parent when adding Channels by button and context menu';
	await channels.getByRole('button', { name: 'Add child item', exact: true }).click();
	await channels.locator('xpath=self::*[@data-channel-count="2"]').waitFor();
	await inspected(source.media);
	await channels.locator(':scope > header').click({ button: 'right' });
	await menu('Add').hover();
	await menu('Channel').click();
	await channels.locator('xpath=self::*[@data-channel-count="3"]').waitFor();
	await inspected(source.media);
	checks.push('Both Channel add actions keep the owning Media inspected');

	stage = 'explicitly inspect the newly created Channel';
	const { page: channelPage } = await edit({
		kind: 'InspectChannels',
		query: { media: source.media, start: 0, count: 8 }
	});
	const added = channelPage.channels.at(-1);
	await channels.getByText(added.label, { exact: true }).click({ button: 'right' });
	await menu('Watch in Inspector').click();
	await inspected(added.reference.channel);
	checks.push('Watch in Inspector still opens the exact Channel');

	stage = 'inspect the Matrix Cell after adding its owned Media';
	await mediaRow('Selection Matrix').click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix(?::|$)/ })
		.click();
	const matrixSnapshot = async () =>
		(await edit({ kind: 'Matrix', media: matrix.media, edit: { kind: 'Inspect' } })).snapshot;
	const initial = await matrixSnapshot();
	const layer = initial.items[0];
	const destination = (index) =>
		grid.getByTitle(`${layer.label} / ${initial.columns[index].label}`, { exact: true });
	const authored = (index) =>
		wait(matrixSnapshot, (snapshot) =>
			snapshot.cells.some(
				(cell) => cell.definition.column === initial.columns[index].id && cell.media
			)
		).then((snapshot) =>
			snapshot.cells.find((cell) => cell.definition.column === initial.columns[index].id)
		);
	const position = async (cell) => {
		const box = await cell.boundingBox();
		assert(box);
		return { x: box.width * 0.2, y: box.height * 0.75 };
	};
	await destination(0).waitFor();
	await destination(0).hover({ position: await position(destination(0)) });
	await grid
		.getByRole('button', {
			name: `Add Media to ${layer.label} / ${initial.columns[0].label}`,
			exact: true
		})
		.click();
	await menu('Solid Color').click();
	const first = await authored(0);
	await inspected(first.id);
	await wait(
		() => destination(0).getAttribute('aria-selected'),
		(selected) => selected === 'true'
	);
	checks.push('Matrix Add Media selects the exact Cell and highlights its address');

	stage = 'inspect the nested Media explicitly and check related Matrix creation routes';
	await destination(0).click({ button: 'right', position: await position(destination(0)) });
	await menu('Inspect media').click();
	await inspected(first.media);
	await destination(1).click({ button: 'right', position: await position(destination(1)) });
	await menu('Add Media').hover();
	await menu('Solid Color').click();
	await inspected((await authored(1)).id);
	checks.push('Inspect media opens the child, and the Matrix context menu selects its new Cell');

	stage = 'select the destination Cell after a Media source drop';
	const matrixPanel = grid.locator('xpath=ancestor::section[1]');
	await wait(
		() => matrixPanel.getAttribute('aria-busy'),
		(busy) => busy === 'false'
	);
	const cell = destination(2);
	const dropZone = cell.locator('..');
	const rect = await cell.boundingBox();
	assert(rect);
	const transfer = await page.evaluateHandle(() => new DataTransfer());
	const sourceRow = mediaRow('Selection source');
	const drop = {
		dataTransfer: transfer,
		clientX: rect.x + rect.width / 2,
		clientY: rect.y + rect.height / 2
	};
	try {
		await sourceRow.dispatchEvent('dragstart', { dataTransfer: transfer });
		await wait(
			async () => {
				await dropZone.dispatchEvent('dragover', drop);
				return dropZone.locator('.drop-hint').count();
			},
			(hints) => hints === 1
		);
		await dropZone.dispatchEvent('drop', drop);
		await sourceRow.dispatchEvent('dragend', { dataTransfer: transfer });
	} finally {
		await transfer.dispose();
	}
	await inspected((await authored(2)).id);
	checks.push('Dropping a top-level Media keeps the destination Cell inspected');
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', checks, errors }, null, 2)
	);
	console.log(JSON.stringify({ result: 'pass', checks }));
} catch (error) {
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ stage, checks, errors, failure: String(error), log: native.log() }, null, 2)
	);
	throw error;
} finally {
	await native.close();
}
