// Real bundled UI against the ordinary host used by tests/headless/media.mjs --ui-reload.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyPictureReloadUi({ base, inspect, picture, asset }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	const errors = [];
	const rejected = observeRejectedRequests(page, () => 'Picture reload and retained history');
	try {
		page.setDefaultTimeout(5000);
		const reloadRequests = [];
		page.on('pageerror', (error) => errors.push(String(error)));
		page.on('websocket', (socket) => {
			socket.on('framesent', ({ payload }) => {
				if (typeof payload === 'string' && payload.includes('ReloadAsset'))
					reloadRequests.push(JSON.parse(payload));
			});
		});
		await page.goto(base);
		const tab = page.getByRole('tab').filter({ hasText: /^Media Manager$/ });
		await tab.click();
		const manager = page.locator('section[aria-label="Media Manager"]');
		async function wait(read, accept) {
			const deadline = Date.now() + 5000;
			for (;;) {
				const value = await read();
				if (accept(value)) return value;
				assert(Date.now() < deadline, JSON.stringify(value));
				await delay(25);
			}
		}
		const before = await inspect();
		const media = before.media.find((entry) => entry.id === picture.first.media);
		const selected = media.channels.find(
			(entry) => entry.reference.channel === picture.first.channel
		);
		await manager.getByRole('button', { name: media.label, exact: true }).click();
		const channels = page.getByRole('region', { name: 'Channels', exact: true });
		const row = channels
			.locator(
				':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
			)
			.filter({ has: page.getByText(selected.label, { exact: true }) });
		const title = row.locator(':scope > .node-header > .node-title');
		if ((await title.getAttribute('aria-checked')) === 'false') await title.press('Enter');
		const reload = row.getByRole('button', { name: 'Reload Picture', exact: true });
		await reload.waitFor();
		await wait(() => reload.isEnabled(), Boolean);
		// Expanding typed children authors presentation metadata. Establish the retained redo
		// after that navigation, so this check isolates Reload rather than folder expansion.
		await manager
			.getByRole('button', { name: media.label, exact: true })
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: /^Delete(?:\s|$)/ }).click();
		await wait(inspect, (value) => !value.media.some((entry) => entry.id === media.id));
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await manager.getByRole('button', { name: media.label, exact: true }).click();
		await wait(() => reload.isEnabled(), Boolean);
		assert(await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).isEnabled());
		await reload.click();
		await wait(
			() => reloadRequests.length,
			(count) => count === 1
		);
		await wait(() => reload.isEnabled(), Boolean);
		assert.equal(reloadRequests.length, 1);
		assert(
			JSON.stringify(reloadRequests[0]).includes(asset),
			'UI must send the selected asset identity'
		);
		assert.deepEqual(await inspect(), before);
		// Dockview keeps its assertive announcer mounted even when it has nothing to announce.
		// Preserve failures for every populated announcer and every application alert.
		const alerts = await page.getByRole('alert').evaluateAll((elements) =>
			elements.map((element) => ({
				text: element.textContent,
				emptyDockviewAnnouncer:
					element.classList.contains('dv-live-region-assertive') && element.textContent === ''
			}))
		);
		assert.deepEqual(
			alerts.filter((alert) => !alert.emptyDockviewAnnouncer),
			[],
			'Unexpected UI alerts'
		);
		// The previous authored step is RemoveMedia/undo. Reload must not consume its redo entry.
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await manager
			.getByRole('button', { name: media.label, exact: true })
			.waitFor({ state: 'detached' });
		assert.equal(
			(await inspect()).media.some((entry) => entry.id === picture.first.media),
			false
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await manager.getByRole('button', { name: media.label, exact: true }).waitFor();
		assert.deepEqual(await inspect(), before);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope: 'real Reload Picture UI, unchanged catalog/IDs and preserved redo/undo',
				browser: browser.version(),
				asset,
				page_errors: errors.length
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				scope: 'Picture reload failure',
				page_errors: errors,
				rejected_requests: rejected,
				alerts: await page.getByRole('alert').allTextContents(),
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		await page.screenshot({ path: 'target/mapgyver-picture-reload-failure.png' });
		throw error;
	} finally {
		await browser.close();
	}
}
