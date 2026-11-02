import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyCreationCatalogsUi({ base }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'creation catalogs');
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	const panel = (name) => page.locator(`section[aria-label="${name}"]`);
	const expected = ['Folder', 'Solid Color', 'Picture', 'Shared Texture', 'Composition', 'Matrix'];
	async function mediaMenu() {
		await panel('Media Manager')
			.getByRole('button', { name: 'Add child item', exact: true })
			.first()
			.click();
		await page.getByRole('menuitem', { name: 'Folder', exact: true }).waitFor();
		assert.deepEqual(
			(await page.getByRole('menuitem').allTextContents()).map((label) => label.trim()),
			expected
		);
	}
	try {
		await page.goto(base);
		await tab('Media Manager').click();
		await mediaMenu();
		await page.getByRole('menuitem', { name: 'Folder', exact: true }).click();
		await panel('Media Manager').getByRole('button', { name: 'Folder', exact: true }).click();
		await mediaMenu();
		await page.getByRole('menuitem', { name: 'Solid Color', exact: true }).click();
		await tab('Screen Manager').click();
		await panel('Screen Manager')
			.getByRole('button', { name: 'Add child item', exact: true })
			.click();
		await panel('Screen Manager').getByRole('button', { name: 'Screen', exact: true }).waitFor();
		assert.equal(
			await page
				.getByText(/^(Create Screen|Screen Operation|Screen Script|Media Operation|Media Script)$/)
				.count(),
			0
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope: 'bundled object-only Media/Folder catalogs and direct Screen creation'
			})
		);
	} finally {
		await browser.close();
	}
}
