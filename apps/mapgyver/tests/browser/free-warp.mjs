// Real bundled handle input; backend requests only observe the resulting document.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');
const near = (a, b) => Math.abs(a.x - b.x) < 0.005 && Math.abs(a.y - b.y) < 0.005;

export async function verifyFreeWarpUi({ base, post }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
	page.setDefaultTimeout(5000);
	let stage = 'startup';
	const errors = [];
	const rejected = observeRejectedRequests(page, () => stage);
	page.on('pageerror', (error) => errors.push(String(error)));
	async function screens() {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload: { kind: 'InspectManager' }
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value.snapshot;
	}
	async function wait(read, predicate) {
		const deadline = Date.now() + 5000;
		for (;;) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		const manager = page.locator('section[aria-label="Screens"]');
		await manager.getByRole('button', { name: 'Add child item', exact: true }).click();
		const original = await wait(screens, (value) => value.screens.length === 1);
		const reference = original.screens[0].surfaces[0].reference;
		await manager
			.getByRole('button', { name: original.screens[0].surfaces[0].label, exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping(?::|$)/ })
			.click();
		const editor = page.locator('section[aria-label="Screen Warping"]');
		const svg = editor.getByRole('group', { name: 'Surface mapping controls', exact: true });
		const shape = svg.locator(`[data-surface-drop-target="${reference.surface}"]`);
		const ready = () =>
			wait(
				() => editor.getAttribute('aria-busy'),
				(value) => value === 'false'
			);
		const rotated = [
			{ x: 0.85, y: 0.85 },
			{ x: 0.15, y: 0.85 },
			{ x: 0.15, y: 0.15 },
			{ x: 0.85, y: 0.15 }
		];
		let redStyle;
		for (const [index, point] of rotated.entries()) {
			stage = `held corner ${index + 1}`;
			await ready();
			const handle = editor.getByRole('button', { name: `Warp corner ${index + 1}`, exact: true });
			const start = await handle.boundingBox(),
				rect = await svg.boundingBox();
			assert(start && rect);
			await page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
			await page.mouse.down();
			await page.mouse.move(rect.x + point.x * rect.width, rect.y + point.y * rect.height, {
				steps: 8
			});
			const held = await wait(screens, (value) =>
				near(value.screens[0].surfaces[0].definition.quad.corners[index], point)
			);
			assert.equal(held.screens[0].surfaces[0].mapping_valid, index === 3);
			await wait(
				() => shape.getAttribute('class'),
				(value) => value.includes('invalid') === (index !== 3)
			);
			assert.equal(await editor.getByRole('alert').count(), 0);
			if (index !== 3) {
				redStyle = await shape.evaluate((element) => {
					const style = getComputedStyle(element);
					return { stroke: style.stroke, fill: style.fill };
				});
				assert.equal(redStyle.stroke, 'rgb(255, 83, 100)');
				assert.match(redStyle.fill, /^rgba\(165, 29, 44, /);
				const alpha = Number.parseFloat(redStyle.fill.split(',').at(-1));
				assert(alpha > 0 && alpha < 0.15, 'selected invalid shape has a light local overlay');
				if (index === 1) await page.screenshot({ path: 'target/free-warp-ui-invalid.png' });
			}
			await page.mouse.up();
			await ready();
		}
		const final = await screens();
		assert.equal(final.screens[0].surfaces[0].mapping_valid, true);
		assert.deepEqual(
			final.screens[0].surfaces[0].definition.source,
			original.screens[0].surfaces[0].definition.source
		);
		// Each drag is one history step, even though its intermediate samples were invalid.
		for (let index = 3; index >= 0; index--) {
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(screens, (value) =>
				near(
					value.screens[0].surfaces[0].definition.quad.corners[index],
					original.screens[0].surfaces[0].definition.quad.corners[index]
				)
			);
		}
		assert.deepEqual(await screens(), original);
		for (let index = 0; index < 4; index++) {
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(screens, (value) =>
				near(value.screens[0].surfaces[0].definition.quad.corners[index], rotated[index])
			);
		}
		assert.deepEqual(await screens(), final);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({ path: 'target/free-warp-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'held invalid warp input, red overlay, upside-down recovery and history; native pixels not asserted',
				redStyle,
				pageErrors: errors,
				rejected,
				screenshot: 'target/free-warp-ui.png'
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/free-warp-ui-failure.png' }).catch(() => {});
		console.error(JSON.stringify({ stage, errors, rejected }));
		throw error;
	} finally {
		await browser.close();
	}
}
