// Actual File menu replacement and browser upload. Backend requests only observe committed state.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixProjectUi({ base, edit, inspectMedia }) {
	const project = path.resolve('target/m07-editor-review/MatrixReview.mapgyver');
	const review = JSON.parse(
		await readFile(path.resolve('target/m07-editor-review/review.json'), 'utf8')
	);
	const projectHash = async () =>
		createHash('sha256')
			.update(await readFile(project))
			.digest('hex');
	const originalHash = await projectHash();
	const uploadName = `MatrixReview-${randomUUID()}.mapgyver`;
	assert.equal(
		originalHash,
		review.project_sha256,
		'use the existing verified review fixture unchanged'
	);
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	try {
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		const operation = async (media, value) =>
			(await edit({ kind: 'Matrix', media, edit: value })).snapshot;
		const inspect = (media) => operation(media, { kind: 'Inspect' });
		const playback = (media) => operation(media, { kind: 'InspectPlayback' });
		const window = (media) =>
			operation(media, {
				kind: 'InspectViewport',
				viewport: { row_start: 0, row_count: 16, column_start: 0, column_count: 8 }
			});
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Matrix$/ });
		await tab.click();
		const panel = page.getByRole('region', { name: 'Matrix', exact: true });
		const button = (name) => panel.getByRole('button', { name, exact: true });
		const grid = panel.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const undo = page.getByTitle('Ctrl/Cmd+Z', { exact: true });
		const redo = page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true });
		const menuItem = (label) =>
			page.getByRole('menuitem').filter({ has: page.getByText(label, { exact: true }) });
		const openFile = () =>
			page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		async function newProject() {
			await openFile();
			await menuItem('New').click();
			await wait(inspectMedia, (value) => value.media.length === 0 && value.assets.length === 0);
			await wait(() => button('Add Matrix').isEnabled(), Boolean);
			await wait(
				() => grid.count(),
				(value) => value === 0
			);
			await wait(
				() => panel.getByLabel('Row name', { exact: true }).count(),
				(value) => value === 0
			);
			await wait(
				() => panel.getByLabel('Column name', { exact: true }).count(),
				(value) => value === 0
			);
			await wait(
				() => panel.getByLabel('Matrix media', { exact: true }).inputValue(),
				(value) => value === ''
			);
			await wait(() => undo.isDisabled(), Boolean);
			await wait(() => redo.isDisabled(), Boolean);
			await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		}
		async function createSelectedRow() {
			const before = await inspectMedia();
			await button('Add Matrix').click();
			const catalog = await wait(inspectMedia, (value) =>
				value.media.some(
					(entry) =>
						entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
				)
			);
			const root = catalog.media.find(
				(entry) => entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
			);
			await wait(
				() => panel.getByLabel('Matrix media', { exact: true }).inputValue(),
				(value) => value === root.id
			);
			await button('Add layer').click();
			const authored = await wait(
				() => inspect(root.id),
				(value) => value.items.length === 1
			);
			await button('Add column').click();
			const snapshot = await wait(
				() => inspect(root.id),
				(value) => value.columns.length === 1
			);
			await grid.getByTitle(`Select ${authored.items[0].label}`, { exact: true }).click();
			await grid.getByTitle(`Select ${snapshot.columns[0].label}`, { exact: true }).click();
			await wait(
				() => panel.getByLabel('Row name', { exact: true }).inputValue(),
				(value) => value === authored.items[0].label
			);
			return snapshot;
		}
		async function rename(label) {
			const form = panel
				.locator('form')
				.filter({ has: page.getByRole('textbox', { name: 'Row name', exact: true }) });
			await form.getByLabel('Row name', { exact: true }).fill(label);
			await form.getByRole('button', { name: 'Rename', exact: true }).click();
			await grid.getByTitle(`Select ${label}`, { exact: true }).waitFor({ state: 'attached' });
		}
		stage = 'active Matrix replaced through File New';
		const first = await createSelectedRow();
		await rename('Before replacement');
		await button('Cut').click();
		const active = await wait(
			() => playback(first.media),
			(value) => value.desired_column === first.columns[0].id
		);
		const firstEpoch = (await window(first.media)).project;
		await newProject();
		assert.deepEqual(await inspectMedia(), { assets: [], media: [] });
		stage = 'fresh editor identities and history after replacement';
		const second = await createSelectedRow();
		assert.notEqual(second.media, first.media);
		assert.notEqual(second.final_channel.channel, first.final_channel.channel);
		assert.notEqual(second.items[0].id.id, first.items[0].id.id);
		assert.notEqual(second.columns[0].id, first.columns[0].id);
		assert.equal((await playback(second.media)).desired_column, null);
		const secondEpoch = (await window(second.media)).project;
		assert.notEqual(secondEpoch, firstEpoch);
		await rename('After replacement');
		const renamed = await wait(
			() => inspect(second.media),
			(value) => value.items[0].label === 'After replacement'
		);
		await undo.click();
		await wait(
			() => inspect(second.media),
			(value) => isDeepStrictEqual(value, second)
		);
		await redo.click();
		await wait(
			() => inspect(second.media),
			(value) => isDeepStrictEqual(value, renamed)
		);
		assert.equal((await playback(second.media)).desired_column, null);
		stage = 'actual browser File Open upload';
		await openFile();
		await menuItem('Open').hover();
		const choosing = page.waitForEvent('filechooser');
		await menuItem('Load From...').click();
		const chooser = await choosing;
		await chooser.setFiles({
			name: uploadName,
			mimeType: 'application/octet-stream',
			buffer: await readFile(project)
		});
		await wait(inspectMedia, (value) =>
			value.media.some((entry) => entry.id === review.root.media)
		);
		await wait(
			() => panel.getByLabel('Matrix media', { exact: true }).inputValue(),
			(value) => value === review.root.media
		);
		await wait(
			() => panel.getByLabel('Row name', { exact: true }).count(),
			(value) => value === 0
		);
		await wait(
			() => panel.getByLabel('Column name', { exact: true }).count(),
			(value) => value === 0
		);
		const loaded = await inspect(review.root.media);
		assert.equal(loaded.items.length, review.rows);
		assert.equal(loaded.cells.length, review.populated_cells);
		assert.deepEqual(
			loaded.columns.map((value) => value.id),
			review.columns
		);
		assert.deepEqual(loaded.final_channel, review.root);
		assert(!loaded.items.some((value) => value.id.id === second.items[0].id.id));
		assert.equal((await playback(review.root.media)).desired_column, null);
		const loadedEpoch = (await window(review.root.media)).project;
		assert.notEqual(loadedEpoch, secondEpoch);
		assert.notEqual(loadedEpoch, firstEpoch);
		const feedback = panel.getByRole('region', { name: 'Matrix runtime feedback', exact: true });
		await wait(
			() => feedback.innerText(),
			(value) => value.includes('Requested column: None')
		);
		assert((await feedback.innerText()).includes('No native Matrix observation.'));
		await wait(() => undo.isDisabled(), Boolean);
		await wait(() => redo.isDisabled(), Boolean);
		stage = 'loaded editor mutation and history stay in loaded project';
		await button('Add layer').click();
		const added = await wait(
			() => inspect(review.root.media),
			(value) => value.items.length === loaded.items.length + 1
		);
		await undo.click();
		await wait(
			() => inspect(review.root.media),
			(value) => isDeepStrictEqual(value, loaded)
		);
		await redo.click();
		await wait(
			() => inspect(review.root.media),
			(value) => isDeepStrictEqual(value, added)
		);
		assert.equal((await playback(review.root.media)).desired_column, null);
		stage = 'loaded project replaced again';
		await newProject();
		assert.deepEqual(await inspectMedia(), { assets: [], media: [] });
		const missingMediaErrors = (
			(await page.locator('body').innerText()).match(/media reference is missing/g) ?? []
		).length;
		assert.equal(
			missingMediaErrors,
			0,
			'project transitions must not dispatch expired media inspections'
		);
		assert.equal(await projectHash(), originalHash, 'upload never modifies the reference project');
		assert.deepEqual(errors, []);
		await page.screenshot({ path: 'target/m07-matrix-project-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual File New/browser upload replaces active Matrix project, clears selection/history, preserves saved IDs with fresh epochs, and retains usable controls; native feedback/pending-outcome integration unverified',
				browser: browser.version(),
				replaced_media: first.media,
				fresh_media: second.media,
				loaded_media: review.root.media,
				previous_launch_id: active.launch_id,
				project_epochs: [firstEpoch, secondEpoch, loadedEpoch],
				review_sha256: originalHash,
				upload_name: uploadName,
				page_errors: errors.length,
				missing_media_errors: missingMediaErrors,
				screenshot: 'target/m07-matrix-project-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-4500)
			})
		);
		await page.screenshot({ path: 'target/m07-matrix-project-ui-failure.png' }).catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
