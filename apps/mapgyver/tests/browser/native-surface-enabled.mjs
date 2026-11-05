import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';

export async function verifyNativeSurfaceEnabled({ page, edit, post, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_SURFACE_ENABLED_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Surface metadata output');
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	async function screens(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	}
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Surface enable source'
	});
	const { surface } = await output.demand(source);
	const read = async () => (await screens({ kind: 'InspectSurface', reference: surface })).snapshot;
	async function pixels(phase, enabled) {
		const deadline = Date.now() + 5000;
		while ((await read()).definition.enabled !== enabled) {
			assert(Date.now() < deadline, 'Surface authored enable state');
			await delay(25);
		}
		// Allow steady output before the SDK snapshot; this does not qualify transition latency.
		await delay(250);
		// Screen Composite output is premultiplied; an empty layer set clears to transparent.
		await output.observePixels(phase, enabled ? [255, 255, 255, 255] : [0, 0, 0, 0]);
	}
	try {
		await pixels('initial-enabled', true);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		await page
			.getByRole('region', { name: 'Screens', exact: true })
			.getByRole('button', { name: (await read()).label, exact: true })
			.click();
		assert.equal(
			await page
				.locator('.inspector .parameter-inspector')
				.filter({ has: page.getByText('Enabled', { exact: true }) })
				.count(),
			0
		);
		const toggle = page
			.locator('.inspector-header')
			.getByRole('switch', { name: 'Toggle node enabled' });
		await toggle.click();
		await pixels('metadata-disabled', false);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await pixels('undo-enabled', true);
		await screens({
			kind: 'ConfigureSurface',
			reference: surface,
			definition: { ...(await read()).definition, enabled: false }
		});
		await pixels('domain-disabled', false);
		await toggle.click();
		await pixels('metadata-restored', true);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		const report = {
			result: 'pass',
			nativePid,
			adapter: output.adapter,
			surface,
			observations: output.observations,
			errors,
			rejected,
			limits: 'Five settled complete 96x64 SDK pixel snapshots; no transition timing claim'
		};
		await writeFile(path.join(directory, 'report.json'), JSON.stringify(report, null, 2));
		console.log(JSON.stringify(report));
	} finally {
		await output.finish();
	}
}
