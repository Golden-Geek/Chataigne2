// Real workbench menu and independent WebView RGBA layer; never inject a visual overlay.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { chromium } from 'playwright-core';

const directory = path.resolve(process.argv[2]);
const action = process.argv[3];
assert(['open', 'layer', 'clear-layer', 'close'].includes(action));
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
try {
	const pages = browser
		.contexts()
		.flatMap((c) => c.pages())
		.filter((p) => p.url().startsWith(ready.base + '/'));
	assert.equal(pages.length, 1);
	const page = pages[0];
	page.setDefaultTimeout(5000);
	if (action === 'clear-layer') {
		assert.equal(await page.locator('.gc-context-menu-layer').count(), 0);
		await page.mouse.move(30, 30);
		await page.screenshot({
			path: path.join(directory, 'clear-layer.png'),
			omitBackground: true
		});
		console.log(JSON.stringify({ layer: 'clear-layer.png' }));
	} else if (action === 'close') {
		await page.keyboard.press('Escape');
		await page.waitForFunction(
			() => document.querySelectorAll('.gc-context-menu-layer').length === 0
		);
		await writeFile(path.join(directory, 'menu-closed.json'), JSON.stringify({ closed: true }));
		console.log(JSON.stringify({ closed: true }));
	} else {
		if (action === 'open') {
			const source = page.getByRole('tab').filter({ hasText: /^Surface Source:/ });
			const group = source.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
			await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		}
		const menu = page.locator('.gc-context-menu');
		await menu.waitFor({ state: 'visible' });
		assert.equal(await menu.count(), 1);
		await menu.evaluate(async (element) => {
			await Promise.all(
				element
					.getAnimations({ subtree: true })
					.map((animation) => animation.finished.catch(() => {}))
			);
		});
		await page.mouse.move(30, 30);
		const geometry = await menu.evaluate((element) => {
			const style = getComputedStyle(element);
			return {
				dpr: devicePixelRatio,
				rect: JSON.parse(JSON.stringify(element.getBoundingClientRect())),
				background: style.background,
				backdropFilter: style.backdropFilter,
				opacity: style.opacity,
				entries: [...element.querySelectorAll('[role="menuitem"]')].map((item) =>
					item.textContent.trim()
				)
			};
		});
		assert.equal(geometry.entries.length, 12);
		if (action === 'open') {
			await writeFile(path.join(directory, 'menu-ready.json'), JSON.stringify(geometry, null, 2));
			console.log(JSON.stringify(geometry));
		} else {
			await page.screenshot({ path: path.join(directory, 'menu-layer.png'), omitBackground: true });
			await writeFile(
				path.join(directory, 'menu-layer-geometry.json'),
				JSON.stringify(geometry, null, 2)
			);
			console.log(JSON.stringify({ layer: 'menu-layer.png', dpr: geometry.dpr }));
		}
	}
} finally {
	await browser.close();
}
