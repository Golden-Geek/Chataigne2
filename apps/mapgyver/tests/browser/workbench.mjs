import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyWorkbenchUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const region = (name) => page.getByRole('region', { name, exact: true });
	async function wait(read, predicate) {
		const deadline = Date.now() + 7000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	async function createMenu(name) {
		await region('Media Manager')
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
		await tab('Media Manager').click();
		await createMenu('Folder');
		const mediaTree = region('Media Manager');
		await mediaTree
			.getByRole('treeitem')
			.filter({ hasText: /Folder/ })
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
			.click();
		await page.locator('.properties fieldset').first().waitFor();
		assert.equal(await mediaTree.locator('input:not([type=search]),select,fieldset').count(), 0);
		await page.screenshot({ path: 'target/mapgyver-workbench-media.png' });

		stage = 'screen manager tree and full-panel warping';
		await tab('Screen Manager').click();
		await region('Screen Manager')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await region('Screen Manager')
			.getByRole('treeitem')
			.filter({ hasText: /Screen/ })
			.first()
			.click();
		await page.getByRole('combobox', { name: /^Target/ }).waitFor();
		await region('Screen Manager')
			.getByRole('treeitem')
			.filter({ hasText: /Surface/ })
			.first()
			.click();
		await page.locator('.properties fieldset').first().waitFor();
		await tab('Screen Warping').click();
		await assertCanvas('Screen Warping');
		await page.screenshot({ path: 'target/mapgyver-workbench-screen.png' });
		await tab('Surface Source').click();
		await assertCanvas('Surface Source');

		stage = 'composition canvas and inspector instances';
		await tab('Composition').click();
		await region('Composition')
			.getByRole('button', { name: 'Add Composition', exact: true })
			.click();
		const withComposition = await wait(inspectMedia, (value) =>
			value.media.some((media) => media.media_kind === 'Composition')
		);
		const composition = withComposition.media.find((media) => media.media_kind === 'Composition');
		await tab('Media Manager').click();
		await mediaTree
			.getByRole('treeitem')
			.filter({ hasText: new RegExp(`${composition.channels[0].label}`) })
			.last()
			.click();
		const source = page.getByRole('combobox', { name: 'Source channel', exact: true });
		const choice = await source
			.locator('option')
			.evaluateAll((options) => options.find((option) => option.value)?.value);
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
		const actions = region('Matrix').locator('.editor-toolbar button');
		assert((await actions.count()) >= 5);
		assert((await actions.allTextContents()).every((text) => text.trim() === ''));
		await page.screenshot({ path: 'target/mapgyver-workbench-matrix.png' });

		stage = 'application tree and inspector';
		await tab('Applications').click();
		await region('Applications')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await region('Applications').getByRole('treeitem').first().click();
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
