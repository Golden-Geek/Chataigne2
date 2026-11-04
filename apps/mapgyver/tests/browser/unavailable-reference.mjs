import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { screenParameterControls } from './screen-parameters.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyUnavailableReferenceUi({ base, edit, post, directory, inspectMedia }) {
	await post('/api/ui/project-new', {});
	const { reference: target } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Reference recovery target'
	});
	await edit({ kind: 'RenameChannel', reference: target, label: 'First recovery output' });
	const { reference: peer } = await edit({
		kind: 'AddChannel',
		media: target.media,
		label: 'Second recovery output'
	});
	const { media: blank } = await edit({
		kind: 'CreateReference',
		target: null,
		label: 'Unconfigured source'
	});
	const { reference: gone } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Removed producer'
	});
	const { media: missing } = await edit({
		kind: 'CreateReference',
		target: gone.media,
		label: 'Missing source'
	});
	await edit({ kind: 'RemoveMedia', media: gone.media });
	const { reference: composition } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Unavailable canvas'
	});
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Unavailable grid'
	});
	const matrixEdit = (operation) => edit({ kind: 'Matrix', media: matrix.media, edit: operation });
	await matrixEdit({ kind: 'AddLayers', parent: null, count: 1 });
	await matrixEdit({ kind: 'AddColumns', count: 2 });
	const inspectMatrix = async () => (await matrixEdit({ kind: 'Inspect' })).snapshot;
	const inspectComposition = async () =>
		(await edit({ kind: 'Composition', reference: composition, edit: { kind: 'Inspect' } }))
			.snapshot;
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'unavailable Reference drops');
	const controls = screenParameterControls(page);
	async function wait(read, predicate) {
		const deadline = Date.now() + 6000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(20);
		}
	}
	const tab = (name) =>
		page
			.getByRole('tab')
			.filter({ hasText: new RegExp(`^${name}$`) })
			.click();
	const manager = page.locator('section[aria-label="Medias"]');
	const select = (label) =>
		manager
			.getByRole('treeitem', { name: label, exact: true })
			.first()
			.getByRole('button', { name: label, exact: true })
			.click();
	async function drop(label, zone, copy, fraction = 0.3) {
		const source = manager
			.getByRole('treeitem', { name: label, exact: true })
			.first()
			.getByRole('button', { name: label, exact: true });
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		try {
			await source.dispatchEvent('dragstart', { dataTransfer: transfer });
			const bounds = await zone.boundingBox();
			assert(bounds);
			const event = {
				dataTransfer: transfer,
				altKey: copy,
				clientX: bounds.x + bounds.width * fraction,
				clientY: bounds.y + bounds.height * fraction
			};
			await wait(
				async () => {
					await zone.dispatchEvent('dragover', event);
					return zone.locator('.drop-hint').count();
				},
				(count) => count === 1
			);
			await zone.dispatchEvent('drop', event);
			await source.dispatchEvent('dragend', { dataTransfer: transfer });
		} finally {
			await transfer.dispose();
		}
	}
	const defaults = [];
	const pins = [];
	try {
		await page.goto(base);
		await tab('Medias');
		for (const [index, source] of [
			{ media: blank, label: 'Unconfigured source' },
			{ media: missing, label: 'Missing source' }
		].entries()) {
			await select('Unavailable canvas');
			await tab('Composition');
			const canvas = page.locator('section[aria-label="Composition"] .source-drop-zone');
			await drop(source.label, canvas, false, 0.2 + index * 0.1);
			const added = await wait(
				inspectComposition,
				(value) => value.instances.length === index * 2 + 1
			);
			defaults.push(added.instances.at(-1).reference.instance);
			assert.equal(added.instances.at(-1).definition.source, null);
			await drop(source.label, canvas, true, 0.65 + index * 0.1);
			const copied = await wait(
				inspectComposition,
				(value) => value.instances.length === index * 2 + 2
			);
			pins.push(copied.instances.at(-1).reference.instance);
			await page
				.locator('.inspector-header .title-text')
				.filter({ hasText: new RegExp('^' + RegExp.escape(source.label) + '$') })
				.waitFor();
			await controls.reference('Target', 'Reference recovery target', 'Reference recovery target');
			await wait(
				inspectComposition,
				(value) => value.instances.at(-1).definition.source?.channel === target.channel
			);
			await controls.reference('Output', 'First recovery output', 'First recovery output');
			assert.equal((await inspectComposition()).instances[index * 2].definition.source, null);
			assert.equal(
				(await inspectMedia()).media.find((entry) => entry.id === source.media).channels.length,
				0
			);
			await select('Unavailable grid');
			await tab('Matrix');
			const grid = page.locator('section[aria-label="Matrix"]');
			await grid.getByRole('gridcell').first().waitFor();
			await drop(source.label, grid.getByRole('gridcell').nth(0).locator('..'), false);
			const before = await inspectMatrix();
			await drop(source.label, grid.getByRole('gridcell').nth(1).locator('..'), true);
			const after = await wait(
				inspectMatrix,
				(value) => value.cells.length === 2 && !isDeepStrictEqual(value, before)
			);
			assert(after.cells.every((cell) => cell.media && cell.definition.source === null));
			await wait(
				() => grid.getByRole('gridcell').allTextContents(),
				(labels) =>
					labels.length === 2 && labels.every((label) => label.trim() === 'Missing source')
			);
			await controls.parameter('Output').waitFor();
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(inspectMatrix, (value) => isDeepStrictEqual(value, before));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(inspectMatrix, (value) => isDeepStrictEqual(value, after));
			await select(source.label);
			await controls.reference('Target', 'Reference recovery target', 'Reference recovery target');
			await wait(
				inspectComposition,
				(value) => value.instances[index * 2].definition.source?.channel === target.channel
			);
			const automatic = await wait(inspectMatrix, (value) =>
				value.cells.some((cell) => cell.definition.source?.channel === target.channel)
			);
			const copiedCell = automatic.cells.find((cell) => cell.definition.source === null);
			assert(copiedCell);
			await edit({ kind: 'ConfigureReference', media: copiedCell.media, target: target.media });
			await wait(inspectMatrix, (value) =>
				value.cells.every((cell) => cell.definition.source?.channel === target.channel)
			);
		}
		await edit({ kind: 'ReorderChannel', reference: peer, after: null });
		await wait(inspectComposition, (value) =>
			value.instances
				.filter((item) => defaults.includes(item.reference.instance))
				.every((item) => item.definition.source?.channel === peer.channel)
		);
		assert(
			(await inspectComposition()).instances
				.filter((item) => pins.includes(item.reference.instance))
				.every((item) => item.definition.source?.channel === target.channel)
		);
		await wait(inspectMatrix, (value) =>
			value.cells.every((cell) => cell.definition.source?.channel === peer.channel)
		);
		const saved = [await inspectComposition(), await inspectMatrix()];
		const project = path.join(directory, 'UnavailableReference.mapgyver');
		await post('/api/ui/project-save', { path: project });
		await post('/api/ui/project-new', {});
		await post('/api/ui/project-load', { path: project });
		assert.deepEqual([await inspectComposition(), await inspectMatrix()], saved);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'missing/unconfigured Reference and Alt-copy drops in Composition/Matrix, generic Target/Output recovery, independent copies, auto/pinned outputs, history and project roundtrip'
			})
		);
	} finally {
		await browser.close();
	}
}
