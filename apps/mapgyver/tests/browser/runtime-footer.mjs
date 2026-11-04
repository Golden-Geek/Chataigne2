import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyRuntimeFooter({ base, post, page: nativePage = null }) {
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1200, height: 850 } }));
	page.setDefaultTimeout(10000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const status = page.locator('footer.gc-footer output[aria-label="Video runtime"]');
	try {
		if (!nativePage) await page.goto(base);
		await status.waitFor();
		await page.locator('footer.gc-footer .engine-rate').waitFor();
		if (!nativePage) {
			assert.equal(await status.getAttribute('data-state'), 'unavailable');
			assert.equal(await status.innerText(), 'Video unavailable');
		} else {
			const edit = async (payload) => {
				const ack = await post('/api/ui/intent', {
					kind: 'invokeAppOperation',
					namespace: 'mapgyver.screens',
					operation: 'edit',
					payload
				});
				assert(ack.success, JSON.stringify(ack));
				return ack.result.value;
			};
			const { screen } = await edit({ kind: 'CreateScreen', label: 'Footer screen' });
			await edit({
				kind: 'ConfigureScreen',
				screen,
				definition: {
					enabled: true,
					target: { kind: 'Spout', sender: `Footer-${screen}`, extent: { width: 640, height: 360 } }
				}
			});
			await page
				.getByRole('tab')
				.filter({ hasText: /^Screens$/ })
				.click();
			await page
				.locator('section[aria-label="Screens"]')
				.getByRole('button', { name: 'Footer screen', exact: true })
				.click();
			await page.locator('.gc-dv-tab[data-panel-type="screenWarp"]').click();
			await page
				.locator(
					'[data-panel-id="screen-warp"] .native-preview[data-preview-state="available"][data-preview-installed="true"]'
				)
				.waitFor();
			const wait = async () =>
				page.waitForFunction(() => {
					const status = document.querySelector(
						'footer.gc-footer output[aria-label="Video runtime"]'
					);
					return (
						status?.getAttribute('data-state') === 'Running' &&
						/\d+\.\d+ fps/.test(status.textContent ?? '')
					);
				});
			await wait();
			const text = await status.innerText(),
				title = await status.getAttribute('title');
			assert(Number(text.match(/([\d.]+) fps/)?.[1]) > 0, text);
			assert.match(text, /[\d.]+ ms CPU/);
			assert.match(text, /[\d.]+ (MB|GB)/);
			assert(!/submit|\.\.\.|…|MiB/.test(text), text);
			assert(!text.includes('Test GPU') && !text.includes('GPU —'), text);
			assert.match(title, /not display presentation/);
			assert.match(title, /not physical VRAM/);
			const first = Number(await status.getAttribute('data-sequence'));
			await page.waitForTimeout(2200);
			const next = Number(await status.getAttribute('data-sequence'));
			assert(
				next > first && next - first <= 3,
				'coarse runtime observations continue at at most about 1 Hz'
			);
			await post('/api/ui/project-new', {});
			await page.waitForFunction((before) => {
				const status = document.querySelector(
					'footer.gc-footer output[aria-label="Video runtime"]'
				);
				return Number(status?.getAttribute('data-sequence')) >= before + 2;
			}, next);
			assert.notEqual(await status.getAttribute('data-state'), 'Faulted');
			assert.notEqual(await status.getAttribute('data-state'), 'error');
			console.log(
				JSON.stringify({
					nativeVideoFooter: text,
					semantics: title,
					observedUpdates: next - first,
					projectRecovery: true
				})
			);
		}
		const box = await status.boundingBox(),
			footer = await page.locator('footer.gc-footer').boundingBox();
		assert(box && footer && box.x >= footer.x && box.x + box.width <= footer.x + footer.width + 1);
		for (const width of [700, 500]) {
			await page.setViewportSize({ width, height: 850 });
			const bounds = await status.boundingBox();
			const narrowFooter = await page.locator('footer.gc-footer').boundingBox();
			assert(
				bounds &&
					narrowFooter &&
					bounds.x >= narrowFooter.x &&
					bounds.x + bounds.width <= narrowFooter.x + narrowFooter.width + 1,
				`footer fits ${width}`
			);
			assert(!/submit|\.\.\.|…|MiB/.test(await status.innerText()));
		}
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				runtimeFooter:
					'public app contribution preserves engine status; explicit unavailable or measured native metrics',
				native: !!nativePage,
				pageErrors: []
			})
		);
	} finally {
		await browser?.close();
	}
}
