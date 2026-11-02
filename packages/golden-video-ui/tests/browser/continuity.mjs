import assert from 'node:assert/strict';

/** Real Svelte/layout ordering with delayed native metadata; no claim about native pixels. */
export async function verifyContinuity(browser, baseUrl) {
	const page = await browser.newPage();
	const errors = [];
	page.on('pageerror', (error) => errors.push(error.message));
	try {
		await page.addInitScript(() => {
			window.isTauri = true;
			window.__continuityInstalled = false;
			window.__continuityRequests = [];
			let views = [];
			window.__TAURI_INTERNALS__ = {
				invoke: async (command, args) => {
					if (command !== 'native_presentation_control') throw new Error(command);
					const request = JSON.parse(args.request);
					window.__continuityRequests.push(request);
					if (request.kind === 'Connect')
						return JSON.stringify({
							kind: 'Connected',
							session: 'continuity',
							version: 1,
							max_views: 64
						});
					if (request.kind === 'Disconnect') return JSON.stringify({ kind: 'Disconnected' });
					if (request.kind === 'Layout') views = request.views;
					return JSON.stringify({
						kind: 'Applied',
						revision: request.revision,
						visible_views: views.filter((view) => view.visible).length,
						installed: window.__continuityInstalled
					});
				}
			};
		});
		await page.goto(`${baseUrl}?mode=continuity`);
		const screen = page.getByLabel('Screen preview', { exact: true });
		const initialCover = screen.getByText('Preview unavailable', { exact: true });
		await initialCover.waitFor();
		assert.equal(
			await initialCover.evaluate((element) => getComputedStyle(element).backgroundColor),
			'rgb(0, 0, 0)'
		);
		await page.evaluate(() => (window.__continuityInstalled = true));
		await screen.locator('[class*="unavailable"]').waitFor({ state: 'detached' });
		await page.waitForFunction(() => document.querySelector('[data-preview-installed="true"]'));
		assert.equal(
			await screen.evaluate((element) => getComputedStyle(element).backgroundColor),
			'rgba(0, 0, 0, 0)'
		);
		const firstId = await page.evaluate(
			() => window.__continuityRequests.find((request) => request.kind === 'Layout').views[0].id
		);
		await page.evaluate(() => {
			window.__continuityFlashes = [];
			new MutationObserver(() => {
				const screen = document.querySelector('[data-continuity-screen]');
				if (screen.textContent.includes('Preview unavailable'))
					window.__continuityFlashes.push('placeholder');
				const bounds = screen.getBoundingClientRect();
				for (const mask of document.querySelectorAll('[data-native-preview-retirement]')) {
					const rect = mask.getBoundingClientRect();
					if (
						rect.left < bounds.right &&
						rect.right > bounds.left &&
						rect.top < bounds.bottom &&
						rect.bottom > bounds.top
					)
						window.__continuityFlashes.push('retirement mask');
				}
			}).observe(document.body, { subtree: true, childList: true, attributes: true });
			window.__continuityInstalled = false;
		});
		await page.getByRole('button', { name: 'Change screen' }).click();
		await page.waitForFunction(() =>
			window.__continuityRequests.some(
				(request) => request.kind === 'Layout' && request.views[0]?.source === 'second-screen'
			)
		);
		assert.equal(await screen.getAttribute('data-preview-installed'), 'false');
		assert.equal(await screen.getByText('Preview unavailable').count(), 0);
		await page.evaluate(() => (window.__continuityInstalled = true));
		await page.waitForFunction(() => document.querySelector('[data-preview-installed="true"]'));
		await page.evaluate(() => (window.__continuityInstalled = false));
		await page.getByRole('button', { name: 'Toggle Inspector preview' }).click();
		await page
			.getByLabel('Inspector preview', { exact: true })
			.getByText('Preview unavailable')
			.waitFor();
		assert.equal(
			await page
				.getByLabel('Inspector preview', { exact: true })
				.getByText('Preview unavailable')
				.evaluate((element) => getComputedStyle(element).backgroundColor),
			'rgb(0, 0, 0)'
		);
		assert.equal(await screen.getByText('Preview unavailable').count(), 0);
		await page.evaluate(() => (window.__continuityInstalled = true));
		await page
			.getByLabel('Inspector preview', { exact: true })
			.getByText('Preview unavailable')
			.waitFor({ state: 'detached' });
		await page.evaluate(() => (window.__continuityInstalled = false));
		await page.getByRole('button', { name: 'Toggle Inspector preview' }).click();
		await page.getByLabel('Inspector preview', { exact: true }).waitFor({ state: 'detached' });
		await page.waitForFunction(
			() =>
				window.__continuityRequests.at(-1)?.kind === 'Layout' &&
				window.__continuityRequests.at(-1).views.length === 1
		);
		const retirementColors = await page
			.locator('[data-native-preview-retirement]')
			.evaluateAll((elements) =>
				elements.map((element) => getComputedStyle(element).backgroundColor)
			);
		assert(retirementColors.length > 0, 'removed native view must stay covered until withdrawal');
		assert(retirementColors.every((color) => color === 'rgb(0, 0, 0)'));
		const result = await page.evaluate(() => ({
			flashes: window.__continuityFlashes,
			ids: window.__continuityRequests
				.filter((request) => request.kind === 'Layout')
				.map((request) => request.views[0].id)
		}));
		assert.deepEqual(result.flashes, []);
		assert(result.ids.every((id) => id === firstId));
		assert.deepEqual(errors, []);
		await page.evaluate(() => (window.__continuityInstalled = true));
		return {
			blackPendingViews: true,
			blackRetirementCovers: true,
			sourceSwap: true,
			inspectorChurn: true,
			screenFlashes: result.flashes,
			stableView: true
		};
	} finally {
		await page.close();
	}
}
