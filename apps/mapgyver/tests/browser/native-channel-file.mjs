import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeNativeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls } from './screen-parameters.mjs';

export async function verifyNativeChannelFile({ page, edit, post, nativePid }) {
	const directory = path.resolve(process.env.MAPGYVER_CHANNEL_FILE_DIRECTORY);
	await mkdir(directory);
	const pageErrors = [];
	page.on('pageerror', (error) => pageErrors.push(String(error)));
	const { rejected, observed } = await observeNativeRejectedRequests(
		page,
		() => 'native channel File'
	);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Picture',
		label: 'Channel files'
	});
	const { reference: peer } = await edit({ kind: 'AddChannel', media: reference.media });
	const inspect = async () => (await edit({ kind: 'InspectManager' })).snapshot;
	const read = async (selected = reference) =>
		(await edit({ kind: 'InspectChannel', reference: selected })).snapshot;
	const binding = async (selected = reference) =>
		(await edit({ kind: 'InspectPictureFile', reference: selected })).binding;
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
	async function select() {
		await page.getByRole('tab').first().click();
		await page.getByRole('button', { name: 'Channel files', exact: true }).click();
	}
	await select();
	const rows = page
		.locator('.channels-section')
		.locator(
			':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
		);
	await wait(
		() => rows.count(),
		(count) => count === 2
	);
	const first = rows.first();
	const second = rows.nth(1);
	async function enter(row, value) {
		const input = row.locator('.channel-file input.path-input');
		await input.fill(value);
		await input.press('Tab');
	}
	for (const row of [first, second]) {
		await row.locator('.unconfigured-file input.path-input').waitFor();
		assert.equal(await row.getByText('Parameters', { exact: true }).count(), 0);
		assert.equal(await row.getByText('Source location', { exact: true }).count(), 0);
		assert.equal(
			await row.getByRole('button', { name: 'Import and assign', exact: true }).count(),
			0
		);
	}
	await enter(first, 'images/first.png');
	const created = await wait(binding, (value) => value?.parameter != null);
	await first.locator('.parameter-inspector input.path-input').waitFor();
	assert.equal(
		Number(await first.locator('.parameter-inspector').getAttribute('data-node-id')),
		created.parameter.node
	);
	assert.equal((await inspect()).assets.length, 1);
	assert.equal((await read(peer)).definition.source.asset, null);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(read, (value) => value.definition.source.asset === null);
	await first.locator('.unconfigured-file input.path-input').waitFor();
	assert.equal((await inspect()).assets.length, 0);
	await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	await wait(binding, (value) => value?.parameter?.uuid === created.parameter.uuid);
	await first.locator('.parameter-inspector input.path-input').waitFor();
	await enter(second, 'images/peer.png');
	const independent = await wait(
		() => binding(peer),
		(value) => value?.parameter != null
	);
	assert.notEqual(independent.asset, created.asset);
	await second.getByRole('button', { name: 'Choose existing Picture asset', exact: true }).click();
	await screenParameterControls(page).reference('Picture asset', 'first.png', 'first.png');
	await wait(
		() => binding(peer),
		(value) => value?.asset === created.asset
	);
	await second.getByRole('button', { name: 'Choose existing Picture asset', exact: true }).click();
	const sharedAddress = String((await binding()).parameter.node);
	await wait(
		() => second.locator('.parameter-inspector').getAttribute('data-node-id'),
		(value) => value === sharedAddress
	);
	await enter(first, 'images/shared.png');
	await wait(
		() => second.locator('.channel-file input.path-input').inputValue(),
		(value) => value === 'images/shared.png'
	);
	const layouts = [];
	for (const row of [first, second]) {
		const measurement = await row.evaluate((element) => {
			const file = element.querySelector('.file-input-row');
			const input = file.querySelector('input').getBoundingClientRect();
			const actions = [...file.querySelectorAll('button')].map((button) =>
				button.getBoundingClientRect()
			);
			const header = element.querySelector(':scope > .node-header');
			return {
				height: element.getBoundingClientRect().height,
				rem: parseFloat(getComputedStyle(document.documentElement).fontSize),
				inputWidth: input.width,
				oneLine: actions.every((button) => Math.abs(button.top - input.top) < 2),
				headerActions: header.querySelectorAll(
					'button[aria-label="Choose existing Picture asset"],button[aria-label="Reload Picture"],button[aria-label="Inspect Picture asset"]'
				).length
			};
		});
		assert(measurement.oneLine && measurement.inputWidth > 0, JSON.stringify(measurement));
		assert(measurement.height <= measurement.rem * 7.5, JSON.stringify(measurement));
		assert.equal(measurement.headerActions, 3);
		layouts.push(measurement);
	}
	for (let cycle = 0; cycle < 3; cycle++) {
		const input = first.locator('.channel-file input.path-input');
		const changed = `images/enter-${cycle}.png`;
		await input.fill(changed);
		await input.press('Enter');
		await wait(
			() => second.locator('.channel-file input.path-input').inputValue(),
			(value) => value === changed
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => second.locator('.channel-file input.path-input').inputValue(),
			(value) => value === 'images/shared.png'
		);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(
			() => second.locator('.channel-file input.path-input').inputValue(),
			(value) => value === changed
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			() => second.locator('.channel-file input.path-input').inputValue(),
			(value) => value === 'images/shared.png'
		);
	}
	const cancelled = first.locator('.channel-file input.path-input');
	await cancelled.fill('../cancelled.png');
	await cancelled.press('Escape');
	assert.equal(await cancelled.inputValue(), 'images/shared.png');
	await enter(second, '');
	await wait(
		() => first.locator('.channel-file input.path-input').inputValue(),
		(value) => value === ''
	);
	assert.equal((await binding()).asset, created.asset);
	assert.equal((await binding(peer)).asset, created.asset);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await wait(
		() => first.locator('.channel-file input.path-input').inputValue(),
		(value) => value === 'images/shared.png'
	);
	await wait(
		() => page.locator('.user-action-activity').getAttribute('aria-hidden'),
		(value) => value === 'true'
	);
	const project = path.join(directory, 'channel-files.mapgyver');
	assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
	assert.equal((await post('/api/ui/project-new', {})).ok, true);
	await post('/api/ui/project-load', { path: project });
	await select();
	await wait(
		() => first.locator('.channel-file input.path-input').inputValue(),
		(value) => value === 'images/shared.png'
	);
	assert.equal((await binding()).parameter.uuid, created.parameter.uuid);
	await wait(
		() => page.locator('.user-action-activity').getAttribute('aria-hidden'),
		(value) => value === 'true'
	);
	const clientErrors = await page.locator('.toast.error .toast-message').allTextContents();
	assert.deepEqual(clientErrors, []);
	assert.deepEqual(pageErrors, []);
	assert.deepEqual(rejected, []);
	assert(observed.requests > 0 && observed.receipts > 0);
	await page.waitForFunction(() => {
		const inputs = [
			...document.querySelectorAll('.channels-section .channel-file input.path-input')
		];
		return (
			inputs.length === 2 &&
			inputs.every((input) => {
				const box = input.getBoundingClientRect(),
					x = box.x + box.width / 2,
					y = box.y + box.height / 2;
				if (box.width <= 0 || box.height <= 0) return false;
				for (let element = input; element; element = element.parentElement) {
					const style = getComputedStyle(element),
						bounds = element.getBoundingClientRect();
					if (
						Number(style.opacity) < 1 ||
						style.visibility !== 'visible' ||
						style.display === 'none'
					)
						return false;
					if (
						['hidden', 'clip', 'auto', 'scroll'].includes(style.overflowY) &&
						(y < bounds.top || y > bounds.bottom)
					)
						return false;
					if (
						['hidden', 'clip', 'auto', 'scroll'].includes(style.overflowX) &&
						(x < bounds.left || x > bounds.right)
					)
						return false;
				}
				return true;
			})
		);
	});
	await page.evaluate(
		() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)))
	);
	await page.screenshot({ path: path.join(directory, 'channel-file.png') });
	const receipt = {
		nativePid,
		oneFilePerChannel: true,
		noImportFormsOrNestedParameters: true,
		materializationUndoRedo: true,
		independentChannels: true,
		existingAssetPicker: true,
		sharedRealParameter: true,
		layouts,
		enterUndoRedoCycles: 3,
		escapeCancelsDraft: true,
		clearPreservesBindings: true,
		saveNewLoad: true,
		observed,
		rejected,
		pageErrors,
		clientErrors,
		finalActivityIdle: true,
		settledVisibleFiles: true,
		limits: 'Authoring and native Inspector only; no decoded/GPU pixel or cadence qualification.'
	};
	await writeFile(path.join(directory, 'channel-file.json'), JSON.stringify(receipt, null, 2));
	console.log(JSON.stringify(receipt));
}
