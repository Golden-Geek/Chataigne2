import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { verifyNavigationPixels } from './navigation-pixels.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyViewportNavigation({
	base,
	post,
	edit,
	directory,
	page: nativePage = null,
	nativePid,
	nativePixels = false
}) {
	assert(!nativePixels || (nativePage && nativePid));
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
	let surfaceSource = source;
	const inset = nativePixels ? 0.25 : 0.1;
	if (nativePixels) {
		await edit({
			kind: 'ConfigureChannel',
			reference: source,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		const picture = path.join(directory, 'navigation-landmark.png');
		await promisify(execFile)(
			'python',
			[
				'-c',
				'from PIL import Image; import sys; im=Image.new("RGB",(320,180),"black"); im.paste("red",(80,45,240,135)); im.save(sys.argv[1])',
				picture
			],
			{ windowsHide: true, timeout: 8000 }
		);
		({ reference: surfaceSource } = await edit({
			kind: 'CreateMedia',
			media_kind: 'Picture',
			label: 'Navigation landmark'
		}));
		await edit({
			kind: 'ImportPicture',
			target: surfaceSource,
			locator: { kind: 'absolute', path: picture },
			label: null
		});
	}
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
			source: surfaceSource,
			crop: { x: inset, y: inset, width: 1 - 2 * inset, height: 1 - 2 * inset },
			quad: {
				corners: [
					{ x: inset, y: inset },
					{ x: 1 - inset, y: inset },
					{ x: 1 - inset, y: 1 - inset },
					{ x: inset, y: 1 - inset }
				]
			},
			opacity: 1,
			enabled: true,
			calibration: 'Off'
		}
	});
	await screens({ kind: 'RenameSurface', reference: surface, label: 'Navigation warped surface' });
	const { snapshot: authoredSurface } = await screens({
		kind: 'InspectSurface',
		reference: surface
	});
	const { reference: compositionReference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Navigation composition'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: compositionReference.media,
		size: { kind: 'Fixed', extent: { width: 640, height: 360 } }
	});
	const { instances } = await edit({
		kind: 'Composition',
		reference: compositionReference,
		edit: { kind: 'AddInstances', sources: [source] }
	});
	if (nativePixels) {
		const current = (
			await edit({
				kind: 'Composition',
				reference: compositionReference,
				edit: { kind: 'Inspect' }
			})
		).snapshot;
		await edit({
			kind: 'Composition',
			reference: compositionReference,
			edit: {
				kind: 'ConfigureInstance',
				instance: instances[0].instance,
				definition: {
					...current.instances[0].definition,
					transform: {
						position: [208, 90],
						scale: [1, 1],
						rotation_degrees: 0,
						pivot: [0, 0]
					}
				}
			}
		});
	}
	await edit({
		kind: 'Composition',
		reference: compositionReference,
		edit: {
			kind: 'RenameInstance',
			instance: instances[0].instance,
			label: 'Navigation framed item'
		}
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'shared smooth navigation');
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp(`^${name}$`) });
	const reports = [];
	const hwnd = nativePixels ? Number(ownedWindowHandle(nativePid)) : null;
	async function transition(view, action) {
		await view.evaluate((element) => {
			window.__frameHomeSamples = [];
			window.__frameHomeDone = false;
			const until = performance.now() + 400;
			function sample() {
				const image = element.querySelector('.native-preview')?.getBoundingClientRect();
				const overlay = element.querySelector('svg[role="group"]')?.getBoundingClientRect();
				window.__frameHomeSamples.push({
					zoom: Number(element.querySelector('output[data-zoom]').dataset.zoom),
					image: image?.toJSON(),
					overlay: overlay?.toJSON()
				});
				if (performance.now() < until) requestAnimationFrame(sample);
				else window.__frameHomeDone = true;
			}
			requestAnimationFrame(sample);
		});
		await action();
		await page.waitForFunction(() => window.__frameHomeDone);
		const samples = await page.evaluate(() => window.__frameHomeSamples);
		const low = Math.min(samples[0].zoom, samples.at(-1).zoom),
			high = Math.max(samples[0].zoom, samples.at(-1).zoom);
		const intermediate = samples.filter((sample) => sample.zoom > low && sample.zoom < high);
		assert(
			new Set(intermediate.map((sample) => sample.zoom)).size >= 2,
			`${await view.getAttribute('aria-label')} Frame/Home present intermediate cameras: ${JSON.stringify(samples.map((sample) => sample.zoom))}`
		);
		for (const sample of samples) {
			assert(sample.image && sample.overlay, 'navigation keeps image and controls mounted');
			for (const axis of ['x', 'y', 'width', 'height'])
				assert(
					Math.abs(sample.image[axis] - sample.overlay[axis]) <= 1,
					'Frame/Home share image/control DOM geometry'
				);
		}
		return intermediate.length;
	}
	const beforeNavigation = (await screens({ kind: 'InspectManager' })).snapshot;
	try {
		if (!nativePage) await page.goto(base);
		await tab('Screens').click();
		await page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: authoredSurface.label, exact: true })
			.click();
		for (const [label, panelId] of [
			['Screen view', 'screen-warp'],
			['Surface source view', 'surface-source'],
			['Composition view', 'composition']
		]) {
			if (panelId !== 'composition') {
				await tab('Screens').click();
				await page
					.locator('section[aria-label="Screens"]')
					.getByRole('button', { name: authoredSurface.label, exact: true })
					.click();
			}
			if (panelId === 'composition') {
				await tab('Medias').click();
				await page
					.locator('section[aria-label="Medias"]')
					.getByRole('treeitem', { name: 'Navigation composition', exact: true })
					.getByRole('button', { name: 'Navigation composition', exact: true })
					.click();
				const parent = page
					.locator('section[aria-label="Medias"]')
					.getByRole('treeitem', { name: 'Navigation composition', exact: true });
				if ((await parent.getAttribute('aria-expanded')) !== 'true') {
					await parent.focus();
					await parent.press('ArrowRight');
				}
				await page
					.locator('section[aria-label="Medias"]')
					.getByRole('treeitem', { name: 'Navigation framed item', exact: true })
					.getByRole('button', { name: 'Navigation framed item', exact: true })
					.click();
			}
			await page.locator(`.gc-dv-tab[data-content-panel-id="${panelId}"]`).click();
			const view = page.getByRole('application', { name: label, exact: true });
			await view.waitFor();
			if (panelId === 'surface-source')
				await view.getByRole('button', { name: 'Select source crop', exact: true }).press('Enter');
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
			const frame = () =>
				transition(view, () =>
					view.getByRole('button', { name: 'Frame selection', exact: true }).click()
				);
			const framePixels = nativePixels
				? await verifyNavigationPixels({
						page,
						view,
						hwnd,
						label: `${label} Frame`,
						navigate: frame
					})
				: null;
			const frameFrames = framePixels?.navigation ?? (await frame());
			await view.focus();
			const home = () => transition(view, () => view.press('h'));
			const homePixels = nativePixels
				? await verifyNavigationPixels({ page, view, hwnd, label: `${label} Home`, navigate: home })
				: null;
			const homeFrames = homePixels?.navigation ?? (await home());
			assert.equal(
				await view.locator('output[data-zoom]').getAttribute('data-zoom'),
				'1',
				`${label} Home reaches exact zoom`
			);
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
				frameFrames,
				homeFrames,
				...(nativePixels ? { framePixels, homePixels } : {}),
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
		await page.locator('.gc-dv-tab[data-content-panel-id="composition"]').click();
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
