import assert from 'node:assert/strict';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { verifyInspectorStability } from './inspector-stability.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function verifyNativeInputStability(options) {
	const { page, edit, post, nativePid } = options;
	const directory = path.resolve(process.env.MAPGYVER_INPUT_STABILITY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'));
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const main = page.locator('.media-preview-panel[data-panel-id="media-preview"]');
	const captures = {};
	async function waitFile(file) {
		const deadline = Date.now() + 180_000;
		while (!(await stat(file).catch(() => null))) {
			assert(Date.now() < deadline, `native input observer did not signal ${file}`);
			await delay(100);
		}
	}
	async function captureStart(phase, nativeViews, uiBoxes) {
		const target = path.join(directory, phase);
		await mkdir(target);
		const boxes = await Promise.all((await nativeViews.all()).map((view) => view.boundingBox()));
		assert(boxes.every(Boolean) && uiBoxes.every(Boolean), 'mounted native/UI witnesses required');
		await writeFile(
			path.join(target, 'ready.json'),
			JSON.stringify({
				pid: nativePid,
				boxes,
				ui_boxes: uiBoxes,
				dpr: await page.evaluate(() => window.devicePixelRatio)
			})
		);
		console.log(
			JSON.stringify({
				action: 'start-owned-native-pixel-observer',
				phase,
				directory: target,
				pid: nativePid
			})
		);
		await waitFile(path.join(target, 'capture-started'));
	}
	async function captureEnd(phase) {
		const target = path.join(directory, phase);
		await writeFile(path.join(target, 'complete'), 'complete');
		await waitFile(path.join(target, 'capture-finished'));
		captures[phase] = JSON.parse(await readFile(path.join(target, 'pixels.json'), 'utf8'));
		assert.equal(captures[phase].result, 'pass', JSON.stringify(captures[phase]));
	}
	async function select(label) {
		await tab('Medias').click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: label, exact: true })
			.click();
	}
	async function settled(count) {
		await page.waitForFunction((count) => {
			const views = [...document.querySelectorAll('.media-preview-panel .native-preview')];
			return (
				views.length === count &&
				views.every(
					(view) =>
						view.dataset.previewState === 'available' &&
						view.dataset.previewInstalled === 'true' &&
						!view.querySelector('.unavailable')
				)
			);
		}, count);
	}
	try {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Stable native red'
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		await select('Stable native red');
		await tab('Media Preview').click();
		await settled(1);
		await main.getByRole('button', { name: 'Lock media', exact: true }).click();
		await output.observe('first-grid', { first: reference });
		const inspector = await verifyInspectorStability({
			...options,
			observe: async (phase, details) => {
				if (phase === 'ready')
					await captureStart('inspector', main.locator('.native-preview'), details.uiBoxes);
				else await captureEnd('inspector');
			}
		});
		await output.observePixels('inspector-complete', [255, 0, 0, 255]);
		// Put a locked native witness in the other dock group while Matrix owns the canvas.
		await select('Stable native red');
		const inspectorGroup = page
			.getByRole('tab')
			.filter({ hasText: /^Inspector(?:$|:)/ })
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await inspectorGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		const peerId = await page
			.locator('.media-preview-panel')
			.evaluateAll(
				(panels) =>
					panels.find((panel) => panel.dataset.panelId !== 'media-preview')?.dataset.panelId
			);
		assert(peerId);
		const witness = page.locator(`.media-preview-panel[data-panel-id="${peerId}"]`);
		await witness.getByRole('button', { name: 'Lock media', exact: true }).click();
		const { reference: matrixReference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Matrix',
			label: 'Native input Matrix'
		});
		const matrix = (operation) =>
			edit({ kind: 'Matrix', media: matrixReference.media, edit: operation });
		const { snapshot } = await matrix({ kind: 'Inspect' });
		for (const column of snapshot.columns.slice(0, 2))
			await matrix({
				kind: 'CreateCellMedia',
				layer: snapshot.items[0].id.id,
				column: column.id,
				media_kind: 'SolidColor'
			});
		await select('Native input Matrix');
		await tab('Matrix').click();
		const grid = page.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true });
		const cells = snapshot.columns
			.slice(0, 2)
			.map((column) =>
				grid.getByTitle(`${snapshot.items[0].label} / ${column.label}`, { exact: true })
			);
		await cells[0].click({ position: { x: 5, y: 5 } });
		await page.waitForFunction((id) => {
			const view = document.querySelector(
				`.media-preview-panel[data-panel-id="${id}"] .native-preview`
			);
			return (
				view?.dataset.previewState === 'available' &&
				view.dataset.previewInstalled === 'true' &&
				!view.querySelector('.unavailable')
			);
		}, peerId);
		await captureStart('matrix', witness.locator('.native-preview'), [
			await witness.locator('.editor-toolbar').boundingBox()
		]);
		for (let index = 0; index < 16; index++) {
			const cell = cells[(index + 1) % 2];
			await cell.click({ position: { x: 5, y: 5 } });
			assert.equal(await cell.getAttribute('aria-selected'), 'true');
			await delay(100);
		}
		await captureEnd('matrix');
		await output.observePixels('matrix-complete', [255, 0, 0, 255]);
		const receiver = await output.finish();
		const report = {
			inspector,
			matrixClicks: 16,
			captures,
			receiver,
			outputPixels: output.observations
		};
		await writeFile(path.join(directory, 'input-stability.json'), JSON.stringify(report, null, 2));
		console.log(JSON.stringify(report));
	} finally {
		await output.finish();
	}
}
