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
							version: request.version,
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
		const initialCover = screen.locator('.unavailable');
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
		assert.deepEqual(
			await page.evaluate(
				() => window.__continuityRequests.find((request) => request.kind === 'Layout').background
			),
			[31, 32, 33, 255]
		);
		await page.evaluate(() =>
			document.documentElement.style.setProperty('--gc-color-panel', 'rgb(72, 74, 80)')
		);
		await page.waitForFunction(() =>
			window.__continuityRequests.some(
				(request) =>
					request.kind === 'Layout' && JSON.stringify(request.background) === '[72,74,80,255]'
			)
		);
		assert.equal(
			await page.evaluate(
				() =>
					window.__continuityRequests.filter((request) => request.kind === 'Layout').at(-1).views[0]
						.id
			),
			firstId,
			'theme update retains the real view identity'
		);
		await page.evaluate(() => {
			window.__continuityFlashes = [];
			window.__continuityEdgeOverlap = 0;
			const initial = document
				.querySelector('[data-continuity-screen] .native-preview')
				.getBoundingClientRect();
			new MutationObserver(() => {
				const screen = document.querySelector('[data-continuity-screen] .native-preview');
				if (screen.textContent.includes('Preview unavailable'))
					window.__continuityFlashes.push('placeholder');
				const bounds = screen.getBoundingClientRect();
				for (const mask of document.querySelectorAll('[data-native-preview-retirement]')) {
					const rect = mask.getBoundingClientRect();
					const width = Math.min(rect.right, bounds.right) - Math.max(rect.left, bounds.left);
					const height = Math.min(rect.bottom, bounds.bottom) - Math.max(rect.top, bounds.top);
					// Fixed CSS boxes quantize at 1/64px; transformed aperture bounds can sit
					// halfway between those steps. Record that edge error separately from covers.
					if (width > 1 / 64 && height > 1 / 64) window.__continuityFlashes.push('retirement mask');
					else if (width > 0 && height > 0)
						window.__continuityEdgeOverlap = Math.max(
							window.__continuityEdgeOverlap,
							Math.min(width, height)
						);
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
		const viewport = page.getByRole('application', { name: 'Continuity view' });
		const initialRect = await screen.boundingBox();
		await viewport.getByRole('button', { name: 'Zoom in', exact: true }).click();
		await page.waitForFunction(
			(width) =>
				document.querySelector('[data-continuity-screen] .native-preview').getBoundingClientRect()
					.width > width,
			initialRect.width
		);
		await viewport.focus();
		await page.keyboard.down('Space');
		await page.mouse.move(
			initialRect.x + initialRect.width / 2,
			initialRect.y + initialRect.height / 2
		);
		await page.mouse.down();
		await page.mouse.move(
			initialRect.x + initialRect.width / 2 + 24,
			initialRect.y + initialRect.height / 2 + 12,
			{ steps: 8 }
		);
		await page.mouse.up();
		await page.keyboard.up('Space');
		await page.waitForFunction(
			(left) =>
				window.__continuityRequests.filter((request) => request.kind === 'Layout').at(-1)?.views[0]
					.rect.x !== left,
			initialRect.x
		);
		assert.equal(
			await screen.locator('.unavailable').count(),
			0,
			'navigation retains its presented overlap'
		);
		assert.deepEqual(await page.evaluate(() => window.__continuityFlashes), []);
		await page.evaluate(() => (window.__continuityInstalled = true));
		await page.waitForFunction(() => document.querySelector('[data-preview-installed="true"]'));
		await page.evaluate(() => (window.__continuityInstalled = false));
		await page.getByRole('button', { name: 'Toggle Inspector preview' }).click();
		await page.getByLabel('Inspector preview', { exact: true }).locator('.unavailable').waitFor();
		assert.equal(
			await page
				.getByLabel('Inspector preview', { exact: true })
				.locator('.unavailable')
				.evaluate((element) => getComputedStyle(element).backgroundColor),
			'rgb(0, 0, 0)'
		);
		assert.equal(await screen.getByText('Preview unavailable').count(), 0);
		await page.evaluate(() => (window.__continuityInstalled = true));
		await page
			.getByLabel('Inspector preview', { exact: true })
			.locator('.unavailable')
			.waitFor({ state: 'detached' });
		await page.evaluate(() => (window.__continuityInstalled = false));
		await page.getByRole('button', { name: 'Toggle Inspector preview' }).click();
		await page.getByLabel('Inspector preview', { exact: true }).waitFor({ state: 'detached' });
		await page.waitForFunction(
			() =>
				window.__continuityRequests.at(-1)?.kind === 'Layout' &&
				window.__continuityRequests.at(-1).views.length === 1
		);
		const retirement = await page
			.locator('[data-native-preview-retirement-layer]')
			.evaluate((element) => ({
				covers: element.querySelectorAll('[data-native-preview-retirement^="cover:"]').length,
				paint: getComputedStyle(element.querySelector(':scope > rect')).fill,
				background: getComputedStyle(document.documentElement)
					.getPropertyValue('--gc-color-background')
					.trim()
			}));
		assert(retirement.covers > 0, 'removed native view must stay covered until withdrawal');
		const expectedPaint = await page.evaluate((background) => {
			const element = document.createElement('div');
			element.style.color = background || '#000';
			document.body.append(element);
			const color = getComputedStyle(element).color;
			element.remove();
			return color;
		}, retirement.background);
		assert.equal(retirement.paint, expectedPaint);
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
			opaqueRetirementCovers: true,
			sourceSwap: true,
			inspectorChurn: true,
			panZoom: true,
			screenFlashes: result.flashes,
			edgeOverlapCssPixels: await page.evaluate(() => window.__continuityEdgeOverlap),
			stableView: true
		};
	} finally {
		await page.close();
	}
}
