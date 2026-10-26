import assert from 'node:assert/strict';
import { resolve, relative } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeFileSync, appendFileSync } from 'node:fs';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';

const root = fileURLToPath(new URL('./fixture/', import.meta.url));
const repository = fileURLToPath(new URL('../../../../../', import.meta.url));
const generated = resolve(repository, 'target/qualification');
const outDir = resolve(generated, 'p11-app-trees');
assert.equal(relative(generated, outDir), 'p11-app-trees');
await build({
	configFile: false,
	root,
	plugins: [svelte({ configFile: false })],
	logLevel: 'warn',
	resolve: { alias: { '#lib': fileURLToPath(new URL('../../src/lib', import.meta.url)) } },
	build: { outDir, emptyOutDir: true }
});
const report = resolve(
	generated,
	`p11-app-trees-${new Date().toISOString().replaceAll(/[:.]/g, '-')}.jsonl`
);
writeFileSync(report, '');
const emit = (value) => {
	appendFileSync(report, JSON.stringify(value) + '\n');
	const { samples, warmups, ...summary } = value;
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
const errors = [];
try {
	browser = await chromium.launch({
		headless: true,
		...(process.env.GOLDEN_UI_BROWSER_PATH
			? { executablePath: process.env.GOLDEN_UI_BROWSER_PATH }
			: { channel: process.platform === 'win32' ? 'msedge' : undefined })
	});
	const page = await browser.newPage({ viewport: { width: 1680, height: 1200 } });
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`http://127.0.0.1:${server.httpServer.address().port}`, {
		waitUntil: 'networkidle'
	});
	await page.waitForFunction(() => Boolean(window.appTrees));
	emit({ kind: 'environment', browser: await browser.version(), production: true, report });
	if (process.argv.includes('--profile-formulas')) {
		await page.evaluate(() => window.appTrees.setup('formulas', 50000));
		const cdp = await page.context().newCDPSession(page);
		await cdp.send('Profiler.enable');
		await cdp.send('Profiler.start');
		const timing = await page.evaluate(() => window.appTrees.measure('focus'));
		const { profile } = await cdp.send('Profiler.stop');
		writeFileSync(resolve(generated, 'p11-formula-tree.cpuprofile'), JSON.stringify(profile));
		await cdp.detach();
		emit({ kind: 'diagnostic-profile', ...timing });
	} else {
		const kinds = process.argv.includes('--properties')
			? ['properties']
			: ['modules', 'formulas', 'processors', 'properties'];
		for (const kind of kinds)
			for (const size of [1000, 10000, 20000, 50000, 100000]) {
				emit({
					kind: 'setup',
					fixture: await page.evaluate(
						([kind, size]) => window.appTrees.setup(kind, size),
						[kind, size]
					)
				});
				assert.equal(await page.locator('.manager-list-content').count(), 1);
				await page.evaluate(() => window.appTrees.focusNode(2));
				await page.evaluate(() => window.appTrees.setExpanded(true));
				assert.equal(await page.locator(`[data-tree-node-id="${size + 3}"]`).count(), 1);
				assert.equal(
					await page.locator(`[data-tree-node-id="${size + 4}"]`).count(),
					0,
					'Leaf-owned implementation must not leak into the manager tree'
				);
				await page.evaluate(() => window.appTrees.setExpanded(false));
				await page.evaluate(() => window.appTrees.navigate('End'));
				assert.equal(
					await page.evaluate(() =>
						Number(document.activeElement?.getAttribute('data-tree-node-id'))
					),
					size + 1
				);
				assert(
					(await page.locator(`[data-tree-node-id="${size + 1}"]`).textContent()).includes(
						`Item ${String(size - 1).padStart(6, '0')}`
					)
				);
				await page.evaluate(() => window.appTrees.navigate('Enter'));
				assert.deepEqual(await page.evaluate(() => window.appTrees.selection()), [size + 1]);
				await page.evaluate(() => window.appTrees.navigate('Home'));
				const counts = await page
					.locator('.manager-list-content .virtual-rows')
					.evaluateAll((nodes) => nodes.map((n) => Number(n.dataset.mountedRowCount)));
				assert(
					counts.every((n) => n > 0 && n <= 42),
					JSON.stringify({
						counts,
						geometry: await page.locator('.manager-list-content').evaluate((element) => ({
							height: element.clientHeight,
							scroll: element.scrollHeight,
							font: getComputedStyle(element).fontSize,
							parents: [
								...(function* () {
									let parent = element;
									for (let n = 0; n < 5 && parent; n++, parent = parent.parentElement)
										yield { name: parent.className, height: parent.clientHeight };
								})()
							]
						}))
					})
				);
				emit({ kind: 'app-tree-correctness', consumer: kind, size, passed: true, counts });
				for (const action of ['focus', 'expand']) {
					const timing = await page.evaluate((action) => window.appTrees.measure(action), action);
					emit({ kind: 'app-tree-timing', ...timing });
					assert(timing.passed, `${timing.name}/${size}: ${JSON.stringify(timing.summary)}`);
				}
				if (kind === 'formulas') {
					await page.evaluate(() => window.appTrees.focusNode(3));
					assert.equal(await page.locator('[data-tree-node-id="3"]').count(), 1);
					const before = await page
						.locator('.manager-list-content .virtual-rows')
						.getAttribute('data-row-count');
					await page.evaluate(() => window.appTrees.filterBuiltin());
					assert.equal(await page.locator('[data-tree-node-id="3"]').count(), 0);
					assert.equal(
						Number(
							await page
								.locator('.manager-list-content .virtual-rows')
								.getAttribute('data-row-count')
						),
						Number(before) - 1
					);
					assert.notEqual(
						await page.evaluate(() =>
							Number(document.activeElement?.getAttribute('data-tree-node-id') ?? -1)
						),
						-1
					);
					await page.evaluate(() => window.appTrees.filterBuiltin());
				}
				assert.deepEqual(errors, []);
			}
	}
	if (!process.argv.includes('--profile-formulas')) emit({ kind: 'result', passed: true, errors });
} catch (error) {
	emit({ kind: 'failure', message: String(error), errors });
	throw error;
} finally {
	await browser?.close();
	await new Promise((done, reject) =>
		server.httpServer.close((error) => (error ? reject(error) : done()))
	);
}
