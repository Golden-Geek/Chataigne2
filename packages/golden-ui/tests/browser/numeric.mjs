import assert from 'node:assert/strict';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { writeFile } from 'node:fs/promises';
import { build, preview } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { chromium } from 'playwright-core';
const root = fileURLToPath(new URL('./fixture/', import.meta.url));
const outDir = fileURLToPath(
	new URL('../../../../target/qualification/m1312-numeric', import.meta.url)
);
await build({
	configFile: false,
	root,
	plugins: [svelte({ configFile: false })],
	logLevel: 'warn',
	build: { outDir, emptyOutDir: true, rollupOptions: { input: resolve(root, 'numeric.html') } }
});
const server = await preview({
	configFile: false,
	root,
	build: { outDir },
	logLevel: 'warn',
	preview: { host: '127.0.0.1', port: 0, strictPort: true }
});
let browser;
const errors = [],
	checks = [];
try {
	browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1000, height: 1100 } });
	page.on('pageerror', (error) => errors.push(error.message));
	await page.goto(`http://127.0.0.1:${server.httpServer.address().port}/numeric.html`);
	await page.waitForFunction(() => Boolean(window.numericFixture));
	const state = () => page.evaluate(() => window.numericFixture.inspect());
	const reset = async () => {
		await page.evaluate(() => window.numericFixture.setup());
		await page.waitForTimeout(30);
	};
	const region = (name) => page.locator(`[data-numeric="${name}"]`);
	async function drag(name, dx, options = {}) {
		const slider = region(name).getByRole('slider').first();
		const bounds = await slider.boundingBox();
		assert(bounds);
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
		await page.mouse.down();
		await page.mouse.move(bounds.x + bounds.width / 2 + dx, bounds.y + bounds.height / 2, {
			steps: 8
		});
		if (options.cancel) await page.keyboard.press('Escape');
		await page.mouse.up();
		await page.waitForFunction(() => !window.numericFixture.inspect().active);
	}
	await drag('Milliseconds', 12);
	let result = await state();
	assert.equal(result.values[0].value, 3.25);
	assert.equal(result.records.filter((intent) => intent.kind === 'beginEdit').length, 1);
	assert.equal(result.records.filter((intent) => intent.kind === 'endEdit').length, 1);
	assert.equal(await region('Milliseconds').locator('input').inputValue(), '00:00:00.00325');
	checks.push('relative fractional milliseconds; one grouped gesture');
	await drag('Milliseconds', -100);
	assert.equal((await state()).values[0].value, 0);
	await drag('Milliseconds', 100);
	assert.equal((await state()).values[0].value, 10);
	checks.push('zero and declared upper bound');
	await reset();
	await drag('Unbounded', 12);
	assert.equal((await state()).values[3].value, 6.5);
	await drag('Unbounded', 12, { cancel: true });
	assert.equal((await state()).values[3].value, 6.5);
	checks.push('unbounded stepped drag; Escape restoration');
	await reset();
	await page.keyboard.down('Alt');
	await drag('Milliseconds', 20);
	await page.keyboard.up('Alt');
	assert.equal((await state()).values[0].value, 0.75);
	await reset();
	await page.keyboard.down('Shift');
	await drag('Milliseconds', 1);
	await page.keyboard.up('Shift');
	assert.equal((await state()).values[0].value, 2.75);
	checks.push('fine/coarse pointer modifiers retain the declared grid');
	await reset();
	for (const cancellation of ['pointercancel', 'lostpointercapture']) {
		const target = region('Milliseconds').getByRole('slider');
		await target.evaluate((element) =>
			element.addEventListener(
				'pointerdown',
				(event) => {
					element.dataset.testPointer = String(event.pointerId);
				},
				{ once: true }
			)
		);
		const bounds = await target.boundingBox();
		await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
		await page.mouse.down();
		await page.mouse.move(bounds.x + bounds.width / 2 + 10, bounds.y + bounds.height / 2);
		const pointerId = Number(await target.getAttribute('data-test-pointer'));
		if (cancellation === 'pointercancel')
			await target.dispatchEvent('pointercancel', { pointerId });
		else
			await target.evaluate(
				(element, pointerId) => element.releasePointerCapture(pointerId),
				pointerId
			);
		await page.mouse.up();
		assert.equal((await state()).values[0].value, 0.25);
		assert.equal((await state()).active, false);
	}
	checks.push('pointer cancellation and capture loss restore the initial value');
	await region('Seconds').getByRole('slider').focus();
	await page.keyboard.press('ArrowRight');
	assert.equal((await state()).values[1].value, 1.2501);
	assert.equal(await region('Seconds').locator('input').inputValue(), '00:00:01.2501');
	await region('Seconds').locator('input').fill('00:00:02.3456');
	await region('Seconds').locator('input').press('Enter');
	assert.equal((await state()).values[1].value, 2.3456);
	checks.push('seconds precision and keyboard/clock entry');
	assert.equal(await region('Free precision').locator('input').inputValue(), '00:00:01.234567');
	await region('Free precision').locator('input').focus();
	await region('Free precision').locator('input').press('Enter');
	assert.equal((await state()).values[8].value, 1.234567);
	await drag('Free precision', 20);
	assert(Math.abs((await state()).values[8].value - 1.254567) < 1e-10);
	checks.push('unstepped fractional time survives focus/blur and relative drag');
	await drag('Bounded', 100);
	const bounded = (await state()).values[2].value;
	assert(bounded > 2 && bounded <= 10 && bounded % 0.5 === 0);
	await region('Bounded').getByRole('slider').press('End');
	assert.equal((await state()).values[2].value, 10);
	await region('Bounded').getByRole('slider').press('Home');
	assert.equal((await state()).values[2].value, 0);
	checks.push('bounded time slider and keyboard endpoints');
	assert(await region('Read only').getByRole('slider').isDisabled());
	assert((await region('Read only').locator('input').getAttribute('readonly')) !== null);
	assert(await region('Disabled').getByRole('slider').isDisabled());
	checks.push('disabled and read-only admission');
	await reset();
	await drag('Integer', 12);
	result = await state();
	assert.equal(result.values[6].value, 12);
	await drag('Vector', 20);
	result = await state();
	assert(result.records.some((intent) => intent.kind === 'setParamComponent'));
	assert.equal(result.records.filter((intent) => intent.kind === 'beginEdit').length, 2);
	checks.push('ordinary integer and zero-start vector regression');
	await reset();
	await drag('Dashboard', 20);
	assert((await state()).values[1].value > 1.25);
	checks.push('actual shared Dashboard consumer');
	await page.evaluate(() => window.numericFixture.holdEvents());
	await drag('Seconds', 20);
	result = await state();
	assert(result.pending > 0);
	const draft = await region('Seconds').locator('input').inputValue();
	await page.evaluate(() => window.numericFixture.releaseEvents());
	await page.waitForTimeout(30);
	assert.equal(await region('Seconds').locator('input').inputValue(), draft);
	checks.push('held graph publication retains final local projection');
	const bounds = await region('Unbounded').getByRole('slider').boundingBox();
	await page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
	await page.mouse.down();
	await page.evaluate(() => window.numericFixture.unmount());
	await page.mouse.up();
	await page.waitForFunction(() => !window.numericFixture.inspect().active);
	assert.deepEqual(errors, []);
	checks.push('unmount closes gesture without global listeners');
	await writeFile(resolve(outDir, 'result.json'), JSON.stringify({ checks, errors }, null, 2));
	console.log(JSON.stringify({ result: 'pass', checks, errors }));
} catch (error) {
	console.error(
		JSON.stringify({
			errors,
			checks,
			body: await browser?.contexts()[0]?.pages()[0]?.locator('body').innerText()
		})
	);
	throw error;
} finally {
	await browser?.close();
	await server.httpServer.close();
}
