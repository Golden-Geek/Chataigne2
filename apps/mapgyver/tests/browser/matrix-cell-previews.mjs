import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { clearMatrixScene } from './matrix-scene.mjs';
const run = promisify(execFile);

export async function verifyMatrixCellPreviews({ page, edit, post, nativePid }) {
	const directory = path.resolve(
		process.env.MAPGYVER_MATRIX_PREVIEW_DIRECTORY ?? 'target/m118-grid-native'
	);
	await post('/api/ui/project-new', {});
	const source = async (label, rgba) => {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
		await edit({ kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
		return reference;
	};
	const red = await source('Tile red', [1, 0, 0, 1]);
	const blue = await source('Tile blue', [0, 0, 1, 1]);
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Native Cell grid'
	});
	await clearMatrixScene(edit, matrix.media);
	const matrixEdit = (value) => edit({ kind: 'Matrix', media: matrix.media, edit: value });
	const { items } = await matrixEdit({ kind: 'AddLayers', parent: null, count: 2 });
	const { columns } = await matrixEdit({ kind: 'AddColumns', count: 32 });
	await matrixEdit({
		kind: 'SetCells',
		assignments: items.flatMap((item, row) =>
			columns
				.slice(0, 2)
				.map((column) => ({ layer: item.id, column, source: row === 0 ? red : blue }))
		)
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$|^Media Manager$/ })
		.click();
	await page
		.locator('[data-panel-id="media-manager"]')
		.getByRole('button', { name: 'Native Cell grid', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Matrix$/ })
		.click();
	const panel = page.locator('[data-panel-id="matrix"]');
	const viewport = panel.getByRole('grid');
	const groups = panel.locator('[data-matrix-preview-source]');
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	async function wait(read, test) {
		const deadline = Date.now() + 8000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(40);
		}
	}
	async function installed(count) {
		await wait(
			() => groups.count(),
			(value) => value === count
		);
		if (count)
			await wait(
				() =>
					groups
						.locator('.native-preview')
						.evaluateAll((views) =>
							views.every(
								(view) =>
									view.dataset.previewState === 'available' &&
									view.dataset.previewInstalled === 'true'
							)
						),
				Boolean
			);
	}
	const cell = (row, column) =>
		panel.locator(`.row[data-row-id="${items[row].id}"]`).getByRole('gridcell').nth(column);
	async function pixels(phase, top, extra = []) {
		const target = path.join(directory, phase);
		await mkdir(target, { recursive: true });
		const boxes = await Promise.all(
			[
				[0, 0],
				[0, 1],
				[1, 0],
				[1, 1]
			].map(async ([row, column]) => ({
				box: await cell(row, column).boundingBox(),
				rgb: row === 0 ? top : [0, 0, 255]
			}))
		);
		assert(boxes.every((cell) => cell.box));
		boxes.push(...extra);
		await writeFile(
			path.join(target, 'ready.json'),
			JSON.stringify({
				pid: nativePid,
				dpr: await page.evaluate(() => devicePixelRatio),
				cells: boxes
			})
		);
		const result = await run(
			'python',
			['apps/mapgyver/tests/native/matrix_cell_pixels.py', target],
			{ timeout: 10000, windowsHide: true }
		);
		assert.equal(JSON.parse(result.stdout).result, 'pass', result.stdout);
		return JSON.parse(result.stdout);
	}
	await installed(2);
	assert.equal(await groups.count(), 2, 'four Cells share two canonical source leases');
	const observations = [await pixels('initial', [255, 0, 0])];
	await cell(0, 0).click();
	assert.equal(
		(await matrixEdit({ kind: 'InspectPlayback' })).snapshot.desired_column,
		null,
		'thumbnail visibility and selection do not launch playback'
	);
	await edit({
		kind: 'ConfigureChannel',
		reference: red,
		source: { kind: 'SolidColor', rgba: [0, 1, 0, 1] }
	});
	await installed(2);
	await delay(150);
	observations.push(await pixels('live-edit', [0, 255, 0]));
	await edit({ kind: 'RemoveMedia', media: red.media });
	await installed(1);
	await wait(
		() => cell(0, 0).innerText(),
		(text) => text.includes('Missing source')
	);
	observations.push(await pixels('source-loss', null));
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await installed(2);
	observations.push(await pixels('undo-recovery', [0, 255, 0]));
	const authored = (await matrixEdit({ kind: 'Inspect' })).snapshot;
	const alias = authored.cells.find(
		(entry) => entry.layer === items[0].id && entry.definition.column === columns[0]
	).definition.source;
	await edit({
		kind: 'ConfigureMediaResolution',
		media: alias.media,
		size: { kind: 'Fixed', extent: { width: 32, height: 128 } }
	});
	await installed(3);
	observations.push(
		await pixels(
			'reference-fit',
			[0, 255, 0],
			[{ box: await cell(0, 0).boundingBox(), fraction_y: 0.2, rgb: [0, 0, 0] }]
		)
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await installed(2);
	await viewport.evaluate((element) => {
		element.scrollLeft = element.scrollWidth;
	});
	await installed(0);
	await viewport.evaluate((element) => {
		element.scrollLeft = 0;
	});
	await installed(2);
	observations.push(await pixels('scroll-recovery', [0, 255, 0]));
	await post('/api/ui/project-new', {});
	await wait(
		() => groups.count(),
		(value) => value === 0
	);
	assert.deepEqual(errors, []);
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify({ result: 'pass', observations }, null, 2)
	);
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Native Matrix shared-source tiles, actual pixels, live source edit, selection without launch, loss/Undo, scroll lease withdrawal/recovery and New'
		})
	);
}
