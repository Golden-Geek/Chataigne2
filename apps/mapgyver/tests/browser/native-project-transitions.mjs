import assert from 'node:assert/strict';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function verifyNativeProjectTransitions({
	page,
	post,
	edit,
	nativePid,
	directory: privateDirectory
}) {
	const directory = path.resolve(process.env.MAPGYVER_TRANSITION_DIRECTORY);
	await mkdir(directory);
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'));
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const panel = (id) => page.locator(`.media-preview-panel[data-panel-id="${id}"]`);
	const group = (element) =>
		element.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	async function addPreview(target) {
		const previous = await page
			.locator('.media-preview-panel')
			.evaluateAll((panels) => panels.map((panel) => panel.dataset.panelId));
		await target.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		const id = await page
			.locator('.media-preview-panel')
			.evaluateAll(
				(panels, previous) =>
					panels.find((panel) => !previous.includes(panel.dataset.panelId))?.dataset.panelId,
				previous
			);
		assert(id);
		return panel(id);
	}
	async function ready(target) {
		await page.waitForFunction(
			(id) => {
				const view = document.querySelector(
					`.media-preview-panel[data-panel-id="${id}"] .native-preview`
				);
				return (
					view?.dataset.previewState === 'available' &&
					view.dataset.previewInstalled === 'true' &&
					!view.querySelector('.unavailable')
				);
			},
			await target.getAttribute('data-panel-id')
		);
	}
	async function waitFile(name) {
		const deadline = Date.now() + 180_000;
		while (!(await stat(path.join(directory, name)).catch(() => null))) {
			assert(Date.now() < deadline, `native transition observer did not signal ${name}`);
			await delay(100);
		}
	}
	try {
		const { reference } = await edit({
			kind: 'CreateMedia',
			media_kind: 'SolidColor',
			label: 'Transition red'
		});
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		await tab('Medias').click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: 'Transition red', exact: true })
			.click();
		await tab('Media Preview').click();
		let main = panel('media-preview');
		await ready(main);
		await main.getByRole('button', { name: 'Lock media', exact: true }).click();
		const peer = await addPreview(
			group(page.getByRole('tab').filter({ hasText: /^Inspector(?:$|:)/ }))
		);
		await ready(peer);
		await peer.getByRole('button', { name: 'Lock media', exact: true }).click();
		await output.observe('first-grid', { first: reference });
		const peerId = await peer.getAttribute('data-panel-id');
		const originalView = await peer.locator('.native-preview').getAttribute('data-preview-view');
		const menu = page.getByRole('button', { name: 'Open File menu', exact: true });
		await writeFile(
			path.join(directory, 'ready.json'),
			JSON.stringify({
				pid: nativePid,
				box: await peer.locator('.native-preview').boundingBox(),
				ui_box: await menu.boundingBox(),
				dpr: await page.evaluate(() => window.devicePixelRatio)
			})
		);
		console.log(
			JSON.stringify({ action: 'start-owned-transition-pixel-observer', directory, pid: nativePid })
		);
		await waitFile('capture-started');
		for (let cycle = 0; cycle < 3; cycle++) {
			await group(main)
				.getByRole('tab')
				.filter({ hasText: /^Media Preview$/ })
				.getByRole('button', { name: 'Close panel', exact: true })
				.click();
			await page.waitForFunction(
				() => document.querySelectorAll('.media-preview-panel').length === 1
			);
			await ready(peer);
			await delay(150);
			main = await addPreview(group(tab('Matrix')));
			await ready(main);
			assert.equal(await main.getAttribute('data-preview-media'), reference.media);
			await main.getByRole('button', { name: 'Lock media', exact: true }).click();
			assert.equal(
				await peer.locator('.native-preview').getAttribute('data-preview-view'),
				originalView
			);
			await output.observePixels(`close-reopen-${cycle}`, [255, 0, 0, 255]);
		}
		const receiver = await output.finish();
		const project = path.join(privateDirectory, 'native-transitions.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		await writeFile(path.join(directory, 'new-started'), 'new');
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await peer.getByText('Locked media belongs to another project.', { exact: true }).waitFor();
		await page.waitForFunction(
			() =>
				!document.querySelector('.media-preview-panel .native-preview') &&
				!document.querySelector('[data-native-preview-retirement]')
		);
		await writeFile(path.join(directory, 'new-settled'), 'new settled');
		await delay(500);
		await writeFile(path.join(directory, 'load-started'), 'load');
		const loaded = await post('/api/ui/project-load', { path: project });
		assert.equal(path.resolve(loaded.path), path.resolve(project));
		await ready(peer);
		assert.equal(await peer.getAttribute('data-preview-media'), reference.media);
		await writeFile(path.join(directory, 'load-settled'), 'load settled');
		await delay(500);
		await output.observePixels('restored-output', [255, 0, 0, 255]);
		await writeFile(path.join(directory, 'complete'), 'complete');
		await waitFile('capture-finished');
		const pixels = JSON.parse(await readFile(path.join(directory, 'pixels.json'), 'utf8'));
		assert.equal(pixels.result, 'pass', JSON.stringify(pixels));
		const report = {
			cycles: 3,
			peerId,
			originalView,
			peerRegistrationPreserved: true,
			newViewsRetired: true,
			lockedLoadRecovered: true,
			receiver,
			pixels,
			outputPixels: output.observations
		};
		await writeFile(
			path.join(directory, 'project-transitions.json'),
			JSON.stringify(report, null, 2)
		);
		console.log(JSON.stringify(report));
	} finally {
		await output.finish();
	}
}
