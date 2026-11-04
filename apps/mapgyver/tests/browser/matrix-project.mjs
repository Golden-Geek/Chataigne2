// Actual File menu replacement and browser upload. Backend requests only observe committed state.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMatrixProjectUi({ base, post, edit, inspectMedia }) {
	const project = path.resolve('target/m07-grid-review/MatrixReview.mapgyver');
	const review = JSON.parse(
		await readFile(path.resolve('target/m07-grid-review/review.json'), 'utf8')
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
	const rejectedRequests = observeRejectedRequests(page, () => stage);
	const drops = [];
	page.on('websocket', (socket) =>
		socket.on('framesent', ({ payload }) => {
			try {
				const frame = JSON.parse(String(payload));
				const edit = frame.intent?.payload?.edit;
				if (edit?.kind === 'DropItem' || edit?.kind === 'DropColumn') drops.push(edit);
			} catch {}
		})
	);
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
		const tab = page.getByRole('tab').filter({ hasText: /^Matrix(?::|$)/ });
		await tab.click();
		const panel = page.locator('section[aria-label="Matrix"]');
		const properties = page.locator('.inspector');
		const actions = panel;
		const button = (name) => actions.getByRole('button', { name, exact: true });
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
			await wait(
				() => grid.count(),
				(value) => value === 0
			);
			await wait(
				() => properties.locator('.parameter-inspector').count(),
				(count) => count === 0
			);
			assert.equal(await grid.getByRole('textbox').count(), 0);
			await wait(() => undo.isDisabled(), Boolean);
			await wait(() => redo.isDisabled(), Boolean);
			await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		}
		async function createSelectedRow() {
			const before = await inspectMedia();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Medias$/ })
				.click();
			const manager = page.getByRole('region', { name: 'Medias', exact: true });
			await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
			await page.getByRole('menuitem', { name: 'Matrix', exact: true }).click();
			const catalog = await wait(inspectMedia, (state) =>
				state.media.some(
					(entry) =>
						entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
				)
			);
			const root = catalog.media.find(
				(entry) => entry.media_kind === 'Matrix' && !before.media.some((old) => old.id === entry.id)
			);
			await manager.getByRole('button', { name: root.label, exact: true }).click();
			await tab.click();
			await wait(
				() => grid.getAttribute('aria-rowcount'),
				(count) => count === '4'
			);
			const snapshot = await inspect(root.id);
			assert.equal(snapshot.items.length, 3);
			assert.equal(snapshot.columns.length, 3);
			assert.equal(snapshot.cells.length, 0);
			await grid.getByTitle(`Select ${snapshot.items[0].label}`, { exact: true }).click();
			return snapshot;
		}
		async function rename(label) {
			const selected = grid.getByRole('button', { pressed: true }).first();
			await selected.dblclick();
			const input = grid.getByRole('textbox');
			await input.fill(label);
			await input.press('Enter');
			await grid.getByTitle(`Select ${label}`, { exact: true }).waitFor({ state: 'attached' });
		}
		stage = 'active Matrix replaced through File New';
		const first = await createSelectedRow();
		await rename('Before replacement');
		await grid.getByRole('button', { name: 'Launch Column 1', exact: true }).click();
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
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Review Matrix', exact: true })
			.click();
		await tab.click();
		await grid.getByTitle('Select Review Group', { exact: true }).click();
		await wait(
			() => grid.getAttribute('aria-rowcount'),
			(value) => value === String(review.rows + 1)
		);
		await properties
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Opacity', { exact: true }) })
			.locator('input.number-field')
			.waitFor();
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
		const feedback = properties.getByRole('region', {
			name: 'Matrix runtime feedback',
			exact: true
		});
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
		const header = (id) => grid.locator(`[role="rowheader"][data-row-id="${id}"]`);
		const source = header(review.layers[0].id);
		const ordinarySource = await source.boundingBox(),
			ordinaryTarget = await header(review.layers[1].id).boundingBox();
		await page.mouse.move(ordinarySource.x + 20, ordinarySource.y + 30);
		await page.mouse.down();
		await page.mouse.move(ordinaryTarget.x + 40, ordinaryTarget.y + ordinaryTarget.height - 4, {
			steps: 5
		});
		await page.mouse.move(ordinaryTarget.x + 40, ordinaryTarget.y + ordinaryTarget.height - 4);
		await page.mouse.up();
		await wait(
			() => inspect(review.root.media),
			(state) => state.items[1].id.id === review.layers[1].id
		);
		assert.equal(drops.length, 1, 'the observer captures a normal dispatched row drop');
		await undo.click();
		await wait(
			() => inspect(review.root.media),
			(state) => isDeepStrictEqual(state, added)
		);
		stage = 'same persisted IDs reload during a held drag';
		const sourceBounds = await source.boundingBox();
		await page.mouse.move(sourceBounds.x + 20, sourceBounds.y + 30);
		await page.mouse.down();
		await page.mouse.move(sourceBounds.x + 60, sourceBounds.y + 50, { steps: 5 });
		const beforeDrops = drops.length;
		await post('/api/ui/project-load', { path: project });
		await wait(() => undo.isDisabled(), Boolean);
		await wait(
			() => grid.getByTitle('Select Upper', { exact: true }).count(),
			(count) => count === 1
		);
		const target = await header(review.layers[1].id).boundingBox();
		await page.mouse.move(target.x + 40, target.y + target.height - 4, { steps: 5 });
		await page.mouse.move(target.x + 40, target.y + target.height - 4);
		await page.mouse.up();
		await page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
		assert.equal(
			drops.length,
			beforeDrops,
			'the old project drag never dispatches into reloaded identical IDs'
		);
		assert.deepEqual(await inspect(review.root.media), loaded);
		assert.notEqual((await window(review.root.media)).project, loadedEpoch);
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
		assert.deepEqual(
			rejectedRequests,
			[],
			'project lifecycle must not reject stale channel-editor inspections'
		);
		assert.equal(await projectHash(), originalHash, 'upload never modifies the reference project');
		assert.deepEqual(errors, []);
		await page.screenshot({ path: 'target/m07-matrix-project-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual File New/browser upload replaces active Matrix project, clears selection/history, preserves saved IDs with fresh epochs, fences a held drag across same-ID reload with a verified drop observer, and retains usable current grid controls; native feedback verified separately',
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
				rejected_requests: rejectedRequests.length,
				screenshot: 'target/m07-matrix-project-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				rejected_requests: rejectedRequests,
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
