import assert from 'node:assert/strict';

// Presentation-only dimensions deliberately exercise fractional CSS-pixel boundaries.
export async function verifyMatrixBoundaries({ page, grid, lease, snapshot }) {
	const dimensions = await grid.evaluate((element) => ({
		rows: Number(element.getAttribute('aria-rowcount')) - 2,
		columns: Number(element.getAttribute('aria-colcount')) - 2
	}));
	const depth = Math.max(0, ...snapshot.column_groups.map((group) => group.depth + 1));
	const originalFont = await page.evaluate(() => document.documentElement.style.fontSize);
	const handle = await grid.elementHandle();
	const thresholds = [];
	try {
		for (const rem of [12, 16, 24, 32]) {
			await page.evaluate((size) => {
				document.documentElement.style.fontSize = `${size}px`;
			}, rem);
			for (const dx of [-1, 0, 1])
				for (const dy of [-1, 0, 1]) {
					const result = await grid.evaluate(
						async (element, { rem, dx, dy, dimensions, depth }) => {
							const original = element.style.cssText;
							const gutter = element.offsetWidth - element.clientWidth;
							const minimumWidth = (14 + dimensions.columns * 7.5 + 2.25) * rem;
							const minimumHeight =
								(3.5 + Math.min(depth * 1.8, 8) + dimensions.rows * 7.5 + 2.25) * rem;
							element.style.width = `${minimumWidth + gutter + dx}px`;
							element.style.height = `${minimumHeight + dy}px`;
							const samples = [];
							try {
								for (let frame = 0; frame < 40; frame++) {
									await new Promise(requestAnimationFrame);
									if (frame < 15) continue;
									const header = element.querySelector('.row-label');
									const bounds = header.getBoundingClientRect();
									const editors = [
										...header.querySelectorAll('[data-matrix-parameter] .editor')
									].map((editor) => {
										const rect = editor.getBoundingClientRect();
										return (
											rect.left >= bounds.left &&
											rect.right <= bounds.right &&
											rect.bottom <= bounds.bottom
										);
									});
									samples.push({
										client: [element.clientWidth, element.clientHeight],
										content: [element.scrollWidth, element.scrollHeight],
										spans: [
											getComputedStyle(element).getPropertyValue('--row'),
											getComputedStyle(element).getPropertyValue('--column')
										],
										lease:
											element
												.closest('.viewport-container')
												.querySelector('.native-preview')
												?.getAttribute('data-preview-view') ?? null,
										editorsFit: editors.length === 2 && editors.every(Boolean)
									});
								}
							} finally {
								element.style.cssText = original;
							}
							return { rem, dx, dy, minimumWidth, minimumHeight, samples };
						},
						{ rem, dx, dy, dimensions, depth }
					);
					assert.equal(
						new Set(result.samples.map(JSON.stringify)).size,
						1,
						'exact-fit geometry and scrollbars settle'
					);
					const sample = result.samples.at(-1);
					assert.equal(
						sample.lease,
						lease,
						'font/threshold changes keep the populated preview lease'
					);
					assert(sample.editorsFit, 'inline editors stay inside the minimum-size row header');
					assert(
						Math.abs(sample.client[0] - (result.minimumWidth + dx)) <= 2,
						'width straddles the independent minimum'
					);
					if (dx === 1)
						assert(
							sample.content[0] <= sample.client[0] + 1,
							'fitted width has no excess horizontal extent'
						);
					thresholds.push({
						...result,
						samples: undefined,
						settled: sample,
						stable_frames: result.samples.length
					});
				}
		}
	} finally {
		await page.evaluate((font) => {
			document.documentElement.style.fontSize = font;
		}, originalFont);
	}
	assert(await handle.evaluate((element) => element.isConnected));
	await grid.evaluate(async () => {
		for (let frame = 0; frame < 10; frame++) await new Promise(requestAnimationFrame);
	});
	const original = await grid.boundingBox();
	const originalClient = await grid.evaluate((element) => element.clientWidth);
	const candidates = page.locator(
		'.dv-horizontal > .dv-sash-container > .dv-sash:not(.dv-disabled)'
	);
	let divider;
	for (let i = 0; i < (await candidates.count()); i++) {
		const candidate = candidates.nth(i),
			rect = await candidate.boundingBox();
		if (
			rect &&
			rect.height > 100 &&
			rect.width <= 20 &&
			Math.abs(rect.x - original.x - original.width) < 24
		) {
			divider = candidate;
			break;
		}
	}
	assert(divider, 'actual Matrix dock divider is available');
	async function drag(x, y, destination) {
		await page.mouse.move(x, y);
		await page.mouse.down();
		try {
			await page.mouse.move(destination, y, { steps: 12 });
		} finally {
			await page.mouse.up();
		}
	}
	async function settledDock() {
		return grid.evaluate(async (element) => {
			const samples = [];
			for (let frame = 0; frame < 45; frame++) {
				await new Promise(requestAnimationFrame);
				if (frame >= 15)
					samples.push([
						element.clientWidth,
						element.clientHeight,
						element.scrollWidth,
						element.scrollHeight
					]);
			}
			return samples;
		});
	}
	const rect = await divider.boundingBox();
	await drag(rect.x + rect.width / 2, rect.y + rect.height / 2, rect.x - 1000);
	const minimum = await settledDock();
	assert.equal(new Set(minimum.map(JSON.stringify)).size, 1, 'dock minimum does not oscillate');
	assert(
		minimum.at(-1)[0] < original.width - 20,
		'trusted divider drag reaches a smaller real dock layout'
	);
	const moved = await divider.boundingBox();
	await drag(moved.x + moved.width / 2, moved.y + moved.height / 2, rect.x + rect.width / 2);
	const restored = await settledDock();
	assert.equal(
		new Set(restored.map(JSON.stringify)).size,
		1,
		'restored dock bounds do not oscillate'
	);
	assert(
		restored.at(-1)[0] > minimum.at(-1)[0] + 20,
		`dock re-expands: ${JSON.stringify({ originalClient, minimum: minimum.at(-1), expanded: restored.at(-1) })}`
	);
	return {
		thresholds,
		dock: {
			original_client_width: originalClient,
			minimum: minimum.at(-1),
			expanded: restored.at(-1),
			stable_frames_each: 30
		}
	};
}
