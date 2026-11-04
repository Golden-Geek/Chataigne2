import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

// This verifies actual UI registration/feedback lifetime, not native window pixels.
export async function verifyMatrixSparsePreviews({ page, edit, post }) {
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { rejected } = await observeNativeRejectedRequests(
		page,
		() => 'large sparse Cell feedback'
	);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.waitFor();
	await page.evaluate(async () => {
		const host = window.__TAURI_INTERNALS__;
		window.__matrixPreviewStatuses = { sequence: 0, sources: null };
		await host.invoke('plugin:event|listen', {
			event: 'mapgyver-preview-status',
			target: { kind: 'Any' },
			handler: host.transformCallback((event) => {
				window.__matrixPreviewStatuses = {
					sequence: window.__matrixPreviewStatuses.sequence + 1,
					sources: event.payload
				};
			})
		});
	});
	await post('/api/ui/project-new', {});
	const source = async (label) => {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 32, height: 24 } }
		});
		return reference;
	};
	const red = await source('Sparse red');
	const blue = await source('Sparse blue');
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Sparse preview Matrix'
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	await matrix({ kind: 'AddLayers', parent: null, count: 997 });
	await matrix({ kind: 'AddColumns', count: 997 });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	const initial = await inspect();
	assert.equal(initial.items.length, 1000);
	assert.equal(initial.columns.length, 1000);
	const addresses = [
		[0, 0],
		[0, 1],
		[1, 0],
		[998, 998],
		[998, 999],
		[999, 998]
	];
	await matrix({
		kind: 'SetCells',
		assignments: addresses.map(([row, col], index) => ({
			layer: initial.items[row].id.id,
			column: initial.columns[col].id,
			source: index % 3 === 2 ? blue : red
		}))
	});
	const authored = await inspect();
	assert.equal(authored.cells.length, 6, 'one million addresses retain only six authored Cells');
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Sparse preview Matrix', exact: true })
		.click();
	const tab = page.getByRole('tab').filter({ hasText: /^Matrix(?::|$)/ });
	await tab.click();
	const panel = page.locator('[data-panel-id="matrix"]');
	const grid = panel.getByRole('grid');
	const surface = panel.locator('.cell-previews');
	const preview = surface.locator('.native-preview');
	async function wait(read, test) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	async function installed(sources) {
		await wait(
			() => surface.getAttribute('data-matrix-preview-source-count'),
			(count) => count === String(sources)
		);
		assert.equal(await preview.count(), 1);
		if (sources)
			await wait(
				() =>
					preview.evaluate(
						(view) =>
							view.dataset.previewInstalled === 'true' &&
							view.dataset.previewState === 'available' &&
							!view.querySelector('.tile-unavailable')
					),
				Boolean
			);
		else
			await wait(
				() => preview.getAttribute('data-preview-view'),
				(value) => value === null
			);
		assert((await grid.getByRole('gridcell').count()) <= 2048, 'mounted addresses stay bounded');
	}
	const runtime = () =>
		page.evaluate(async () =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
				})
			)
		);
	const observations = [];
	async function observe(phase) {
		const status = await runtime();
		assert.equal(status.error, null, JSON.stringify(status));
		if (status.resources) {
			assert(status.resources.used_bytes <= status.resources.limit_bytes);
			assert(status.resources.in_flight <= 2 && status.resources.retiring <= 2);
		}
		observations.push({
			phase,
			sourceGroups: await surface.getAttribute('data-matrix-preview-source-count'),
			mountedCells: await grid.getByRole('gridcell').count(),
			state: status.state
		});
	}
	const cell = (row, column = 0) =>
		grid
			.locator(`.row[data-row-id="${initial.items[row].id.id}"]`)
			.locator(`[role="gridcell"][aria-colindex="${column + 2}"]`);
	await installed(2);
	await wait(
		() => cell(0).innerText(),
		(text) => text.includes('Sparse red [Channel 1]')
	);
	await cell(0).click();
	const labelGeometry = await cell(0)
		.locator('.source-label')
		.evaluate((label) => ({
			height: label.clientHeight,
			content: label.scrollHeight,
			maximum: parseFloat(getComputedStyle(label).maxHeight),
			line: parseFloat(getComputedStyle(label).lineHeight),
			title: label.title
		}));
	assert(labelGeometry.maximum >= 2 * labelGeometry.line - 1, JSON.stringify(labelGeometry));
	assert(
		labelGeometry.content <= labelGeometry.height + 1,
		'two-line source label has no clipped glyphs'
	);
	assert.equal(labelGeometry.title, 'Sparse red [Channel 1]');
	assert.equal((await matrix({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
	await observe('near');
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
		element.scrollLeft = element.scrollWidth;
	});
	await grid.getByTitle('Select Layer 1000', { exact: true }).waitFor();
	await installed(2);
	await wait(
		() => cell(998, 998).innerText(),
		(text) => text.includes('Sparse red [Channel 1]')
	);
	await observe('far');
	const lease = await preview.getAttribute('data-preview-view');
	assert.deepEqual(await inspect(), authored, 'viewport movement does not author anything');
	await edit({ kind: 'RemoveMedia', media: red.media });
	await installed(1);
	await wait(
		() => cell(998, 998).innerText(),
		(text) => text.includes('Missing source')
	);
	await observe('source-loss');
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await installed(2);
	await wait(
		() => cell(998, 998).innerText(),
		(text) => text.includes('Sparse red [Channel 1]')
	);
	assert.equal(
		await preview.getAttribute('data-preview-view'),
		lease,
		'loss and Undo preserve the logical grid lease'
	);
	assert.deepEqual(await inspect(), authored);
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight / 2;
		element.scrollLeft = element.scrollWidth / 2;
	});
	await installed(0);
	await observe('empty-viewport');
	await grid.evaluate((element) => {
		element.scrollTop = 0;
		element.scrollLeft = 0;
	});
	await installed(2);
	await observe('scroll-recovery');
	await page
		.getByRole('tab')
		.filter({ hasText: /^Composition(?::|$)/ })
		.click();
	await wait(
		() => panel.locator('.native-preview').count(),
		(count) => count === 0
	);
	await tab.click();
	await installed(2);
	await observe('tab-recovery');
	const beforeNew = await page.evaluate(() => window.__matrixPreviewStatuses.sequence);
	await post('/api/ui/project-new', {});
	await wait(
		() => panel.locator('.native-preview').count(),
		(count) => count === 0
	);
	// The host retains its clearing presentation window after demand disappears. Observe the
	// actual backend empty observation set, not runtime destruction. Physical owner retirement
	// is checked separately by the complete native fixture's balanced allocation assertions.
	const withdrawn = await wait(
		() => page.evaluate(() => window.__matrixPreviewStatuses),
		(value) => value.sequence > beforeNew && value.sources?.length === 0
	);
	assert.equal((await runtime()).error, null);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	console.log(
		JSON.stringify({
			result: 'pass',
			dimensions: [1000, 1000],
			authoredCells: 6,
			observations,
			withdrawn,
			scope:
				'Actual native WebView bounded sparse Cell labels, shared registration, source loss/Undo, scroll and tab lifetime, backend empty observations after New; no native window pixel or complete GPU-owner retirement claim'
		})
	);
}
