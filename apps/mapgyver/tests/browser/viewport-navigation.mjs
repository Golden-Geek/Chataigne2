import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyViewportNavigation({
	base,
	post,
	edit,
	directory,
	page: nativePage = null
}) {
	async function screens(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	}
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Navigation source'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 320, height: 180 } }
	});
	const { screen, surface } = await screens({ kind: 'CreateScreen', label: 'Navigation screen' });
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Navigation-${screen}`, extent: { width: 320, height: 180 } }
		}
	});
	await screens({
		kind: 'ConfigureSurface',
		reference: surface,
		definition: {
			source,
			crop: { x: 0.1, y: 0.1, width: 0.8, height: 0.8 },
			quad: {
				corners: [
					{ x: 0.1, y: 0.1 },
					{ x: 0.9, y: 0.1 },
					{ x: 0.9, y: 0.9 },
					{ x: 0.1, y: 0.9 }
				]
			},
			opacity: 1,
			enabled: true,
			calibration: 'Off'
		}
	});
	const { snapshot: authoredSurface } = await screens({
		kind: 'InspectSurface',
		reference: surface
	});
	await edit({ kind: 'CreateMedia', media_kind: 'Composition', label: 'Navigation composition' });
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'shared smooth navigation');
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	const reports = [];
	const beforeNavigation = (await screens({ kind: 'InspectManager' })).snapshot;
	try {
		if (!nativePage) await page.goto(base);
		await tab('Screen Manager').click();
		await page
			.locator('section[aria-label="Screen Manager"]')
			.getByRole('button', { name: authoredSurface.label, exact: true })
			.click();
		for (const [label, tabName] of [
			['Screen view', 'Screen Warping'],
			['Surface source view', 'Surface Source'],
			['Composition view', 'Composition']
		]) {
			if (tabName === 'Composition') {
				await tab('Media Manager').click();
				await page
					.locator('section[aria-label="Media Manager"]')
					.getByRole('treeitem', { name: 'Navigation composition', exact: true })
					.getByRole('button', { name: 'Navigation composition', exact: true })
					.click();
			}
			await tab(tabName).click();
			const view = page.getByRole('application', { name: label, exact: true });
			await view.waitFor();
			const toolbar = view.locator('.canvas-toolbar');
			const viewBox = await view.boundingBox(),
				toolbarBox = await toolbar.boundingBox();
			assert(
				viewBox &&
					toolbarBox &&
					Math.abs(toolbarBox.x + toolbarBox.width / 2 - viewBox.x - viewBox.width / 2) < 1,
				`${label} toolbar is centered`
			);
			assert.equal(await view.locator('.editor-toolbar').count(), 0);
			const active = view.locator('.active-zone');
			if (nativePage)
				await view
					.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
					.waitFor();
			await view.getByRole('button', { name: 'Home view', exact: true }).click();
			const before = await active.boundingBox();
			assert(before);
			const fraction = { x: 0.4, y: 0.45 },
				anchor = {
					x: before.x + before.width * fraction.x,
					y: before.y + before.height * fraction.y
				};
			await view.evaluate(
				(element, { fraction }) => {
					window.__navigationSamples = [];
					const until = performance.now() + 500;
					const sample = () => {
						if (!element.isConnected) return;
						const rect = element.querySelector('.active-zone').getBoundingClientRect();
						const image = element.querySelector('.native-preview')?.getBoundingClientRect();
						const overlay = element.querySelector('svg[role="group"]')?.getBoundingClientRect();
						window.__navigationSamples.push({
							zoom: Number(element.querySelector('output[data-zoom]').dataset.zoom),
							x: rect.x + rect.width * fraction.x,
							y: rect.y + rect.height * fraction.y,
							image: image?.toJSON(),
							overlay: overlay?.toJSON()
						});
						if (performance.now() < until) requestAnimationFrame(sample);
					};
					requestAnimationFrame(sample);
				},
				{ fraction }
			);
			await page.mouse.move(anchor.x, anchor.y);
			await page.mouse.wheel(0, -100);
			await page.waitForFunction(
				({ label, target }) =>
					Math.abs(
						Number(
							document.querySelector(`.viewport-2d[aria-label="${label}"] output[data-zoom]`)
								?.dataset.zoom
						) - target
					) < 1e-9,
				{ label, target: Math.exp(0.14) }
			);
			const samples = await page.evaluate(() => window.__navigationSamples);
			const intermediate = samples.filter(
				(sample) => sample.zoom > 1 && sample.zoom < Math.exp(0.14)
			);
			assert(new Set(intermediate.map((sample) => sample.zoom)).size >= 2, `${label} animates`);
			for (const sample of samples) {
				assert(
					Math.abs(sample.x - anchor.x) <= 1 && Math.abs(sample.y - anchor.y) <= 1,
					`${label} retains pointer anchor`
				);
				assert(sample.image && sample.overlay, `${label} exposes both image and overlay geometry`);
				for (const axis of ['x', 'y', 'width', 'height'])
					assert(
						Math.abs(sample.image[axis] - sample.overlay[axis]) <= 1,
						`${label} shares native/overlay DOM geometry`
					);
			}
			// Domain pointer input freezes any unfinished camera before hit testing.
			await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
			await page.mouse.move(anchor.x, anchor.y);
			await page.mouse.down();
			const frozen = await view.locator('output[data-zoom]').getAttribute('data-zoom');
			await page.waitForTimeout(180);
			assert.equal(await view.locator('output[data-zoom]').getAttribute('data-zoom'), frozen);
			await page.keyboard.press('Escape');
			await page.mouse.up();
			if (nativePage)
				await view
					.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
					.waitFor();
			reports.push({
				view: label,
				intermediateFrames: intermediate.length,
				pointerAnchor: true,
				domImageOverlayGeometry: true,
				editCancelsMotion: true
			});
			await page.emulateMedia({ reducedMotion: 'reduce' });
			await view.getByRole('button', { name: 'Home view', exact: true }).click();
			await view.getByRole('button', { name: 'Zoom in', exact: true }).click();
			assert(
				Math.abs(Number(await view.locator('output[data-zoom]').getAttribute('data-zoom')) - 1.2) <
					1e-9,
				`${label} reduced motion applies immediately`
			);
			await page.emulateMedia({ reducedMotion: 'no-preference' });
		}
		assert.deepEqual(
			(await screens({ kind: 'InspectManager' })).snapshot,
			beforeNavigation,
			'camera navigation does not author Screen/Surface settings'
		);
		const project = path.join(directory, 'navigation.mapgyver');
		await post('/api/ui/project-save', { path: project });
		const composition = page.getByRole('application', { name: 'Composition view', exact: true });
		await composition.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await post('/api/ui/project-new', {});
		await composition.waitFor({ state: 'detached' });
		await post('/api/ui/project-load', { path: project });
		await page.getByRole('tab').first().click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Navigation composition', exact: true })
			.click();
		await tab('Composition').click();
		await composition.waitFor();
		await page.waitForTimeout(200);
		assert.equal(
			await composition.locator('output[data-zoom]').getAttribute('data-zoom'),
			'1',
			'same UUID reload resets obsolete generation camera'
		);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				smoothNavigation: reports,
				sameUuidProjectReplacement: true,
				native: !!nativePage,
				pageErrors: []
			})
		);
	} finally {
		await browser?.close();
	}
}
