import assert from 'node:assert/strict';
import { verifyRuntimeFooter } from './runtime-footer.mjs';

export async function verifyWorkbenchIdentity(options) {
	const { page, edit, post } = options;
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const references = new Map();
	await page.locator('.gc-dv-tab[data-panel-type="mediaManager"]').waitFor();
	for (const media_kind of ['Picture', 'SolidColor', 'SharedTexture', 'Composition', 'Matrix']) {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind,
			label: `Identity ${media_kind}`
		});
		references.set(media_kind, reference);
	}
	await page.locator('.gc-dv-tab[data-panel-type="mediaManager"]').click();
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	const icons = [];
	for (const kind of references.keys()) {
		const button = manager.getByRole('button', { name: `Identity ${kind}`, exact: true });
		await button.waitFor();
		const row = button.locator('xpath=ancestor::*[contains(@class,"outliner-item-content")][1]');
		const icon = row.locator('img.outliner-item-icon').first();
		await icon.waitFor();
		assert(await icon.evaluate((image) => image.complete && image.naturalWidth > 0));
		icons.push(await icon.getAttribute('src'));
	}
	assert.equal(new Set(icons).size, references.size, 'each media kind has its supplied icon');
	assert(
		await page
			.locator('img.app-icon')
			.evaluate((image) => image.complete && image.naturalWidth === 128)
	);
	for (const [kind, panel] of [
		['Matrix', 'matrix'],
		['Composition', 'composition']
	]) {
		await page.locator('.gc-dv-tab[data-panel-type="mediaManager"]').click();
		await manager.getByRole('button', { name: `Identity ${kind}`, exact: true }).click();
		const tab = page.locator(`.gc-dv-tab[data-panel-type="${panel}"]`);
		await tab.click();
		await page.waitForFunction(
			({ panel, label }) =>
				document
					.querySelector(`.gc-dv-tab[data-panel-type="${panel}"] .gc-dv-tab-label`)
					?.textContent?.includes(label),
			{ panel, label: `Identity ${kind}` }
		);
		await edit({
			kind: 'RenameMedia',
			media: references.get(kind).media,
			label: `Renamed ${kind}`
		});
		await page.waitForFunction(
			({ panel, label }) =>
				document
					.querySelector(`.gc-dv-tab[data-panel-type="${panel}"] .gc-dv-tab-label`)
					?.textContent?.includes(label),
			{ panel, label: `Renamed ${kind}` }
		);
	}
	await verifyRuntimeFooter(options);
	await page.locator('.gc-dv-tab[data-panel-type="matrix"]').click();
	await page.waitForFunction(
		() =>
			document.querySelector('.gc-dv-tab[data-panel-type="matrix"] .gc-dv-tab-label')
				?.textContent === 'Matrix'
	);
	assert.deepEqual(errors, []);
	console.log(
		JSON.stringify({
			identity:
				'five distinct supplied media icons, app header, Matrix/Composition selection and rename, project title reset, native footer',
			pageErrors: errors
		})
	);
}
