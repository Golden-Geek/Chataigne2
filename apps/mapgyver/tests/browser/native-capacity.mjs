import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

// Product-host smoke: incrementally demand three authored 4K channels in the actual WebView.
// The complete eight-output/adapters/retirement contract belongs to capacity_rendering.rs.
export async function verifyNativeCapacity({ page, edit, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_CAPACITY_DIRECTORY);
	await mkdir(directory);
	const baseline = process.env.MAPGYVER_CAPACITY_BASELINE === '1';
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Native 4K capacity'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 4096, height: 4096 } }
	});
	await page
		.getByRole('tab')
		.filter({ hasText: /^Medias$/ })
		.click();
	await page
		.getByRole('region', { name: 'Medias', exact: true })
		.getByRole('button', { name: 'Native 4K capacity', exact: true })
		.click();
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview$/ })
		.click();
	const panel = page.locator('.media-preview-panel[data-panel-id="media-preview"]');
	await panel.getByRole('button', { name: 'All-channel grid', exact: true }).click();
	const samples = [];
	for (let count = 1; count <= 3; count++) {
		if (count > 1) await edit({ kind: 'AddChannel', media: reference.media, label: `4K ${count}` });
		await page.waitForFunction(
			(count) => document.querySelectorAll('.media-preview-panel .native-preview').length === count,
			count
		);
		await delay(1500);
		const status = await page.evaluate(async () =>
			JSON.parse(
				await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
					request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
				})
			)
		);
		const messages = await panel.locator('.message').allTextContents();
		const installed = await panel
			.locator('.native-preview')
			.evaluateAll((views) =>
				views.every(
					(view) => view.dataset.previewInstalled === 'true' && !view.querySelector('.unavailable')
				)
			);
		samples.push({ count, status, messages, installed });
		console.log(JSON.stringify(samples.at(-1)));
		if (messages.some((message) => /resource charge|budget|footprint/i.test(message))) break;
	}
	await writeFile(
		path.join(directory, 'capacity.json'),
		JSON.stringify({ nativePid, baseline, samples }, null, 2)
	);
	if (baseline)
		assert(
			samples.some((sample) =>
				sample.messages.some((message) => /resource charge|budget|footprint/i.test(message))
			),
			'recorded baseline must reproduce admission rejection'
		);
	else {
		assert.equal(samples.length, 3);
		assert(
			samples.every((sample) => sample.installed && sample.messages.length === 0),
			JSON.stringify(samples)
		);
		assert(
			samples.at(-1).status.resources.limit_bytes > 512 * 1024 * 1024,
			'actual adapter-aware ceiling must replace the fixed cap'
		);
		if (process.env.MAPGYVER_CAPACITY_EXPECTED_ADAPTER)
			for (const sample of samples)
				assert.equal(sample.status.gpu_adapter, process.env.MAPGYVER_CAPACITY_EXPECTED_ADAPTER);
	}
}
