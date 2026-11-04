import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyWorkbenchUi({ base, post, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const region = (name) => page.locator(`section[aria-label="${name}"]`);
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	async function inspectScreens() {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload: { kind: 'InspectManager' }
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value.snapshot;
	}
	async function rename(label, value) {
		const field = page.getByRole('textbox', { name: label, exact: true });
		await field.fill(value);
		await field.press('Enter');
	}
	async function createMenu(name) {
		await region('Medias')
			.getByRole('button', { name: 'Add child item', exact: true })
			.first()
			.click();
		await page.getByRole('menuitem', { name, exact: true }).click();
	}
	async function assertCanvas(name) {
		const panel = region(name);
		const outer = await panel.boundingBox();
		const canvas = await panel.locator('.editor-canvas').boundingBox();
		assert(
			canvas.width >= outer.width * 0.95 && canvas.height >= outer.height * 0.8,
			`${name} uses its panel`
		);
		assert.equal(
			await panel.locator('input,select,fieldset').count(),
			0,
			`${name} parameters belong in Inspector`
		);
	}
	try {
		await page.goto(base);
		stage = 'media tree folder creation and inspector';
		await tab('Medias').click();
		await createMenu('Folder');
		const mediaTree = region('Medias');
		await mediaTree
			.getByRole('treeitem')
			.filter({ hasText: /Folder/ })
			.getByRole('button', { name: 'Folder', exact: true })
			.click();
		const before = await inspectMedia();
		await createMenu('Solid Color');
		const nested = await wait(
			inspectMedia,
			(value) => value.media.length === before.media.length + 1
		);
		const solid = nested.media.find((media) => !before.media.some((old) => old.id === media.id));
		await mediaTree
			.getByRole('treeitem')
			.filter({ hasText: new RegExp(`${solid.channels[0].label}`) })
			.last()
			.getByRole('button', { name: solid.channels[0].label, exact: true })
			.click();
		await page.locator('.properties fieldset').first().waitFor();
		assert.equal(await mediaTree.locator('input:not([type=search]),select,fieldset').count(), 0);
		await rename('Channel name', 'Nested channel');
		await wait(
			inspectMedia,
			(value) =>
				value.media.find((media) => media.id === solid.id)?.channels[0].label === 'Nested channel'
		);
		await mediaTree.getByRole('button', { name: solid.label, exact: true }).click();
		await rename('Media name', 'Nested solid');
		await wait(inspectMedia, (value) =>
			value.media.some((media) => media.id === solid.id && media.label === 'Nested solid')
		);
		const parentField = page.getByRole('combobox', { name: 'Parent folder', exact: true });
		const folderId = await parentField.inputValue();
		assert(folderId, 'Created media belongs to the selected persisted folder');
		await parentField.selectOption('');
		await wait(
			() => parentField.inputValue(),
			(value) => value === ''
		);
		await parentField.selectOption(folderId);
		await wait(
			() => parentField.inputValue(),
			(value) => value === folderId
		);
		await mediaTree.getByRole('button', { name: 'Folder', exact: true }).click();
		await rename('Folder name', 'Clips');
		await mediaTree.getByRole('button', { name: 'Clips', exact: true }).waitFor();
		await mediaTree.getByRole('button', { name: 'Nested channel', exact: true }).click();
		await page.screenshot({ path: 'target/mapgyver-workbench-media.png' });
		const existing = before.media.find((media) => media.channels.length === 2);
		const reordered = existing.channels[1];
		await mediaTree.getByRole('button', { name: reordered.label, exact: true }).click();
		await page.getByRole('button', { name: 'Move channel earlier', exact: true }).click();
		await wait(
			inspectMedia,
			(value) =>
				value.media.find((media) => media.id === existing.id)?.channels[0].reference.channel ===
				reordered.reference.channel
		);
		await page.getByRole('button', { name: 'Move channel later', exact: true }).click();
		await wait(
			inspectMedia,
			(value) =>
				value.media.find((media) => media.id === existing.id)?.channels[1].reference.channel ===
				reordered.reference.channel
		);

		stage = 'Screens tree and full-panel warping';
		await tab('Screens').click();
		await region('Screens')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await region('Screens')
			.getByRole('treeitem')
			.filter({ hasText: /Screen/ })
			.first()
			.getByRole('button', { name: 'Screen', exact: true })
			.click();
		await page.getByRole('combobox', { name: /^Target/ }).waitFor();
		await rename('Screen name', 'Main output');
		await wait(inspectScreens, (value) => value.screens[0]?.label === 'Main output');
		await region('Screens')
			.getByRole('treeitem')
			.filter({ hasText: /Surface/ })
			.first()
			.getByRole('button')
			.filter({ hasText: /\S/ })
			.first()
			.click();
		await page.locator('.properties fieldset').first().waitFor();
		await tab('Screen Warping').click();
		const managerBounds = await region('Screens').boundingBox();
		const editorBounds = await region('Screen Warping').boundingBox();
		assert(editorBounds.width > managerBounds.width * 1.5, 'Default layout prioritizes the editor');
		await assertCanvas('Screen Warping');
		await page.screenshot({ path: 'target/mapgyver-workbench-screen.png' });
		await tab('Surface Source').click();
		await assertCanvas('Surface Source');
		await rename('Surface name', 'Mapped surface');
		await wait(
			inspectScreens,
			(value) => value.screens[0]?.surfaces[0]?.label === 'Mapped surface'
		);
		await region('Screens')
			.getByRole('button', { name: 'Add Surface', exact: true })
			.click();
		let screenSnapshot = await wait(
			inspectScreens,
			(value) => value.screens[0]?.surfaces.length === 2
		);
		const addedSurface = screenSnapshot.screens[0].surfaces[1].reference.surface;
		await page.getByRole('button', { name: 'Move surface earlier', exact: true }).click();
		await wait(
			inspectScreens,
			(value) => value.screens[0]?.surfaces[0]?.reference.surface === addedSurface
		);
		await page.getByRole('button', { name: 'Delete surface', exact: true }).click();
		await wait(inspectScreens, (value) => value.screens[0]?.surfaces.length === 1);
		await region('Screens')
			.getByRole('button', { name: 'Mapped surface', exact: true })
			.click();

		stage = 'composition canvas and inspector instances';
		await tab('Composition').click();
		await region('Composition')
			.getByRole('button', { name: 'Add Composition', exact: true })
			.click();
		const withComposition = await wait(inspectMedia, (value) =>
			value.media.some((media) => media.media_kind === 'Composition')
		);
		const composition = withComposition.media.find((media) => media.media_kind === 'Composition');
		await tab('Medias').click();
		await mediaTree
			.getByRole('treeitem')
			.filter({ hasText: new RegExp(`${composition.channels[0].label}`) })
			.last()
			.getByRole('button', { name: composition.channels[0].label, exact: true })
			.click();
		const source = page.getByRole('combobox', { name: 'Source channel', exact: true });
		await source.waitFor();
		const choice = await wait(
			() =>
				source
					.locator('option')
					.evaluateAll((options) => options.find((option) => option.value)?.value),
			Boolean
		);
		await source.selectOption(choice);
		await page.getByRole('button', { name: 'Add instance', exact: true }).click();
		await wait(
			() =>
				edit({
					kind: 'Composition',
					reference: composition.channels[0].reference,
					edit: { kind: 'Inspect' }
				}),
			(value) => value.snapshot?.instances.length === 1
		);
		await tab('Composition').click();
		await assertCanvas('Composition');
		await wait(
			() => page.locator('.properties .instance-row').count(),
			(count) => count === 1
		);
		await page.locator('.properties .instance-row .name').click();
		await region('Composition').locator('.selected-pivot').waitFor();
		const compositionCanvas = region('Composition').locator('.editor-canvas');
		const preview = compositionCanvas.locator('.surface');
		const canvasBounds = await compositionCanvas.boundingBox();
		const previewBounds = await preview.boundingBox();
		assert(
			previewBounds.width >= canvasBounds.width * 0.95,
			'Composition image fills the available canvas width'
		);
		await page.screenshot({ path: 'target/mapgyver-workbench-composition.png' });

		stage = 'matrix grid icon actions and inspector selection';
		await tab('Matrix').click();
		await region('Matrix').getByRole('button', { name: 'Add Matrix', exact: true }).click();
		await region('Matrix').getByRole('button', { name: 'Add layer', exact: true }).click();
		await region('Matrix').getByRole('button', { name: 'Add column', exact: true }).click();
		const matrixCatalog = await wait(inspectMedia, (value) =>
			value.media.some((media) => media.media_kind === 'Matrix')
		);
		const matrix = matrixCatalog.media.find((media) => media.media_kind === 'Matrix');
		await region('Matrix').getByRole('gridcell').first().click();
		await page.getByRole('combobox', { name: /^Source to assign/ }).waitFor();
		assert.equal(
			(await edit({ kind: 'Matrix', media: matrix.id, edit: { kind: 'InspectPlayback' } })).snapshot
				.desired_column,
			null
		);
		await assertCanvas('Matrix');
		const gridBounds = await region('Matrix').getByRole('grid').boundingBox();
		const gridCanvasBounds = await region('Matrix').locator('.editor-canvas').boundingBox();
		assert(
			gridBounds.height >= gridCanvasBounds.height * 0.95,
			'Actual Matrix viewport fills its panel'
		);
		const matrixSource = page.getByRole('combobox', { name: /^Source to assign/ });
		const sourceKey = JSON.stringify([solid.id, solid.channels[0].reference.channel]);
		await wait(
			() =>
				matrixSource
					.locator('option')
					.evaluateAll((options) => options.map((option) => option.value)),
			(values) => values.includes(sourceKey)
		);
		await matrixSource.selectOption(sourceKey);
		await page.getByRole('button', { name: 'Assign source', exact: true }).click();
		const assigned = await wait(
			() => edit({ kind: 'Matrix', media: matrix.id, edit: { kind: 'Inspect' } }),
			(value) => value.snapshot?.cells.length === 1
		);
		assert.deepEqual(assigned.snapshot.cells[0].definition.source, solid.channels[0].reference);
		await wait(
			() => region('Matrix').getByRole('gridcell').first().textContent(),
			(label) => label?.includes('Nested')
		);
		const actions = region('Matrix').locator('.editor-toolbar button');
		assert((await actions.count()) >= 5);
		assert((await actions.allTextContents()).every((text) => text.trim() === ''));
		await page.screenshot({ path: 'target/mapgyver-workbench-matrix.png' });

		stage = 'application tree and inspector';
		await tab('Applications').click();
		await region('Applications')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await region('Applications')
			.getByRole('treeitem')
			.first()
			.getByRole('button')
			.filter({ hasText: /\S/ })
			.first()
			.click();
		await page.getByLabel('Executable path', { exact: true }).waitFor();
		assert.equal(
			await region('Applications').locator('fieldset,input:not([type=search])').count(),
			0
		);

		stage = 'project replacement disposes editor selections';
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('New', { exact: true }) })
			.click();
		await wait(inspectMedia, (value) => value.media.length === 0);
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		assert.deepEqual(rejected, []);
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'Golden manager trees, nested media creation, shared Inspector selection, full-panel editors, Matrix icon grid, Applications Inspector, project New',
				browser: browser.version(),
				page_errors: errors.length,
				rejected_requests: rejected.length
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				page_errors: errors,
				rejected_requests: rejected,
				body: (await page.locator('body').innerText()).slice(-6500)
			})
		);
		await page.screenshot({ path: 'target/mapgyver-workbench-failure.png' });
		throw error;
	} finally {
		await browser.close();
	}
}
