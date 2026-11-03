import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyMediaPreviewUi({ base, edit, page: nativePage = null }) {
	const { reference: first } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Preview A'
	});
	const { reference: second } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Preview B'
	});
	await edit({ kind: 'CreateMedia', media_kind: 'Matrix', label: 'Preview Matrix' });
	for (let index = 1; index < 6; index++)
		await edit({ kind: 'AddChannel', media: first.media, label: `Preview variant ${index}` });
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Media Preview panels');
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const manager = page.locator('section[aria-label="Media Manager"]');
	async function select(label) {
		await tab('Media Manager').click();
		await manager.getByRole('button', { name: label, exact: true }).click();
	}
	async function installed(count) {
		if (!nativePage) return;
		await page.waitForFunction((count) => {
			const views = [...document.querySelectorAll('.media-preview-panel .native-preview')];
			return (
				views.length === count &&
				views.every(
					(view) =>
						view.dataset.previewState === 'available' && view.dataset.previewInstalled === 'true'
				) &&
				!document.querySelector('[data-native-preview-retirement]')
			);
		}, count);
	}
	try {
		if (!nativePage) await page.goto(base);
		await select('Preview A');
		await tab('Media Preview').click();
		const firstPanel = page.locator('[data-panel-id="media-preview"]');
		await firstPanel.locator(`[data-preview-state]`).waitFor();
		await page.waitForFunction(
			(id) =>
				document
					.querySelector('[data-panel-id="media-preview"]')
					?.getAttribute('data-preview-media') === id,
			first.media
		);
		await installed(1);
		await firstPanel.getByRole('button', { name: 'Lock media', exact: true }).click();
		await firstPanel.getByRole('button', { name: 'All-channel grid', exact: true }).click();
		await firstPanel.locator('figure').nth(3).waitFor();
		assert.equal(await firstPanel.locator('figure').count(), 4);
		await installed(4);
		await firstPanel.getByRole('button', { name: 'Next preview channels', exact: true }).click();
		await firstPanel.getByText('Preview variant 5', { exact: true }).waitFor();
		assert.equal(await firstPanel.locator('figure').count(), 2);
		await installed(2);
		await select('Preview B');
		await tab('Media Preview').click();
		assert.equal(await firstPanel.getAttribute('data-preview-media'), first.media);
		const group = firstPanel.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		const panels = page.locator('.media-preview-panel');
		assert.equal(await tab('Media Preview').count(), 2);
		const secondId = await panels.evaluateAll(
			(elements) =>
				elements.find((element) => element.dataset.panelId !== 'media-preview').dataset.panelId
		);
		const independent = page.locator(`[data-panel-id="${secondId}"]`);
		await page.waitForFunction(
			({ id, media }) =>
				document.querySelector(`[data-panel-id="${id}"]`)?.getAttribute('data-preview-media') ===
				media,
			{ id: secondId, media: second.media }
		);
		assert.equal(await independent.getAttribute('data-preview-locked'), 'false');
		assert.equal(await independent.locator('figure').count(), 0);
		await installed(1);
		await page.waitForFunction(
			(id) =>
				Object.entries(localStorage).some(
					([key, value]) => key.endsWith('layout.v1') && value.includes(id)
				),
			secondId
		);
		await page.reload();
		await tab('Media Preview').first().click();
		await firstPanel.getByRole('button', { name: 'Unlock media', exact: true }).waitFor();
		await firstPanel.getByText('Preview variant 5', { exact: true }).waitFor();
		assert.equal(await firstPanel.locator('figure').count(), 2);
		await tab('Media Preview').nth(1).click();
		await independent.waitFor();
		assert.equal(await independent.getAttribute('data-preview-locked'), 'false');
		await tab('Media Preview')
			.nth(1)
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		assert.equal(await tab('Media Preview').count(), 1);
		await tab('Media Preview').first().click();
		await installed(2);
		await firstPanel.getByRole('button', { name: 'Unlock media', exact: true }).click();
		await page.waitForFunction(
			(id) =>
				document
					.querySelector('[data-panel-id="media-preview"]')
					?.getAttribute('data-preview-media') === id,
			second.media
		);
		await installed(1);
		await firstPanel.getByRole('button', { name: 'Lock media', exact: true }).click();
		await select('Preview Matrix');
		await page
			.getByRole('region', { name: 'Channels', exact: true })
			.locator('.node-header > .node-title')
			.first()
			.press('Enter');
		await page.locator('.properties').getByText('Runtime', { exact: true }).waitFor();
		assert.equal(await page.locator('.properties .native-preview').count(), 0);
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('New', { exact: true }) })
			.click();
		await firstPanel
			.getByText('Locked media belongs to another project.', { exact: true })
			.waitFor();
		assert.equal(await firstPanel.locator('.native-preview').count(), 0);
		await installed(0);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				mediaPreview:
					'follow/lock, two independent instances, six-channel paged grid, persisted lock/grid/page, project fencing, no Inspector preview',
				nativeInstallation: nativePage
					? 'single/grid/page, panel switching/close, unlock/relock and New installed; pixel and independent-output continuity unverified'
					: 'unverified',
				pageErrors: []
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				panel: await page.locator('.media-preview-panel').evaluateAll((elements) =>
					elements.map((element) => ({
						text: element.innerText,
						media: element.dataset.previewMedia,
						rect: element.getBoundingClientRect().toJSON()
					}))
				),
				errors
			})
		);
		throw error;
	} finally {
		await browser?.close();
	}
}
