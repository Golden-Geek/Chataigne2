import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

/** Same authored flow in an ordinary browser and the production native WebView. */
export async function verifyPreviewContinuity({
	base,
	edit,
	post,
	directory,
	page: nativePage = null,
	observe = async () => {}
}) {
	const refs = [];
	for (const [label, rgba] of [
		['Concurrent A', [1, 0, 0, 1]],
		['Concurrent B', [0, 0, 1, 1]]
	]) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
		await edit({
			kind: 'ConfigureChannel',
			reference,
			source: { kind: 'SolidColor', rgba }
		});
		refs.push(reference);
		for (let index = 1; index < 4; index++)
			await edit({ kind: 'AddChannel', media: reference.media, label: `${label} ${index}` });
	}
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(8000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'concurrent Media Preview');
	// Fault injection closes the real transport; recovery uses ordinary application code.
	await page.addInitScript(() => {
		window.__previewSockets = [];
		const Original = window.WebSocket;
		window.WebSocket = class extends Original {
			constructor(...args) {
				super(...args);
				window.__previewSockets.push(this);
			}
		};
	});
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}$`) });
	const panel = (id) => page.locator(`.media-preview-panel[data-panel-id="${id}"]`);
	const main = panel('media-preview');
	async function select(label) {
		await tab('Medias').click();
		await page
			.locator('section[aria-label="Medias"]')
			.getByRole('button', { name: label, exact: true })
			.click();
	}
	async function selected(target, media) {
		await page.waitForFunction(
			({ id, media }) =>
				document.querySelector(`.media-preview-panel[data-panel-id="${id}"]`)?.dataset
					.previewMedia === media,
			{ id: await target.getAttribute('data-panel-id'), media }
		);
	}
	async function settled(count) {
		await page.waitForFunction(
			({ count, native }) => {
				const views = [...document.querySelectorAll('.media-preview-panel .native-preview')];
				return (
					views.length === count &&
					!document.querySelector('[data-native-preview-retirement]') &&
					(!native ||
						views.every(
							(view) =>
								view.dataset.previewState === 'available' &&
								view.dataset.previewInstalled === 'true' &&
								!view.querySelector('.unavailable')
						))
				);
			},
			{ count, native: !!nativePage }
		);
	}
	async function checkpoint(phase, count) {
		await settled(count);
		await observe(phase, { first: refs[0], second: refs[1] });
	}
	try {
		if (nativePage) await page.reload();
		else await page.goto(base);
		await select('Concurrent A');
		await tab('Media Preview').click();
		await selected(main, refs[0].media);
		await main.getByRole('button', { name: 'Lock media', exact: true }).click();
		await main.getByRole('button', { name: 'All-channel grid', exact: true }).click();
		await checkpoint('first-grid', 4);
		await select('Concurrent B');
		const inspectorGroup = page
			.getByRole('tab')
			.filter({ hasText: /^Inspector(?:$|:)/ })
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await inspectorGroup
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		const secondId = await page
			.locator('.media-preview-panel')
			.evaluateAll(
				(elements) =>
					elements.find((element) => element.dataset.panelId !== 'media-preview')?.dataset.panelId
			);
		assert(secondId);
		const peer = panel(secondId);
		await selected(peer, refs[1].media);
		await peer.getByRole('button', { name: 'Lock media', exact: true }).click();
		await peer.getByRole('button', { name: 'All-channel grid', exact: true }).click();
		await checkpoint('concurrent-eight', 8);
		const boxes = await Promise.all([main.boundingBox(), peer.boundingBox()]);
		assert(boxes.every((box) => box.width > 0 && box.height > 0));
		assert(boxes[0].x + boxes[0].width <= boxes[1].x + 1, 'two actual side-by-side dock groups');
		await main.getByRole('button', { name: 'Single channel', exact: true }).click();
		const channel = main.getByRole('combobox', { name: 'Preview channel', exact: true });
		await channel.selectOption({ label: 'Concurrent A 2' });
		const chosen = await channel.inputValue();
		await checkpoint('independent-channel-choice', 5);
		const previousSocketCount = await page.evaluate(() => window.__previewSockets.length);
		await page.evaluate(() => {
			const socket = window.__previewSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw new Error('missing live owned application transport');
			socket.close(4001, 'preview reconnect acceptance');
		});
		await page.waitForFunction(
			(count) =>
				window.__previewSockets.length > count &&
				window.__previewSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			previousSocketCount
		);
		await selected(main, refs[0].media);
		await selected(peer, refs[1].media);
		await checkpoint('transport-reconnected', 5);
		assert.equal(await channel.inputValue(), chosen);
		await page.waitForFunction(
			({ id, chosen }) =>
				Object.entries(localStorage).some(
					([key, value]) =>
						key.endsWith('layout.v1') && value.includes(id) && value.includes(chosen)
				),
			{ id: secondId, chosen }
		);
		const project = path.join(directory, 'preview-continuity.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		await page.reload();
		await main.waitFor();
		await peer.waitFor();
		await selected(main, refs[0].media);
		await selected(peer, refs[1].media);
		assert.equal(await channel.inputValue(), chosen);
		assert.equal(await main.getAttribute('data-preview-locked'), 'true');
		assert.equal(await peer.getAttribute('data-preview-locked'), 'true');
		assert.equal(await peer.locator('figure').count(), 4);
		await checkpoint('layout-reloaded', 5);
		const peerGroup = peer.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await peerGroup
			.getByRole('tab')
			.filter({ hasText: /^Media Preview$/ })
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		await checkpoint('peer-closed', 1);
		await select('Concurrent B');
		await main.getByRole('button', { name: 'Unlock media', exact: true }).click();
		await selected(main, refs[1].media);
		await main.getByRole('button', { name: 'Lock media', exact: true }).click();
		await checkpoint('lock-changed', 1);
		const mainGroup = main.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
		await mainGroup
			.getByRole('tab')
			.filter({ hasText: /^Media Preview$/ })
			.getByRole('button', { name: 'Close panel', exact: true })
			.click();
		await checkpoint('all-previews-closed', 0);
		await edit({
			kind: 'ConfigureChannel',
			reference: refs[0],
			source: { kind: 'SolidColor', rgba: [0, 1, 0, 1] }
		});
		await checkpoint('output-edited-without-previews', 0);
		// Stop the independent receiver before New, which intentionally destroys old outputs.
		await observe('before-new', { first: refs[0], second: refs[1] });
		await tab('Screen Warping')
			.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]')
			.getByRole('button', { name: 'Add panel in this group', exact: true })
			.click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Media Preview', { exact: true }) })
			.click();
		const fresh = page.locator('.media-preview-panel');
		await selected(fresh, refs[1].media);
		await fresh.getByRole('button', { name: 'Lock media', exact: true }).click();
		await settled(1);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await fresh.getByText('Locked media belongs to another project.', { exact: true }).waitFor();
		await settled(0);
		await post('/api/ui/project-load', { path: project });
		await selected(fresh, refs[1].media);
		await settled(1);
		assert.equal(await fresh.getAttribute('data-preview-locked'), 'true');
		await observe('project-reloaded', { first: refs[0], second: refs[1] });
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		const report = {
			concurrentViews: 8,
			sideBySide: true,
			channelPersisted: chosen,
			actualTransportReconnect: true,
			layoutReload: true,
			allPreviewClose: true,
			projectNewLoadLockRecovery: true,
			native: !!nativePage,
			pageErrors: []
		};
		console.log(JSON.stringify(report));
		return report;
	} catch (error) {
		console.error(
			JSON.stringify({
				preview: await page
					.locator('.media-preview-panel')
					.innerText()
					.catch(() => 'multiple or absent'),
				errors
			})
		);
		throw error;
	} finally {
		await browser?.close();
	}
}
