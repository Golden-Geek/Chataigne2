import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixOwnedMediaUi({ base, edit, post, directory, inspectMedia }) {
	await post('/api/ui/project-new', {});
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Matrix drop source'
	});
	await edit({ kind: 'AddChannel', media: source.media, label: 'Second variant' });
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Owned Matrix'
	});
	const operation = (value) => edit({ kind: 'Matrix', media: matrix.media, edit: value });
	await operation({ kind: 'AddLayers', parent: null, count: 1 });
	await operation({ kind: 'AddColumns', count: 2 });
	const inspect = async () => (await operation({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Matrix owned media');
	async function wait(read, predicate) {
		const deadline = Date.now() + 6000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		const manager = page.locator('section[aria-label="Media Manager"]');
		await manager
			.getByRole('treeitem', { name: 'Owned Matrix', exact: true })
			.getByRole('button', { name: 'Owned Matrix', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Matrix$/ })
			.click();
		const panel = page.locator('section[aria-label="Matrix"]');
		await panel.getByRole('gridcell').first().waitFor();
		async function drop(index, copy) {
			const sourceLabel = manager
				.getByRole('treeitem', { name: 'Matrix drop source', exact: true })
				.first()
				.getByRole('button', { name: 'Matrix drop source', exact: true });
			const transfer = await page.evaluateHandle(() => new DataTransfer());
			try {
				await sourceLabel.dispatchEvent('dragstart', { dataTransfer: transfer });
				const cell = panel.getByRole('gridcell').nth(index);
				const zone = cell.locator('..');
				const bounds = await cell.boundingBox();
				assert(bounds);
				const event = {
					dataTransfer: transfer,
					altKey: copy,
					clientX: bounds.x + bounds.width / 2,
					clientY: bounds.y + bounds.height / 2
				};
				await wait(
					async () => {
						await zone.dispatchEvent('dragover', event);
						return zone.locator('.drop-hint').count();
					},
					(count) => count === 1
				);
				await zone.dispatchEvent('drop', event);
				await sourceLabel.dispatchEvent('dragend', { dataTransfer: transfer });
			} finally {
				await transfer.dispose();
			}
		}
		await drop(0, false);
		const first = await wait(inspect, (value) => value.cells.length === 1);
		await page
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Column', { exact: true }) })
			.waitFor();
		await drop(1, true);
		const second = await wait(inspect, (value) => value.cells.length === 2);
		const catalog = await inspectMedia();
		const alias = catalog.media.find(
			(entry) => entry.id === first.cells[0].definition.source.media
		);
		assert.equal(alias.media_kind, 'Reference');
		const copy = catalog.media.find(
			(entry) =>
				entry.id ===
				second.cells.find((cell) => cell.definition.column === initial.columns[1].id).definition
					.source.media
		);
		assert.equal(copy.media_kind, 'SolidColor');
		assert.equal(copy.channels.length, 2);
		assert.notEqual(copy.channels[0].reference.channel, source.channel);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (value) => value.cells.length === 1);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (value) => value.cells.length === 2);
		await panel
			.getByRole('button', { name: `Launch ${initial.columns[0].label}`, exact: true })
			.click();
		await wait(
			async () => (await operation({ kind: 'InspectPlayback' })).snapshot,
			(value) => value.desired_column === initial.columns[0].id
		);
		await drop(0, true);
		const replaced = await wait(
			inspect,
			(value) =>
				value.cells.find((cell) => cell.id === first.cells[0].id).definition.source.media !==
				alias.id
		);
		assert.equal(
			replaced.cells.find((cell) => cell.id === first.cells[0].id).id,
			first.cells[0].id
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			inspect,
			(value) =>
				value.cells.find((cell) => cell.id === first.cells[0].id).definition.source.media ===
				alias.id
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			inspect,
			(value) =>
				value.cells.find((cell) => cell.id === first.cells[0].id).definition.source.media !==
				alias.id
		);
		const project = path.join(directory, 'MatrixOwnedMedia.mapgyver');
		await post('/api/ui/project-save', { path: project });
		await post('/api/ui/project-new', {});
		await post('/api/ui/project-load', { path: project });
		assert.deepEqual(await inspect(), replaced);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Matrix default Reference/Alt-copy drops, typed Cell Inspector, independent variants, launched-cell replacement Undo/Redo and project roundtrip'
			})
		);
	} finally {
		await browser.close();
	}
}
