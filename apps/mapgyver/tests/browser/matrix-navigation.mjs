import assert from 'node:assert/strict';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { verifyNavigationPixels } from './navigation-pixels.mjs';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

const panel = (page, id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);

async function settledZoom(view) {
	return await view.evaluate(async (element) => {
		const zoom = () => Number(element.querySelector('output[data-zoom]').dataset.zoom);
		await new Promise((resolve) => setTimeout(resolve, 210));
		return zoom();
	});
}

async function resizeDuringFrame(page, view, framedZoom) {
	const before = await view.boundingBox();
	const group = await view
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
		.boundingBox();
	assert(before && group);
	const sashes = await page
		.locator('.dv-sash.dv-enabled')
		.evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().toJSON()));
	const candidates = sashes.filter(
		(item) => item.height > 100 && Math.abs(item.x + item.width / 2 - group.x - group.width) < 12
	);
	assert.equal(candidates.length, 1, 'ordinary dock divider beside the Matrix editor');
	const sash = candidates[0],
		x = sash.x + sash.width / 2,
		y = sash.y + sash.height / 2;
	const readZoom = async () =>
		Number(await view.locator('output[data-zoom]').getAttribute('data-zoom'));
	await page.mouse.move(x, y);
	await view.press('f');
	await page.waitForFunction(
		({ name, framed }) => {
			const camera = Number(
				document.querySelector(`[aria-label="${name}"] output[data-zoom]`)?.dataset.zoom
			);
			return camera > 1 && camera < framed;
		},
		{ name: await view.getAttribute('aria-label'), framed: framedZoom },
		{ timeout: 1000 }
	);
	const inProgress = await readZoom();
	assert(inProgress > 1 && inProgress < framedZoom);
	await page.mouse.down();
	try {
		await page.mouse.move(x - 135, y);
	} finally {
		await page.mouse.up();
	}
	await page.waitForFunction(
		({ name, width }) => {
			const element = document.querySelector(`[aria-label="${name}"]`);
			return element && Math.abs(element.getBoundingClientRect().width - width) > 30;
		},
		{ name: await view.getAttribute('aria-label'), width: before.width }
	);
	const cancelledAt = await readZoom();
	assert(cancelledAt < framedZoom, 'resize must interrupt the active Frame animation');
	assert.equal(await settledZoom(view), cancelledAt, 'resize retains its displayed camera');
	return {
		beforeWidth: before.width,
		afterWidth: (await view.boundingBox()).width,
		inProgress,
		cancelledAt
	};
}

