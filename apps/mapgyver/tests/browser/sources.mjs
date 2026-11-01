// M04's complete source-authoring matrix through the Golden tree and shared Inspector.
// All authoring uses actual DOM controls; public backend operations only inspect committed state.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { isDeepStrictEqual } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifySourcesUi({ base, inspect }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	const manager = page.locator('section[aria-label="Media Manager"]');
	const properties = page.locator('.properties');
	async function wait(read, accept) {
		const deadline = Date.now() + 7000;
		for (;;) {
			const value = await read();
			if (accept(value)) return value;
			assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
			await delay(25);
		}
	}
	const catalog = (accept) => wait(inspect, accept);
	const state = (id, accept) =>
		catalog((value) => accept(value.media.find((media) => media.id === id)));
	async function menu(name) {
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await page.getByRole('menuitem', { name, exact: true }).click();
	}
	async function rename(label, value) {
		const field = properties.getByRole('textbox', { name: label, exact: true });
		await field.fill(value);
		await field.press('Enter');
	}
	async function selectChannel(label) {
		await manager.getByRole('button', { name: label, exact: true }).click();
		await wait(
			() => properties.getByRole('textbox', { name: 'Channel name', exact: true }).inputValue(),
			(value) => value === label
		);
		await wait(
			() =>
				properties.getByRole('button', { name: 'Apply channel settings', exact: true }).isEnabled(),
			Boolean
		);
	}
	async function selectMedia(label) {
		await manager.getByRole('button', { name: label, exact: true }).click();
		await wait(
			() => properties.getByRole('textbox', { name: 'Media name', exact: true }).inputValue(),
			(value) => value === label
		);
	}
	async function create(name, channelName) {
		const before = await inspect();
		await menu(name);
		const created = await catalog((value) => value.media.length === before.media.length + 1);
		const media = created.media.find((item) => !before.media.some((old) => old.id === item.id));
		await wait(
			() => properties.getByRole('textbox', { name: 'Channel name', exact: true }).inputValue(),
			(value) => value === media.channels[0].label
		);
		await rename('Channel name', channelName);
		await state(media.id, (value) => value?.channels[0].label === channelName);
		return media;
	}
	async function addChannel(media, label) {
		await selectMedia(media.label);
		const before = (await inspect()).media.find((item) => item.id === media.id);
		await properties.getByRole('button', { name: 'Add channel', exact: true }).click();
		const next = await state(
			media.id,
			(value) => value?.channels.length === before.channels.length + 1
		);
		const added = next.media
			.find((item) => item.id === media.id)
			.channels.find(
				(item) => !before.channels.some((old) => old.reference.channel === item.reference.channel)
			);
		await selectChannel(added.label);
		await rename('Channel name', label);
		await state(media.id, (value) =>
			value?.channels.some(
				(item) => item.reference.channel === added.reference.channel && item.label === label
			)
		);
		return added;
	}
	async function apply() {
		await properties.getByRole('button', { name: 'Apply channel settings', exact: true }).click();
	}
	async function size(width, height) {
		await properties.getByLabel('Fixed output size', { exact: true }).check();
		await properties.getByLabel('Width (texels)', { exact: true }).fill(String(width));
		await properties.getByLabel('Height (texels)', { exact: true }).fill(String(height));
		await properties.getByRole('combobox', { name: /^Sampling/ }).selectOption('Nearest');
	}
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
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
		await manager.getByText('No media', { exact: true }).waitFor();

		stage = 'Solid creation, rename, independent channel settings and history';
		let solid = await create('Solid Color', 'Primary');
		await selectMedia(solid.label);
		await rename('Media name', 'Solid sources');
		await state(solid.id, (value) => value?.label === 'Solid sources');
		solid = (await inspect()).media.find((media) => media.id === solid.id);
		const primary = solid.channels[0];
		const secondary = await addChannel(solid, 'Secondary');
		for (const [label, value] of Object.entries({
			Red: '.25',
			Green: '.5',
			Blue: '.75',
			Alpha: '.5'
		}))
			await properties.getByRole('spinbutton', { name: label, exact: true }).fill(value);
		await size(320, 180);
		await apply();
		await state(solid.id, (value) =>
			value?.channels.some(
				(channel) =>
					channel.reference.channel === secondary.reference.channel &&
					isDeepStrictEqual(channel.definition, {
						source: { kind: 'SolidColor', rgba: [0.25, 0.5, 0.75, 0.5] },
						size: { kind: 'Fixed', extent: { width: 320, height: 180 } },
						filter: 'Nearest'
					})
			)
		);
		assert.deepEqual(
			(await inspect()).media.find((media) => media.id === solid.id).channels[0],
			primary
		);
		await properties.getByRole('button', { name: 'Move channel earlier', exact: true }).click();
		let beforeDelete = await state(
			solid.id,
			(value) => value?.channels[0].reference.channel === secondary.reference.channel
		);
		await properties.getByRole('button', { name: 'Delete channel', exact: true }).click();
		await state(solid.id, (value) => value?.channels.length === 1);
		await selectChannel('Primary');
		assert(
			await properties.getByRole('button', { name: 'Delete channel', exact: true }).isDisabled()
		);
		await undo();
		await catalog((value) => isDeepStrictEqual(value, beforeDelete));

		stage = 'Picture import, relink, unconfigure and catalog reselection';
		const picture = await create('Picture', 'Picture A');
		const importer = properties.getByRole('region', {
			name: 'Import a Picture for this channel',
			exact: true
		});
		await importer
			.getByRole('combobox', { name: /^Source location/ })
			.selectOption('project_relative');
		await importer.getByLabel('Picture file path', { exact: true }).fill('source-ui/a.png');
		await importer.getByRole('button', { name: 'Import and assign', exact: true }).click();
		const imported = await catalog((value) => value.assets.length === 1);
		const asset = imported.assets[0];
		await state(picture.id, (value) => value?.channels[0].definition.source.asset === asset.id);
		const relink = properties.getByRole('region', {
			name: 'Relink selected Picture asset',
			exact: true
		});
		await relink.getByLabel('Picture file path', { exact: true }).fill('source-ui/b.png');
		await relink.getByRole('button', { name: 'Relink asset', exact: true }).click();
		await catalog(
			(value) =>
				value.assets[0]?.id === asset.id && value.assets[0].locator.path === 'source-ui/b.png'
		);
		await properties.getByRole('combobox', { name: /^Picture asset/ }).selectOption('');
		await apply();
		await state(picture.id, (value) => value?.channels[0].definition.source.asset === null);
		await properties.getByRole('combobox', { name: /^Picture asset/ }).selectOption(asset.id);
		await apply();
		await state(picture.id, (value) => value?.channels[0].definition.source.asset === asset.id);

		stage = 'Shared Texture exact selectors, admitted layout and independent peers';
		const shared = await create('Shared Texture', 'Receiver A');
		await properties.getByLabel('Spout sender name', { exact: true }).fill('Exact sender A');
		await properties.getByLabel('Maximum width (texels)', { exact: true }).fill('4');
		await properties.getByLabel('Maximum height (texels)', { exact: true }).fill('4');
		await properties.getByRole('combobox', { name: /^Storage/ }).selectOption('Rgba8Unorm');
		await properties.getByRole('combobox', { name: /^Color encoding/ }).selectOption('Linear');
		await properties.getByRole('combobox', { name: /^Source alpha/ }).selectOption('Premultiplied');
		await size(64, 32);
		await apply();
		const expected = {
			source: {
				kind: 'Spout',
				sender: 'Exact sender A',
				maximum: {
					extent: { width: 4, height: 4 },
					format: 'Rgba8Unorm',
					color_encoding: 'Linear',
					alpha_mode: 'Premultiplied'
				}
			},
			size: { kind: 'Fixed', extent: { width: 64, height: 32 } },
			filter: 'Nearest'
		};
		await state(shared.id, (value) => isDeepStrictEqual(value?.channels[0].definition, expected));
		const second = await addChannel(shared, 'Receiver B');
		await properties.getByLabel('Spout sender name', { exact: true }).fill('Exact sender B');
		await apply();
		await state(
			shared.id,
			(value) => value?.channels[1].definition.source.sender === 'Exact sender B'
		);
		const peer = (await inspect()).media.find((media) => media.id === shared.id).channels[1];
		assert.equal(peer.reference.channel, second.reference.channel);
		await selectChannel('Receiver A');
		await properties.getByLabel('Spout sender name', { exact: true }).fill('Changed A');
		await apply();
		await state(shared.id, (value) => value?.channels[0].definition.source.sender === 'Changed A');
		await properties.getByLabel('Spout sender name', { exact: true }).fill('');
		await apply();
		await state(shared.id, (value) => value?.channels[0].definition.source.sender === null);
		assert.deepEqual(
			(await inspect()).media.find((media) => media.id === shared.id).channels[1],
			peer
		);

		stage = 'Media deletion, shared asset retention and exact Undo';
		const beforeRemoval = await inspect();
		await selectMedia(picture.label);
		await properties.getByRole('button', { name: 'Delete media', exact: true }).click();
		const removed = await catalog((value) => !value.media.some((media) => media.id === picture.id));
		assert.deepEqual(removed.assets, beforeRemoval.assets);
		assert.deepEqual(
			removed.media.find((media) => media.id === shared.id),
			beforeRemoval.media.find((media) => media.id === shared.id)
		);
		await undo();
		await catalog((value) => isDeepStrictEqual(value, beforeRemoval));

		stage = 'Atomic top-level Picture import, Undo and browser reload';
		await menu('Import Picture…');
		await properties.getByRole('combobox', { name: /^Source location/ }).selectOption('absolute');
		await properties
			.getByLabel('Picture file path', { exact: true })
			.fill('D:\\source-ui\\missing.png');
		await properties.getByRole('button', { name: 'Import Picture', exact: true }).click();
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
		assert.deepEqual(rejected, []);
		await selectChannel('Receiver A');
		await page.screenshot({ path: 'target/mapgyver-media-source-inspector.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'M04 complete Golden tree/Inspector source CRUD, independent channel settings, Picture import/relink, Spout selectors, guards, Undo and browser reload',
				browser: browser.version(),
				page_errors: errors.length,
				rejected_requests: rejected.length,
				native_preview: 'Unavailable in browser; no native qualification inferred.'
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
