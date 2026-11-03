import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyScreenDetails({ base, post, page: nativePage = null }) {
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 900 } }));
	page.setDefaultTimeout(10000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const edit = async (namespace, payload) => {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: `mapgyver.${namespace}`,
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	};
	try {
		const { reference: source } = await edit('media', {
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Details media'
		});
		const { screen, surface } = await edit('screens', {
			kind: 'CreateScreen',
			label: 'Details screen'
		});
		const definition = {
			enabled: true,
			target: { kind: 'Spout', sender: `Details-${screen}`, extent: { width: 640, height: 360 } }
		};
		await edit('screens', { kind: 'ConfigureScreen', screen, definition });
		const inspected = await edit('screens', { kind: 'InspectSurface', reference: surface });
		await edit('screens', {
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...inspected.snapshot.definition, source }
		});
		if (!nativePage) await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Manager$/ })
			.click();
		const manager = page.locator('section[aria-label="Screen Manager"]');
		const row = manager
			.locator('.outliner-item-content[data-node-id]')
			.filter({ has: page.getByRole('button', { name: 'Details screen', exact: true }) })
			.first();
		const details = row.locator('[aria-label="Screen output details"]');
		await page.waitForFunction(() =>
			[...document.querySelectorAll('[aria-label="Screen output details"]')].some((element) =>
				element.title.includes('640 × 360')
			)
		);
		assert.match(await details.getAttribute('title'), /Spout · Details-/);
		const surfaceDetails = manager
			.locator('[aria-label="Surface source details"]')
			.filter({ hasText: 'Details media' });
		await surfaceDetails.waitFor();
		await edit('media', {
			kind: 'RenameMedia',
			media: source.media,
			label: 'Renamed details media'
		});
		await manager
			.locator('[aria-label="Surface source details"]')
			.filter({ hasText: 'Renamed details media' })
			.waitFor();
		if (nativePage) {
			await page.getByRole('button', { name: 'Details screen', exact: true }).click();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Screen Warping$/ })
				.click();
			await page
				.locator(
					'[data-panel-id="screen-warp"] .native-preview[data-preview-state="available"][data-preview-installed="true"]'
				)
				.waitFor();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Screen Manager$/ })
				.click();
			await page.waitForFunction(() =>
				[...document.querySelectorAll('[aria-label="Screen output details"]')].some((element) =>
					/[1-9]\d*\.\d fps/.test(element.textContent ?? '')
				)
			);
			assert.match(await details.getAttribute('title'), /API-accepted output FPS/);
		} else assert.match(await details.innerText(), /— fps/);
		// Exercise the actual dock split at wide/narrow viewport sizes. Complete details stay in
		// the tooltip while CSS hides secondary columns; no new backend sizing policy is applied.
		for (const width of [2400, 1000]) {
			await page.setViewportSize({ width, height: 900 });
			await page.waitForTimeout(100);
			const bounds = await details.boundingBox(),
				parent = await manager.boundingBox();
			assert(
				bounds &&
					parent &&
					bounds.x >= parent.x &&
					bounds.x + bounds.width <= parent.x + parent.width + 1,
				JSON.stringify({ width, bounds, parent })
			);
			assert.match(await details.getAttribute('title'), /640 × 360/);
			await manager
				.locator('[aria-label="Surface source details"]')
				.filter({ hasText: 'Renamed details media' })
				.waitFor();
		}
		await edit('screens', {
			kind: 'ConfigureScreen',
			screen,
			definition: {
				...definition,
				enabled: false,
				target: {
					kind: 'Rectangle',
					bounds: { left: -320, top: 12, extent: { width: 320, height: 240 } }
				}
			}
		});
		await page.waitForFunction(() =>
			[...document.querySelectorAll('[aria-label="Screen output details"]')].some(
				(element) =>
					element.title.includes('Rectangle · -320, 12') &&
					element.title.includes('320 × 240') &&
					element.title.includes('Disabled')
			)
		);
		await edit('media', { kind: 'RemoveMedia', media: source.media });
		await page.waitForFunction(
			() =>
				![...document.querySelectorAll('[aria-label="Surface source details"]')].some((element) =>
					element.textContent.includes('Renamed details media')
				)
		);
		await post('/api/ui/project-new', {});
		await page
			.getByRole('button', { name: 'Details screen', exact: true })
			.waitFor({ state: 'detached' });
		assert.deepEqual(errors, []);
		console.log(
			JSON.stringify({
				screenDetails:
					'bounded authored target/resolution/source, rename/remove, responsive columns, project replacement',
				actualOutputFps: !!nativePage,
				pageErrors: []
			})
		);
	} finally {
		await browser?.close();
	}
}
