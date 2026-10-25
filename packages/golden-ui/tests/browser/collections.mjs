import assert from 'node:assert/strict';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeFileSync, appendFileSync } from 'node:fs';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';

const root = fileURLToPath(new URL('./fixture/', import.meta.url));
const repository = fileURLToPath(new URL('../../../../', import.meta.url));
const generated = resolve(repository, 'target/qualification');
const outDir = resolve(generated, 'p11-collections');
assert.equal(relative(generated, outDir), 'p11-collections');
await build({
	configFile: false,
	root,
	plugins: [svelte({ configFile: false })],
	logLevel: 'warn',
	build: { outDir, emptyOutDir: true }
});
const report = resolve(
	generated,
	`p11-collections-${new Date().toISOString().replaceAll(/[:.]/g, '-')}.jsonl`
);
writeFileSync(report, '');
const emit = (value) => {
	appendFileSync(report, JSON.stringify(value) + '\n');
	const { warmups, samples, ...summary } = value;
	console.log(JSON.stringify(summary));
};
const server = await preview({
	configFile: false,
	root,
	build: { outDir },
	logLevel: 'warn',
	preview: { host: '127.0.0.1', port: 0, strictPort: true }
});
let browser;
try {
	browser = await chromium.launch({
		headless: true,
		...(process.env.GOLDEN_UI_BROWSER_PATH
			? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
			: { channel: process.platform === 'win32' ? 'msedge' : undefined })
	});
	const page = await browser.newPage({ viewport: { width: 1680, height: 1200 } });
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`http://127.0.0.1:${server.httpServer.address().port}`, {
		waitUntil: 'networkidle'
	});
	await page.waitForFunction(() => Boolean(window.collectionFixture));
	emit({ kind: 'environment', browser: await browser.version(), production: true, report });
	const manager = page.locator('[data-panel="manager"]');
	const treeCounts = async () =>
		page.locator('.virtual-rows').evaluateAll((trees) =>
			trees.map((tree) => ({
				total: Number(tree.dataset.rowCount),
				mounted: Number(tree.dataset.mountedRowCount)
			}))
		);
	for (const count of [1000, 10000, 20000, 50000, 100000]) {
		const setup = await page.evaluate((count) => window.collectionFixture.setup(count), count);
		assert((await treeCounts()).every((tree) => tree.mounted > 0 && tree.mounted <= 42));
		assert(
			await manager
				.locator('[data-tall-supplement]')
				.evaluateAll((elements) =>
					elements.every(
						(element) =>
							element.closest('.virtual-row').getBoundingClientRect().height >=
							element.getBoundingClientRect().height
					)
				)
		);
		await page.evaluate((id) => window.collectionFixture.select(id), count + 1);
		assert.equal(
			await manager.locator(`.outliner-item-content[data-node-id="${count + 1}"].selected`).count(),
			1
		);
		await page.evaluate(() => window.collectionFixture.select(2));
		await manager
			.locator('.outliner-item-content[data-node-id="2"] .outliner-item-label')
			.evaluate((element) => element.focus());
		await page.evaluate((id) => window.collectionFixture.select(id), count + 1);
		assert.equal(
			await page.evaluate(() =>
				document.activeElement?.closest('[data-node-id]')?.getAttribute('data-node-id')
			),
			'2'
		);
		await page.evaluate(() => window.collectionFixture.search('does-not-exist'));
		assert.equal(await manager.locator('.outliner-item-content').count(), 0);
		assert.equal(
			await page.evaluate(() => document.activeElement?.classList.contains('manager-list-content')),
			true
		);
		await page.evaluate(() => window.collectionFixture.search(''));
		await page.evaluate(() => window.collectionFixture.expand(2, true));
		await page.evaluate((id) => window.collectionFixture.expand(id, true), count + 6);
		await page.evaluate(() => window.collectionFixture.select(2));
		assert.equal(
			await manager.locator(`.outliner-item-content[data-node-id="${count + 7}"]`).count(),
			1
		);
		await page.evaluate((id) => window.collectionFixture.select(id), count + 1);
		await page.evaluate(() => window.collectionFixture.select(2));
		assert.equal(
			await manager.locator(`.outliner-item-content[data-node-id="${count + 7}"]`).count(),
			1
		);
		await page.evaluate((id) => window.collectionFixture.expand(id, false, true), count + 6);
		assert.equal(
			await manager.locator(`.outliner-item-content[data-node-id="${count + 7}"]`).count(),
			1
		);
		await page.evaluate((id) => window.collectionFixture.openPicker(id), count + 1);
		assert.equal(
			await page
				.locator(`.picker-tree .outliner-item-content[data-node-id="${count + 1}"]`)
				.count(),
			1
		);
		assert((await treeCounts()).every((tree) => tree.mounted <= 42));
		await page.evaluate(() => window.collectionFixture.pickerSearch('Item 000005'));
		assert.equal(
			await page
				.locator('.picker-tree .outliner-item-label')
				.filter({ hasText: 'Item 000005' })
				.count(),
			1
		);
		emit({ kind: 'tier', ...setup, views: await treeCounts(), passed: true });
		for (const name of ['manager-filter', 'selection', 'picker-filter']) {
			const measured = await page.evaluate(
				(name) => window.collectionFixture.measureFlow(name),
				name
			);
			emit({ kind: 'measurement', ...measured });
			assert(
				measured.passed,
				`Collection feedback gate failed: ${JSON.stringify(measured.summary)}`
			);
		}
	}
	await page.evaluate(() => window.collectionFixture.setupVariable(100000));
	await page.waitForSelector('.node-picker-modal', { state: 'detached' });
	await page.evaluate(() => window.collectionFixture.revealVariable(50000));
	const target = page.locator('[data-variable-row="50000"]');
	await target.evaluate((element) => element.focus());
	await page.evaluate(() => window.collectionFixture.revealVariable(90000));
	assert.equal(await target.evaluate((element) => element === document.activeElement), true);
	assert((await treeCounts()).every((tree) => tree.mounted <= 42));
	await page.evaluate(() => window.collectionFixture.filterVariable(80000));
	assert.equal(
		await page.evaluate(() => document.activeElement?.classList.contains('variable-scroller')),
		true
	);
	assert.equal((await page.locator('.virtual-row [data-variable-row]').count()) > 0, true);
	assert.deepEqual(errors, []);
	emit({ kind: 'result', passed: true, variable: await treeCounts(), errors });
} finally {
	await browser?.close();
	await new Promise((resolve, reject) =>
		server.httpServer.close((error) => (error ? reject(error) : resolve()))
	);
}
