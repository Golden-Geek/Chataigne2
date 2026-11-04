// Generic typed Inspector controls in the actual native WebView; GPU pixels use picture_pixels.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyNativeMediaFit({ page, edit, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_MEDIA_FIT_DIRECTORY);
	await mkdir(directory);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	const fit = parameter('Fit Mode');
	async function wait(read, predicate) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(30);
		}
	}
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Picture',
		label: 'Aspect controls'
	});
	await edit({ kind: 'AddChannel', media: reference.media });
	const read = async () =>
		(await edit({ kind: 'InspectChannels', query: { media: reference.media, start: 0, count: 8 } }))
			.page;
	await page.getByRole('tab').first().click();
	await page.getByRole('button', { name: 'Aspect controls', exact: true }).click();
	const toggle = parameter('Resolution').getByRole('switch', { name: 'Toggle node enabled' });
	await toggle.waitFor();
	assert.equal(await fit.count(), 0);
	await toggle.click();
	await fit.waitFor();
	const choice = fit.locator('select.dropdown-editor');
	assert.equal(await choice.inputValue(), 'Fit');
	for (const mode of ['Fill', 'Stretch', 'Fit']) {
		await choice.selectOption(mode);
		await wait(
			read,
			(page) =>
				page.channels.length === 2 &&
				page.channels.every((channel) => channel.definition.fit_mode === mode)
		);
	}
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (page) =>
		page.channels.every((channel) => channel.definition.fit_mode === 'Stretch')
	);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(read, (page) =>
		page.channels.every((channel) => channel.definition.fit_mode === 'Fit')
	);
	await choice.selectOption('Fill');
	await wait(read, (page) =>
		page.channels.every((channel) => channel.definition.fit_mode === 'Fill')
	);
	await toggle.click();
	await wait(
		() => fit.count(),
		(count) => count === 0
	);
	const auto = await wait(read, (page) =>
		page.channels.every((channel) => channel.definition.size.kind === 'Automatic')
	);
	assert(auto.channels.every((channel) => channel.definition.fit_mode === 'Fill'));
	await toggle.click();
	await fit.waitFor();
	assert.equal(await choice.inputValue(), 'Fill');
	await page.screenshot({ path: path.join(directory, 'media-fit.png') });
	const { reference: solid } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Solid fit omission'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: solid.media,
		size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
	});
	await page.getByRole('button', { name: 'Solid fit omission', exact: true }).click();
	await parameter('Resolution').waitFor();
	assert.equal(await fit.count(), 0);
	const { reference: shared } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Shared fit controls'
	});
	const { media: referenceMedia } = await edit({
		kind: 'CreateReference',
		target: reference.media,
		label: 'Reference fit controls'
	});
	for (const [media, label] of [
		[shared.media, 'Shared fit controls'],
		[referenceMedia, 'Reference fit controls']
	]) {
		await page.getByRole('button', { name: label, exact: true }).click();
		const resolutionToggle = parameter('Resolution').getByRole('switch', {
			name: 'Toggle node enabled'
		});
		await resolutionToggle.waitFor();
		assert.equal(await fit.count(), 0);
		await resolutionToggle.click();
		await fit.waitFor();
		await choice.selectOption('Stretch');
		await wait(
			async () =>
				(await edit({ kind: 'InspectChannels', query: { media, start: 0, count: 8 } })).page,
			(page) =>
				page.channels.length > 0 &&
				page.channels.every((channel) => channel.definition.fit_mode === 'Stretch')
		);
	}
	assert.deepEqual(errors, []);
	const receipt = {
		nativePid,
		modes: ['Fit', 'Fill', 'Stretch'],
		sharedChannels: 2,
		undoRedo: true,
		hiddenInAuto: true,
		autoRetainsChoice: true,
		absentOnSolid: true,
		sharedTextureAndReferenceControls: true,
		pageErrors: errors
	};
	await writeFile(path.join(directory, 'media-fit.json'), JSON.stringify(receipt, null, 2));
	console.log(JSON.stringify(receipt));
}
