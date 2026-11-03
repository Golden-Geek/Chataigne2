// Full source interaction coverage against real manager/Channels/generic Inspector controls.
// Domain operations only inspect committed state; all authoring uses DOM controls.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifySourcesUi({ base, inspect }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'setup';
	const rejected = observeRejectedRequests(page, () => stage);
	const manager = page.locator('section[aria-label="Media Manager"]');
	const channels = page.getByRole('region', { name: 'Channels', exact: true });
	const entries = channels.locator(
		':scope > .node-inspector-children > .node-inspector-children-wrapper > .node-inspector'
	);
	const widgets = screenParameterControls(page);
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	async function wait(read, test) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(20);
		}
	}
	const catalog = (test) => wait(inspect, test);
	const media = (id) => inspect().then((value) => value.media.find((item) => item.id === id));
	const state = (id, test) => wait(() => media(id), test);
	async function selectMedia(id) {
		const item = await media(id);
		assert(item);
		await manager
			.getByRole('treeitem', { name: item.label, exact: true })
			.getByRole('button', { name: item.label, exact: true })
			.click();
		await channels.waitFor();
		return item;
	}
	async function managerSelection() {
		const background = manager.locator('.manager-list-content');
		const rect = await background.boundingBox();
		assert(rect);
		await background.click({ position: { x: rect.width / 2, y: rect.height - 5 } });
		await page.getByRole('button', { name: 'Import Picture', exact: true }).waitFor();
	}
	const entry = (label) => entries.filter({ has: page.getByText(label, { exact: true }) });
	const header = (label) => entry(label).locator(':scope > .node-header > .node-title');
	async function selectChannel(reference) {
		const item = await selectMedia(reference.media);
		const index = item.channels.findIndex(
			(channel) => channel.reference.channel === reference.channel
		);
		assert(index >= 0);
		for (let other = 0; other < item.channels.length; other++) {
			const title = entries.nth(other).locator(':scope > .node-header > .node-title');
			if (other !== index && (await title.getAttribute('aria-checked')) === 'true')
				await title.press('Enter');
		}
		const row = entries.nth(index);
		if ((await row.locator('.parameter-inspector').count()) === 0)
			await header(item.channels[index].label).press('Enter');
		await row.locator('.parameter-inspector').first().waitFor();
		return row;
	}
	async function renameChannel(reference, label) {
		await selectChannel(reference);
		const item = (await media(reference.media)).channels.find(
			(item) => item.reference.channel === reference.channel
		);
		await entry(item.label).locator(':scope > .node-header .title-text').dblclick();
		await channels.locator('input.node-title-input').fill(label);
		await channels.locator('input.node-title-input').press('Enter');
		await state(reference.media, (item) =>
			item.channels.some(
				(channel) => channel.reference.channel === reference.channel && channel.label === label
			)
		);
	}
	async function channelMenu(reference, action) {
		const item = await selectMedia(reference.media);
		const channel = item.channels.find((item) => item.reference.channel === reference.channel);
		const toggle = header(channel.label).getByRole('switch', { name: 'Toggle node enabled' });
		const enabled = await toggle.getAttribute('aria-checked');
		await toggle.click({ button: 'right' });
		assert.equal(
			await toggle.getAttribute('aria-checked'),
			enabled,
			'context menu must not toggle enabled'
		);
		await page.getByRole('menuitem', { name: new RegExp(`^${action}(?:\\s|$)`) }).click();
	}
	async function create(kind, label) {
		const before = await inspect();
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name: kind, exact: true }).click();
		const created = await catalog((value) => value.media.length === before.media.length + 1);
		const item = created.media.find((item) => !before.media.some((old) => old.id === item.id));
		assert(item && item.channels.length === 1);
		await renameChannel(item.channels[0].reference, label);
		return media(item.id);
	}
	async function addChannel(id, label) {
		const before = await selectMedia(id);
		await channels.getByRole('button', { name: 'Add child item', exact: true }).click();
		const next = await state(id, (item) => item.channels.length === before.channels.length + 1);
		const added = next.channels.find(
			(item) => !before.channels.some((old) => old.reference.channel === item.reference.channel)
		);
		await renameChannel(added.reference, label);
		return (await media(id)).channels.find(
			(item) => item.reference.channel === added.reference.channel
		);
	}
	async function size(id, width, height) {
		await selectMedia(id);
		const resolution = widgets.parameter('Resolution');
		const toggle = resolution.getByRole('switch', { name: 'Toggle node enabled' });
		if ((await toggle.getAttribute('aria-checked')) === 'false') await toggle.click();
		await widgets.setNumber('Resolution', width, 0);
		await widgets.setNumber('Resolution', height, 1);
		await widgets.choice('Sampling').selectOption('Nearest');
		await state(id, (item) =>
			item.channels.every(
				(channel) =>
					channel.definition.size.extent?.width === width &&
					channel.definition.size.extent?.height === height &&
					channel.definition.filter === 'Nearest'
			)
		);
	}
	async function importPicture(form, kind, path) {
		await form.getByRole('combobox', { name: /^Source location/ }).selectOption(kind);
		await form.getByLabel('Picture file path', { exact: true }).fill(path);
		await form.getByRole('button', { name: /^(Import Picture|Import and assign)$/ }).click();
	}
	async function sender(reference, value) {
		const row = await selectChannel(reference);
		const input = row
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Spout sender', { exact: true }) })
			.locator('input.string-editor');
		await input.fill(value);
		await input.press('Tab');
	}
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('New', { exact: true }) })
			.click();
		await catalog((value) => value.media.length === 0 && value.assets.length === 0);

		stage = 'Solid source creation, independent color and shared media settings';
		let solid = await create('Solid Color', 'Primary');
		await selectMedia(solid.id);
		await widgets.rename(solid.label, 'Solid sources');
		solid = await state(solid.id, (value) => value.label === 'Solid sources');
		const primary = solid.channels[0];
		const secondary = await addChannel(solid.id, 'Secondary');
		const row = await selectChannel(secondary.reference);
		const color = row
			.locator('.parameter-inspector')
			.filter({ has: page.getByText('Color', { exact: true }) });
		await color.locator('.cp-preview').click();
		for (const [id, value] of [
			['r', '.25'],
			['g', '.5'],
			['b', '.75'],
			['a', '.5']
		]) {
			await color.locator(`#cp-${id}`).fill(value);
			await color.locator(`#cp-${id}`).press('Tab');
		}
		await state(solid.id, (value) =>
			isDeepStrictEqual(
				value.channels.find((channel) => channel.reference.channel === secondary.reference.channel)
					.definition.source.rgba,
				[0.25, 0.5, 0.75, 0.5]
			)
		);
		assert.deepEqual((await media(solid.id)).channels[0], primary);
		await size(solid.id, 320, 180);
		await channelMenu(secondary.reference, 'Move earlier');
		await state(
			solid.id,
			(value) => value.channels[0].reference.channel === secondary.reference.channel
		);
		const beforeDelete = await inspect();
		await channelMenu(secondary.reference, 'Delete');
		await state(solid.id, (value) => value.channels.length === 1);
		const beforeGuard = await inspect();
		const rejectedBefore = rejected.length;
		await channelMenu(primary.reference, 'Delete');
		await wait(
			() => rejected.length,
			(count) => count === rejectedBefore + 1
		);
		assert(
			rejected
				.at(-1)
				.errors.some((error) => /retain at least one channel/.test(error.error_message)),
			JSON.stringify(rejected.at(-1))
		);
		assert.deepEqual(await inspect(), beforeGuard, 'last-channel removal must reject atomically');
		await undo();
		await catalog((value) => isDeepStrictEqual(value, beforeDelete));

		stage = 'Picture import, typed asset relink, unconfigure and picker reselection';
		const picture = await create('Picture', 'Picture A');
		let pictureRow = await selectChannel(picture.channels[0].reference);
		await importPicture(pictureRow.locator('form'), 'project_relative', 'source-ui/a.png');
		const imported = await catalog((value) => value.assets.length === 1);
		const asset = imported.assets[0];
		await state(picture.id, (value) => value.channels[0].definition.source.asset === asset.id);
		await pictureRow.getByRole('button', { name: 'Inspect Picture asset', exact: true }).click();
		const path = widgets.parameter('Project path').locator('input.path-input');
		await path.fill('source-ui/b.png');
		await path.press('Tab');
		await catalog(
			(value) =>
				value.assets[0]?.id === asset.id && value.assets[0].locator.path === 'source-ui/b.png'
		);
		pictureRow = await selectChannel(picture.channels[0].reference);
		await widgets.parameter('Picture asset').locator('button.picker-trigger').click();
		const picker = page.getByRole('dialog', { name: 'Select reference target', exact: true });
		await picker.getByRole('button', { name: 'Clear', exact: true }).click();
		await state(picture.id, (value) => value.channels[0].definition.source.asset === null);
		await widgets.reference('Picture asset', asset.label, asset.label);
		await state(picture.id, (value) => value.channels[0].definition.source.asset === asset.id);

		stage = 'Shared Texture exact typed sender selectors and independent peers';
		const shared = await create('Shared Texture', 'Receiver A');
		await sender(shared.channels[0].reference, 'Exact sender A');
		await state(
			shared.id,
			(value) => value.channels[0].definition.source.sender === 'Exact sender A'
		);
		await size(shared.id, 64, 32);
		const second = await addChannel(shared.id, 'Receiver B');
		await sender(second.reference, 'Exact sender B');
		await state(
			shared.id,
			(value) => value.channels[1].definition.source.sender === 'Exact sender B'
		);
		const peer = (await media(shared.id)).channels[1];
		assert.equal(peer.reference.channel, second.reference.channel);
		await sender(shared.channels[0].reference, 'Changed A');
		await state(shared.id, (value) => value.channels[0].definition.source.sender === 'Changed A');
		await sender(shared.channels[0].reference, '');
		await state(shared.id, (value) => value.channels[0].definition.source.sender === null);
		assert.deepEqual((await media(shared.id)).channels[1], peer);

		stage = 'Generic media deletion, shared asset retention and exact Undo';
		const beforeRemoval = await inspect();
		await selectMedia(picture.id);
		await manager
			.getByRole('button', { name: picture.label, exact: true })
			.click({ button: 'right' });
		await page.getByRole('menuitem', { name: /^Delete(?:\s|$)/ }).click();
		const removed = await catalog((value) => !value.media.some((item) => item.id === picture.id));
		assert.deepEqual(removed.assets, beforeRemoval.assets);
		assert.deepEqual(
			removed.media.find((item) => item.id === shared.id),
			beforeRemoval.media.find((item) => item.id === shared.id)
		);
		await undo();
		await catalog((value) => isDeepStrictEqual(value, beforeRemoval));

		stage = 'Atomic top-level Picture import, Undo and browser reload';
		await managerSelection();
		const form = page
			.locator('.properties form')
			.filter({ has: page.getByRole('button', { name: 'Import Picture', exact: true }) });
		await importPicture(form, 'absolute', 'D:\\source-ui\\missing.png');
		await catalog(
			(value) =>
				value.media.length === beforeRemoval.media.length + 1 &&
				value.assets.length === beforeRemoval.assets.length + 1
		);
		await undo();
		await catalog((value) => isDeepStrictEqual(value, beforeRemoval));
		await page.reload();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await manager.getByRole('button', { name: 'Solid sources', exact: true }).waitFor();
		assert.deepEqual(await inspect(), beforeRemoval);
		assert.equal(await manager.locator('input:not([type=search]),select,fieldset').count(), 0);
		assert.deepEqual(errors, []);
		assert.equal(rejected.length, 1, JSON.stringify(rejected));
		await selectChannel(shared.channels[0].reference);
		await page.screenshot({ path: 'target/mapgyver-media-source-inspector.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'full revised source CRUD/import/relink/reference selection, real typed Channels and shared media settings, independent variants, atomic last-channel guard, media/asset lifetime, Undo and reload through generic Golden UI',
				expectedGuardRejections: 1,
				pageErrors: errors.length
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				page_errors: errors,
				rejected_requests: rejected,
				body: (await page.locator('body').innerText()).slice(-5000)
			})
		);
		await page.screenshot({ path: 'target/mapgyver-media-source-failure.png' });
		throw error;
	} finally {
		await browser.close();
	}
}