export async function verifyMatrixNavigation({ page, edit, nativePid }) {
	let stage = 'seed sparse Matrix';
	const errors = [],
		{ rejected, observed } = await observeNativeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	const hwnd = Number(ownedWindowHandle(nativePid));
	const { reference: red } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Navigation red'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference: red,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Sparse navigation Matrix'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 640, height: 360 } }
	});
	const matrix = (operation) => edit({ kind: 'Matrix', media: reference.media, edit: operation });
	const inspect = async () => (await matrix({ kind: 'Inspect' })).snapshot;
	await matrix({ kind: 'AddLayers', parent: null, count: 129 });
	await matrix({ kind: 'AddColumns', parent: null, count: 61 });
	const initial = await inspect();
	assert.equal(initial.items.length, 132);
	assert.equal(initial.columns.length, 64);
	const row = initial.items.at(-1),
		layer = row.id.id,
		column = initial.columns[0];
	await matrix({
		kind: 'ConfigureLayer',
		layer,
		settings: { position: [160, 120], size: { width: 200, height: 120 }, duration_ms: 0 }
	});
	await matrix({ kind: 'SetCells', assignments: [{ layer, column: column.id, source: red }] });
	const cellSettings = {
		position: [0, 0],
		size: { width: 200, height: 120 },
		fit_mode: 'Fill',
		opacity: 1,
		enabled: true
	};
	await matrix({ kind: 'ConfigureCell', layer, column: column.id, settings: cellSettings });
	await matrix({ kind: 'LaunchCell', layer, column: column.id, mode: 'Cut' });
	const authored = await inspect();
	assert.equal(authored.cells.length, 1);
	await panel(page, 'media-manager').click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Sparse navigation Matrix', exact: true })
		.click();
	await panel(page, 'matrix').click();
	const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
	await grid.evaluate((element) => {
		element.scrollTop = element.scrollHeight;
	});
	await grid.getByRole('rowheader', { name: row.label, exact: true }).press('Enter');
	assert(
		(await grid.getByRole('gridcell').count()) <= 2048,
		'only the visible sparse Matrix window is mounted'
	);
	await panel(page, 'composition').click();
	const layers = page.getByRole('application', { name: 'Matrix Layer layout view', exact: true });
	const cells = page.getByRole('application', { name: 'Matrix Cell content view', exact: true });
	const installed = (view) =>
		view
			.locator('.native-preview[data-preview-installed="true"][data-native-overlays="true"]')
			.waitFor();
	await installed(layers);
	await layers.locator(`[data-layer-id="${layer}"].selected`).waitFor();
	assert.equal(
		await layers.getByText('Page 1 / 2', { exact: true }).count(),
		1,
		'the selected past-page Layer is framed from its bounded selected record'
	);
	const report = [];
	await layers.focus();
	stage = 'Frame selected Matrix Layer';
	let framedZoom;
	report.push(
		await verifyNavigationPixels({
			page,
			view: layers,
			hwnd,
			label: stage,
			navigate: async () => {
				await layers.press('f');
				framedZoom = await settledZoom(layers);
				assert(framedZoom > 1);
				return { zoom: framedZoom };
			}
		})
	);
	stage = 'Home from toolbar in Matrix Layer layout';
	report.push(
		await verifyNavigationPixels({
			page,
			view: layers,
			hwnd,
			label: stage,
			navigate: async () => {
				await layers.getByRole('button', { name: 'Home view', exact: true }).click();
				assert.equal(await settledZoom(layers), 1);
			}
		})
	);
	stage = 'resize during Matrix Frame';
	report.push(
		await verifyNavigationPixels({
			page,
			view: layers,
			hwnd,
			label: stage,
			interrupted: true,
			navigate: () => resizeDuringFrame(page, layers, framedZoom)
		})
	);
	assert.deepEqual(
		await inspect(),
		authored,
		'camera, selection and dock resize leave the sparse document unchanged'
	);
	await matrix({
		kind: 'ConfigureCell',
		layer,
		column: column.id,
		settings: { ...cellSettings, position: [61, 30], size: { width: 66, height: 60 } }
	});
	await layers.getByRole('button', { name: 'Edit Layer Cells', exact: true }).click();
	const cell = authored.cells[0].id;
	const contour = cells.locator(`[data-layout-cell="${cell}"]`);
	await contour.press('Enter');
	await cells.locator(`[data-layout-cell="${cell}"].selected`).waitFor();
	await installed(cells);
	stage = 'Frame selected Matrix Cell';
	report.push(
		await verifyNavigationPixels({
			page,
			view: cells,
			hwnd,
			label: stage,
			navigate: async () => {
				await cells.getByRole('button', { name: 'Frame selection', exact: true }).click();
				const zoom = await settledZoom(cells);
				assert(zoom > 1);
				return { zoom };
			}
		})
	);
	stage = 'Home by key in Matrix Cell layout';
	report.push(
		await verifyNavigationPixels({
			page,
			view: cells,
			hwnd,
			label: stage,
			navigate: async () => {
				await cells.press('h');
				assert.equal(await settledZoom(cells), 1);
			}
		})
	);
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	assert(observed.receipts > 0, 'observe ordinary native WebView edit receipts');
	console.log(
		JSON.stringify({
			matrixNavigation: report,
			layers: 132,
			columns: 64,
			cells: 1,
			errors,
			observed
		})
	);
}
