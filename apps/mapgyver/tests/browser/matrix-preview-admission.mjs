import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
import { captureMatrixCellPixels } from './matrix-cell-previews.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function verifyMatrixPreviewAdmission({ page, edit, post, nativePid }) {
	const directory = path.resolve(
		process.env.MAPGYVER_MATRIX_ADMISSION_DIRECTORY ?? 'target/m118-admission-native'
	);
	await mkdir(directory, { recursive: true });
	await post('/api/ui/project-new', {});
	const colors = [
		{ name: 'Red', linear: [1, 0, 0, 1], rgb: [255, 0, 0] },
		{ name: 'Green', linear: [0, 1, 0, 1], rgb: [0, 255, 0] },
		{ name: 'Blue', linear: [0, 0, 1, 1], rgb: [0, 0, 255] },
		{ name: 'Yellow', linear: [1, 1, 0, 1], rgb: [255, 255, 0] },
		{ name: 'Cyan', linear: [0, 1, 1, 1], rgb: [0, 255, 255] },
		{ name: 'Magenta', linear: [1, 0, 1, 1], rgb: [255, 0, 255] },
		{ name: 'White', linear: [1, 1, 1, 1], rgb: [255, 255, 255] },
		{ name: 'Grey', linear: [0.5, 0.5, 0.5, 1], rgb: [188, 188, 188] },
		{ name: 'Black', linear: [0, 0, 0, 1], rgb: [0, 0, 0] }
	];
	const sources = [];
	for (const color of colors) {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: `Admission ${color.name}`
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: color.linear }
		});
		sources.push(reference);
	}
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Nine source Matrix'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: matrix.media,
		size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
	});
	await clearMatrixScene(edit, matrix.media);
	const matrixEdit = (value) => edit({ kind: 'Matrix', media: matrix.media, edit: value });
	const { items } = await matrixEdit({ kind: 'AddLayers', parent: null, count: 3 });
	const { columns } = await matrixEdit({ kind: 'AddColumns', count: 32 });
	await matrixEdit({
		kind: 'SetCells',
		assignments: items.flatMap((item, row) =>
			columns.slice(0, 4).map((column, col) => ({
				layer: item.id,
				column,
				source: sources[row * 3 + (col === 3 ? 0 : col)]
			}))
		)
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$|^Media Manager$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Nine source Matrix', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix$/ })
		.click();
	const panel = page.locator('[data-panel-id="matrix"]');
	const surface = panel.locator('.cell-previews');
	const views = surface.locator('.native-preview');
	const sourceCount = async () =>
		Number(await surface.getAttribute('data-matrix-preview-source-count'));
	const cell = (row, col) =>
		panel.locator(`.row[data-row-id="${items[row].id}"]`).getByRole('gridcell').nth(col);
	async function installed() {
		const deadline = Date.now() + 8000;
		while (true) {
			if (
				(await sourceCount()) === 9 &&
				(await views.count()) === 1 &&
				(await views.evaluateAll((views) =>
					views.every(
						(view) =>
							view.dataset.previewState === 'available' &&
							view.dataset.previewInstalled === 'true' &&
							!view.querySelector('.tile-unavailable')
					)
				))
			)
				return;
			assert(
				Date.now() < deadline,
				'nine logical preview sources must install within retained native budgets'
			);
			await delay(50);
		}
	}
	async function pixels(phase) {
		const cells = [];
		for (let row = 0; row < 3; row++)
			for (let col = 0; col < 4; col++)
				cells.push({
					box: await cell(row, col).boundingBox(),
					rgb: colors[row * 3 + (col === 3 ? 0 : col)].rgb
				});
		return captureMatrixCellPixels({ directory, phase, page, nativePid, cells });
	}
	await installed();
	const view = (
		await matrixEdit({
			kind: 'InspectViewport',
			viewport: { row_start: 0, row_count: 3, column_start: 0, column_count: 4 },
			selection: null
		})
	).snapshot;
	const actualSources = [
		...new Map(
			view.cells.map((cell) => [JSON.stringify(cell.source_preview), cell.source_preview])
		).values()
	];
	const key = (reference) => `${reference.media}:${reference.channel}`;
	assert.deepEqual(
		actualSources.map(key).sort(),
		sources.map(key).sort(),
		'transparent Cell aliases retain the nine exact source identities'
	);
	const observations = [await pixels('nine-sources')];
	assert.equal((await matrixEdit({ kind: 'InspectPlayback' })).snapshot.desired_column, null);
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	try {
		await output.demand(matrix);
		await matrixEdit({ kind: 'LaunchColumn', column: columns[0], mode: 'Configured' });
		await installed();
		await output.observePixels('matrix-final', [255, 255, 255, 255]);
		observations.push(await pixels('with-final-output'));
		await matrixEdit({ kind: 'ClearPlayback' });
		await installed();
		observations.push(await pixels('after-unload'));
		await post('/api/ui/project-new', {});
	} finally {
		await output.finish();
	}
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', observations, output: output.observations }, null, 2)
	);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Nine logical procedural previews, twelve Cells sharing source bindings, independent launched Matrix Final pixels and unload'
		})
	);
}
