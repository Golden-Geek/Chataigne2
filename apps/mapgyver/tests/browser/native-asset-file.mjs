// One authoritative typed File, exercised through the actual native generic Inspector.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';

export async function verifyNativeAssetFile({ page, edit, post, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_ASSET_FILE_DIRECTORY);
	await mkdir(directory);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const { rejected, observed } = await observeNativeRejectedRequests(
		page,
		() => 'native asset File'
	);
	const original = { kind: 'project_relative', path: 'images/source.png' };
	const { reference, asset } = await edit({
		kind: 'ImportPicture',
		locator: original,
		target: null,
		label: 'File assets'
	});
	const { reference: peer } = await edit({ kind: 'AddChannel', media: reference.media });
	await edit({
		kind: 'ConfigureChannel',
		reference: peer,
		source: { kind: 'Picture', asset }
	});
	const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	const read = async () => (await inspect()).assets.find((item) => item.id === asset);
	async function wait(read, predicate) {
		const deadline = Date.now() + 8000;
		while (true) {
			const value = await read();
			if (predicate(value)) return value;
			if (Date.now() >= deadline) {
				const failure = {
					value,
					rejected,
					observed,
					toasts: await page.locator('.toast').allTextContents()
				};
				await writeFile(path.join(directory, 'failure.json'), JSON.stringify(failure, null, 2));
				assert.fail(JSON.stringify(failure));
			}
			await delay(30);
		}
	}
	async function selectAsset() {
		await page.getByRole('tab').first().click();
		await page.getByRole('button', { name: 'File assets', exact: true }).click();
		await page.getByRole('button', { name: 'Inspect Picture asset', exact: true }).first().click();
		await wait(
			() => page.locator('.parameter-inspector').count(),
			(count) => count === 1
		);
	}
	await selectAsset();
	const file = page
		.locator('.parameter-inspector')
		.filter({ has: page.getByText('File', { exact: true }) });
	const input = file.locator('input.path-input');
	await wait(
		() => input.inputValue(),
		(value) => value === original.path
	);
	const parameterId = await file.getAttribute('data-node-id');
	assert.equal(await file.getByRole('button', { name: 'Browse', exact: true }).isEnabled(), true);
	for (const label of ['Location', 'Project path', 'Absolute path'])
		assert.equal(await page.getByText(label, { exact: true }).count(), 0);
	const absolutePath = path.join(directory, 'source-may-arrive-later.png');
	const absolute = { kind: 'absolute', path: absolutePath };
	async function enter(value) {
		await input.fill(value);
		await input.press('Tab');
	}
	await enter(absolutePath);
	await wait(read, (value) => JSON.stringify(value.locator) === JSON.stringify(absolute));
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (value) => JSON.stringify(value.locator) === JSON.stringify(original));
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(read, (value) => JSON.stringify(value.locator) === JSON.stringify(absolute));
	await enter('');
	await wait(read, (value) => value.locator === null);
	const cleared = await inspect();
	assert.equal(cleared.assets.length, 1);
	assert(
		cleared.media
			.find((item) => item.id === reference.media)
			.channels.every((item) => item.definition.source.asset === asset)
	);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (value) => JSON.stringify(value.locator) === JSON.stringify(absolute));
	const relinked = { kind: 'project_relative', path: 'images/relinked.png' };
	await edit({ kind: 'RelinkAsset', asset, locator: relinked });
	await wait(
		() => input.inputValue(),
		(value) => value === relinked.path
	);
	assert.equal(await file.getAttribute('data-node-id'), parameterId);
	const beforeReload = await inspect();
	await page.getByRole('button', { name: 'Reload asset', exact: true }).click();
	assert.deepEqual(await inspect(), beforeReload);
	await wait(
		() => page.locator('.user-action-activity').getAttribute('aria-hidden'),
		(value) => value === 'true'
	);
	assert.deepEqual(await page.locator('.toast.error .toast-message').allTextContents(), []);
	const project = path.join(directory, 'asset-file.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await post('/api/ui/project-load', { path: project });
	await wait(read, (value) => JSON.stringify(value?.locator) === JSON.stringify(relinked));
	await selectAsset();
	await wait(
		() => input.inputValue(),
		(value) => value === relinked.path
	);
	await wait(
		() => page.locator('.user-action-activity').getAttribute('aria-hidden'),
		(value) => value === 'true'
	);
	const clientErrors = await page.locator('.toast.error .toast-message').allTextContents();
	assert.deepEqual(clientErrors, []);
	await page.screenshot({ path: path.join(directory, 'asset-file.png') });
	assert.deepEqual(errors, []);
	assert.deepEqual(rejected, []);
	assert(observed.requests > 0 && observed.receipts > 0, 'Native transport receipts were observed');
	const receipt = {
		nativePid,
		oneTypedFile: true,
		backendLocationInference: true,
		sharedAssetIdentity: true,
		clearPreservesBindings: true,
		editUndoRedo: true,
		relinkRetainsParameter: true,
		reloadPreservesAuthoring: true,
		saveNewLoad: true,
		browseAvailable: true,
		pageErrors: errors,
		rejectedRequests: rejected,
		observed,
		clientErrors,
		finalActivityIdle: true,
		limits: 'Catalog asset Inspector only; no channel File-row or decoded/native pixel acceptance.'
	};
	await writeFile(path.join(directory, 'asset-file.json'), JSON.stringify(receipt, null, 2));
	console.log(JSON.stringify(receipt));
}
