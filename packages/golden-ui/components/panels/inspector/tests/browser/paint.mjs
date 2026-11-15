import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export async function checkCurvePaint(page, canvas, selected) {
	const settle = () =>
		page.evaluate(
			() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
		);
	const read = async () => {
		await settle();
		const pixels = await canvas.evaluate((element) => {
			const data = element.getContext('2d').getImageData(0, 0, element.width, element.height).data;
			return { width: element.width, height: element.height, rgba: [...data] };
		});
		return {
			width: pixels.width,
			height: pixels.height,
			sha256: createHash('sha256').update(Uint8Array.from(pixels.rgba)).digest('hex')
		};
	};
	await page.getByRole('button', { name: 'Select Curve A', exact: true }).click();
	const owner = await read();
	await page.getByRole('button', { name: 'Select Curve A key 0', exact: true }).click();
	const key = await read();
	assert.notEqual(key.sha256, owner.sha256, 'real selected controls change canvas paint');
	await canvas.focus();
	await page.keyboard.press('Escape');
	assert.equal(await selected.textContent(), 'Curve A');
	const cleared = await read();
	assert.equal(cleared.width, owner.width);
	assert.equal(cleared.height, owner.height);
	assert.notEqual(cleared.sha256, key.sha256, 'idle owner clearing withdraws selected paint');
	return { owner, key, cleared };
}
