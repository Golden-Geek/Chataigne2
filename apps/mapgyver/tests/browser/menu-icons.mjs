import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';

const { chromium } = createRequire(import.meta.url)('playwright-core');

const panels = {
	Composition: 'composition',
	Dashboard: 'dashboard',
	Inspector: 'inspector',
	Logger: 'logger',
	Matrix: 'matrix',
	'Media Preview': 'mediaPreview',
	Medias: 'mediaManager',
	Outliner: 'outliner',
	'Screen Warping': 'screenWarp',
	Screens: 'screenManager',
	'Surface Source': 'surfaceSource',
	Warnings: 'warnings'
};
const kinds = {
	Reference: 'Reference',
	'Solid Color': 'SolidColor',
	Picture: 'Picture',
	'Shared Texture': 'SharedTexture',
	Composition: 'Composition',
	Matrix: 'Matrix'
};

/** Real registered menus and created objects; never infer expected icons from a second mapping. */
export async function verifyMenuIcons({ page, edit, post, directory, surface = 'native' }) {
	const output = path.resolve(process.env.MAPGYVER_MENU_ICONS_DIRECTORY ?? directory, surface);
	await mkdir(output, { recursive: true });
	const errors = [];
	async function dismissMenus() {
		await page.keyboard.press('Escape');
		// Closing layers remain mounted through the ordinary exit transition.
		await page.waitForFunction(
			() => document.querySelectorAll('.gc-context-menu-layer').length === 0
		);
	}
	page.on('pageerror', (error) => errors.push(String(error)));
	await post('/api/ui/project-new', {});
	const references = new Map();
	for (const [label, media_kind] of Object.entries(kinds)) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind, label: `Icon ${label}` });
		references.set(label, reference);
	}
	const manager = page.locator('[data-panel-id="media-manager"]');
	const mediaTab = page.locator('.gc-dv-tab[data-panel-type="mediaManager"]').first();
	await mediaTab.click();
	const expectedMedia = new Map();
	async function imageSource(locator) {
		await locator.waitFor();
		await page.waitForFunction(
			(element) => element.complete && element.naturalWidth > 0,
			await locator.elementHandle()
		);
		assert.equal(await locator.getAttribute('alt'), '', 'decorative icons do not replace labels');
		return locator.getAttribute('src');
	}
	for (const label of Object.keys(kinds)) {
		const row = manager
			.getByRole('button', { name: `Icon ${label}`, exact: true })
			.locator('xpath=ancestor::*[contains(@class,"outliner-item-content")][1]');
		expectedMedia.set(label, await imageSource(row.locator('img.outliner-item-icon').first()));
	}
	assert.equal(new Set(expectedMedia.values()).size, Object.keys(kinds).length);
	const panelIcons = new Map();
	const panelTrigger = () =>
		page.getByRole('button', { name: 'Add panel in this group', exact: true }).first();
	const item = (label) => page.getByRole('menuitem', { name: label, exact: true });
	await panelTrigger().click();
	for (const [label, type] of Object.entries(panels)) {
		const entry = page.locator(`[role="menuitem"][data-menu-item-id="${type}"]`);
		panelIcons.set(type, await imageSource(entry.locator('img')));
		assert.equal((await entry.textContent()).trim(), label);
	}
	await page.screenshot({ path: path.join(output, 'panels-default.png') });
	await dismissMenus();
	// Select every registered panel via its real menu, then compare the spawned tab identity.
	for (const [label, type] of Object.entries(panels)) {
		const tabs = page.locator(`.gc-dv-tab[data-panel-type="${type}"]`);
		const before = await tabs.evaluateAll((nodes) =>
			nodes.map((node) => node.dataset.contentPanelId)
		);
		await panelTrigger().click();
		await item(label).click();
		await page.waitForFunction(
			({ type, before }) =>
				[...document.querySelectorAll(`.gc-dv-tab[data-panel-type="${type}"]`)].some(
					(node) => !before.includes(node.dataset.contentPanelId)
				),
			{ type, before }
		);
		const added = await tabs.evaluateAll(
			(nodes, before) =>
				nodes.map((node) => node.dataset.contentPanelId).filter((id) => !before.includes(id)),
			before
		);
		assert.equal(added.length, 1);
		assert.match(added[0], /^[a-zA-Z0-9_-]+$/);
		const tab = page.locator(`.gc-dv-tab[data-content-panel-id="${added[0]}"]`);
		assert.equal(await imageSource(tab.locator('img.gc-dv-tab-icon')), panelIcons.get(type));
		await tab.getByRole('button', { name: 'Close panel', exact: true }).click();
		await dismissMenus();
	}
	await mediaTab.click();
	await dismissMenus();
	const mediaAdd = () =>
		manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
	async function checkMediaMenu() {
		for (const label of ['Folder', ...Object.keys(kinds)]) {
			const source = await imageSource(item(label).locator('img'));
			if (label === 'Folder') expectedMedia.set(label, expectedMedia.get(label) ?? source);
			assert.equal(source, expectedMedia.get(label), `${label}: catalog/outliner identity`);
		}
	}
	await mediaAdd();
	await checkMediaMenu();
	await page.screenshot({ path: path.join(output, 'media-add-default.png') });
	await item('Folder').click();
	const folder = manager.getByRole('button', { name: 'Folder', exact: true });
	await folder.waitFor();
	const folderRow = folder.locator(
		'xpath=ancestor::*[contains(@class,"outliner-item-content")][1]'
	);
	assert.equal(
		await imageSource(folderRow.locator('img.outliner-item-icon').first()),
		expectedMedia.get('Folder')
	);
	await folder.click({ button: 'right' });
	await page.getByRole('menuitem', { name: /^Add(?:\s|$)/ }).hover();
	await checkMediaMenu();
	await page.screenshot({ path: path.join(output, 'folder-submenu.png') });
	await dismissMenus();
	// Ordinary menu buttons retain their Tab navigation and accessible labels.
	await mediaAdd();
	await item('Folder').focus();
	await page.keyboard.press('Tab');
	assert.equal((await page.locator('[role="menuitem"]:focus').textContent()).trim(), 'Reference');
	assert.equal(
		await imageSource(page.locator('[role="menuitem"]:focus img')),
		expectedMedia.get('Reference')
	);
	await dismissMenus();
	const originalStyle = await page.locator('html').getAttribute('style');
	const variants = [];
	for (const theme of ['default', 'light-tokens']) {
		for (const scale of [1, 1.25, 2]) {
			await page.evaluate(
				({ theme, scale }) => {
					const root = document.documentElement;
					root.style.fontSize = `${16 * scale}px`;
					if (theme === 'light-tokens') {
						root.style.setProperty('--gc-color-panel', '#eef1f5');
						root.style.setProperty('--gc-color-text', '#202733');
					}
				},
				{ theme, scale }
			);
			await panelTrigger().click();
			for (const [label, type] of Object.entries(panels)) {
				const icon = item(label).locator('img');
				await item(label).scrollIntoViewIfNeeded();
				assert.equal(await imageSource(icon), panelIcons.get(type));
				const box = await icon.boundingBox();
				assert(
					box && Math.abs(box.width - 16 * scale) < 0.1 && Math.abs(box.height - 16 * scale) < 0.1
				);
			}
			await page.screenshot({ path: path.join(output, `panels-${theme}-${scale}.png`) });
			await dismissMenus();
			await mediaAdd();
			await checkMediaMenu();
			await page.screenshot({ path: path.join(output, `media-${theme}-${scale}.png`) });
			await dismissMenus();
			variants.push({ theme, scale });
		}
	}
	await page.evaluate((style) => {
		if (style === null) document.documentElement.removeAttribute('style');
		else document.documentElement.setAttribute('style', style);
	}, originalStyle);
	await mediaTab.click();
	await manager.getByRole('button', { name: 'Icon Composition', exact: true }).click();
	await page.locator('.gc-dv-tab[data-panel-type="composition"]').first().click();
	const composition = page.locator('[data-panel-id="composition"]');
	await page.waitForFunction(
		() =>
			document
				.querySelector('[data-panel-id="composition"]')
				?.getAttribute('data-composition-busy') === 'false'
	);
	const zone = await composition.locator('.active-zone').boundingBox();
	assert(zone);
	await page.mouse.click(zone.x + zone.width * 0.4, zone.y + zone.height * 0.6, {
		button: 'right'
	});
	for (const label of Object.keys(kinds))
		assert.equal(await imageSource(item(label).locator('img')), expectedMedia.get(label));
	await page.screenshot({ path: path.join(output, 'composition-add.png') });
	await dismissMenus();
	assert.deepEqual(errors, []);
	const report = {
		passed: true,
		surface,
		panel_entries: Object.keys(panels).length,
		media_entries: expectedMedia.size,
		composition_entries: Object.keys(kinds).length,
		panel_tab_parity: true,
		media_outliner_parity: true,
		folder_submenu: true,
		keyboard_labels: true,
		variants,
		page_errors: errors,
		scope: 'CSS token themes and relative font scales; not physical OS DPI qualification'
	};
	await writeFile(path.join(output, 'results.json'), JSON.stringify(report, null, 2));
	console.log(JSON.stringify(report));
}

export async function verifyMenuIconsInBothHosts(options) {
	await verifyMenuIcons({ ...options, surface: 'native' });
	const browser = await chromium.launch({ headless: true });
	try {
		const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
		page.setDefaultTimeout(7000);
		await page.goto(options.base);
		await verifyMenuIcons({ ...options, page, surface: 'browser' });
	} finally {
		await browser.close();
	}
}
