import assert from 'node:assert/strict';

/** Rasterize the actual SVG owner; geometry assertions alone missed cross-view clipping. */
export async function verifyRetirement(browser, baseUrl) {
	const page = await browser.newPage({ viewport: { width: 400, height: 300 } });
	try {
		await page.goto(`${baseUrl}?mode=retirement`);
		await page.waitForFunction(() => window.__retirementLayer);
		const result = await page.evaluate(async () => {
			const layer = window.__retirementLayer;
			const covers = [
				['old-left', { x: 0, y: 0, width: 200, height: 150 }],
				['old-right', { x: 200, y: 0, width: 140, height: 150 }]
			];
			const left = ['left', { x: 0, y: 0, width: 140, height: 150 }];
			const right = ['right', { x: 140, y: 0, width: 200, height: 150 }];
			async function paint(apertures) {
				layer.update(covers, apertures);
				const element = layer.element.cloneNode(true);
				element.setAttribute('width', '400');
				element.setAttribute('height', '300');
				element.removeAttribute('style');
				const source = new Blob([new XMLSerializer().serializeToString(element)], {
					type: 'image/svg+xml'
				});
				const url = URL.createObjectURL(source);
				try {
					const image = new Image();
					image.src = url;
					await image.decode();
					const canvas = document.createElement('canvas');
					canvas.width = 400;
					canvas.height = 300;
					const context = canvas.getContext('2d');
					context.fillStyle = 'red';
					context.fillRect(0, 0, 400, 300);
					context.drawImage(image, 0, 0);
					return [70, 170, 250, 370].map((x) => [...context.getImageData(x, 75, 1, 1).data]);
				} finally {
					URL.revokeObjectURL(url);
				}
			}
			const crossing = await paint([left, right]);
			const retired = await paint([left]);
			const overlapping = await paint([
				left,
				right,
				['overlap', { x: 100, y: 0, width: 100, height: 150 }]
			]);
			const fenced = await paint([]);
			for (let index = 0; index < 1000; index++) layer.update(covers, [left, right]);
			const rectangles = layer.element.querySelectorAll('[data-native-preview-retirement]').length;
			const full = Array.from({ length: 64 }, (_, index) => [
				String(index),
				{ x: index, y: 0, width: 1, height: 1 }
			]);
			layer.update(full, full);
			const capacityRectangles = layer.element.querySelectorAll(
				'[data-native-preview-retirement]'
			).length;
			let overflowRejected = false;
			try {
				layer.update([...full, ['overflow', full[0][1]]], full);
			} catch {
				overflowRejected = true;
			}
			return {
				crossing,
				retired,
				overlapping,
				fenced,
				rectangles,
				capacityRectangles,
				overflowRejected
			};
		});
		const red = [255, 0, 0, 255],
			black = [0, 0, 0, 255];
		assert.deepEqual(
			result.crossing,
			[red, red, red, red],
			'one view must not cover its live neighbor'
		);
		assert.deepEqual(result.overlapping, result.crossing, 'overlapping apertures remain a union');
		assert.deepEqual(result.retired, [red, black, black, red], 'retired neighbor stays covered');
		assert.deepEqual(
			result.fenced,
			[black, black, black, red],
			'no live aperture exposes historical pixels'
		);
		assert.equal(result.rectangles, 4, 'layout churn reuses bounded SVG primitives');
		assert.equal(result.capacityRectangles, 128, 'full capacity remains linear in view count');
		assert.equal(result.overflowRejected, true);
		return result;
	} finally {
		await page.close();
	}
}
