import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyOwnedMediaUi({ base, edit, post, directory, inspectMedia }) {
	await post('/api/ui/project-new', {});
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Owned media source'
	});
	await edit({ kind: 'AddChannel', media: source.media, label: 'Second variant' });
	const { reference: owner } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Owned media canvas'
	});
	const inspect = async () =>
		(await edit({ kind: 'Composition', reference: owner, edit: { kind: 'Inspect' } })).snapshot;
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'owned media');
	async function wait(read, test) {
		const deadline = Date.now() + 6000;
		while (true) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(20);
		}
	}
	const parameter = (label) =>
		page.locator('.parameter-inspector').filter({ has: page.getByText(label, { exact: true }) });
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		const manager = page.locator('section[aria-label="Media Manager"]');
		const canvas = page.locator('section[aria-label="Composition"] .source-drop-zone');
		await canvas.locator('svg[aria-label="Composition transform controls"]').waitFor();
		async function drop(copy, fraction) {
			const sourceLabel = manager
				.getByRole('treeitem', { name: 'Owned media source', exact: true })
				.first()
				.getByRole('button', { name: 'Owned media source', exact: true });
			const transfer = await page.evaluateHandle(() => new DataTransfer());
			try {
				await sourceLabel.dispatchEvent('dragstart', { dataTransfer: transfer });
				const bounds = await canvas.boundingBox();
				assert(bounds);
				const event = {
					dataTransfer: transfer,
					altKey: copy,
					clientX: bounds.x + bounds.width * fraction,
					clientY: bounds.y + bounds.height * fraction
				};
				await wait(
					async () => {
						await canvas.dispatchEvent('dragover', event);
						return canvas.locator('.drop-hint').count();
					},
					(count) => count === 1
				);
				await canvas.dispatchEvent('drop', event);
				await sourceLabel.dispatchEvent('dragend', { dataTransfer: transfer });
			} finally {
				await transfer.dispose();
			}
		}
		await drop(false, 0.2);
		const initial = await wait(inspect, (value) => value.instances.length === 1);
		const alias = initial.instances[0].reference.instance;
		await parameter('Target').waitFor();
		await parameter('Position').waitFor();
		await drop(true, 0.8);
		const copied = await wait(inspect, (value) => value.instances.length === 2);
		const independent = copied.instances[1].reference.instance;
		const media = (await inspectMedia()).media;
		assert.equal(media.find((item) => item.id === alias).media_kind, 'Reference');
		const owned = media.find((item) => item.id === independent);
		assert.equal(owned.media_kind, 'SolidColor');
		assert.equal(owned.channels.length, 2);
		assert.notEqual(owned.channels[0].reference.channel, source.channel);
		await parameter('Target').waitFor({ state: 'detached' });
		await parameter('Position').waitFor();
		const position = parameter('Position').locator('input.number-field').first();
		await position.fill('123');
		await position.press('Enter');
		await wait(inspect, (value) => value.instances[1].definition.transform.position[0] === 123);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(
			inspect,
			(value) =>
				value.instances[1].definition.transform.position[0] ===
				copied.instances[1].definition.transform.position[0]
		);
		const rotation = parameter('Rotation').locator('input.number-field');
		await rotation.fill('90');
		await rotation.press('Enter');
		const rotated = await wait(
			inspect,
			(value) => value.instances[1].definition.transform.rotation_degrees === 90
		);
		assert.deepEqual(
			rotated.instances[0],
			copied.instances[0],
			'copied child rotation is independent'
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait(inspect, (value) => value.instances[1].definition.transform.rotation_degrees === 0);
		await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
		await wait(inspect, (value) => value.instances[1].definition.transform.rotation_degrees === 90);
		const parent = manager.getByRole('treeitem', { name: 'Owned media canvas', exact: true });
		if ((await parent.getAttribute('aria-expanded')) === 'false') {
			await parent.focus();
			await parent.press('ArrowRight');
		}
		const aliasRow = manager.getByRole('treeitem', {
			name: initial.instances[0].label,
			exact: true
		});
		await aliasRow.waitFor();
		assert.equal(
			Number(await aliasRow.getAttribute('aria-level')),
			Number(await parent.getAttribute('aria-level')) + 1
		);
		await aliasRow.getByRole('button', { name: initial.instances[0].label, exact: true }).click();
		await edit({ kind: 'RemoveMedia', media: source.media });
		await page
			.getByRole('region', { name: 'Channels', exact: true })
			.getByText('Target unavailable.', { exact: true })
			.waitFor();
		assert.equal(
			(await inspectMedia()).media.find((item) => item.id === independent).channels.length,
			2
		);
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await page.locator('.reference-channels li').first().waitFor();
		const before = await inspect();
		const savedMedia = await inspectMedia();
		const project = path.join(directory, 'OwnedMedia.mapgyver');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		await page.waitForFunction(
			() => document.querySelectorAll('.reference-channels li').length === 0
		);
		const loaded = await post('/api/ui/project-load', { path: project });
		assert.equal(path.basename(loaded.path), 'OwnedMedia.mapgyver');
		assert.equal(loaded.recovery ?? null, null);
		assert.deepEqual(await inspect(), before);
		assert.deepEqual(await inspectMedia(), savedMedia);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled default Reference and Alt-copy drops, actual child outliner/typed Placement, independent variants, grouped history, missing recovery and project roundtrip'
			})
		);
	} finally {
		await browser.close();
	}
}
