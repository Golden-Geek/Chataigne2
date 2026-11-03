// Actual File New during held input and File Open review upload, using current Golden widgets.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { screenParameterControls } from './screen-parameters.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyCompositionProjectUi({ base, edit, inspectMedia }) {
	const project = path.resolve('target/m06-editor-review/CompositionReview.mapgyver');
	const review = JSON.parse(
		await readFile(path.resolve('target/m06-editor-review/review.json'), 'utf8')
	);
	const hash = async () =>
		createHash('sha256')
			.update(await readFile(project))
			.digest('hex');
	const originalHash = await hash();
	assert.equal(originalHash, review.project_sha256);
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1600, height: 1100 } });
	page.setDefaultTimeout(6000);
	let stage = 'startup';
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	const widgets = screenParameterControls(page);
	const manager = page.locator('section[aria-label="Media Manager"]');
	const panel = page.locator('section[aria-label="Composition"]');
	const inspect = async (reference) =>
		(await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } })).snapshot;
	const tab = (name) => page.getByRole('tab').filter({ hasText: new RegExp('^' + name + '$') });
	const undo = () => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	const redo = () => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
	const menu = (label) =>
		page.getByRole('menuitem').filter({ has: page.getByText(label, { exact: true }) });
	async function wait(read, test) {
		const deadline = Date.now() + 6000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, stage + ': ' + JSON.stringify(value));
			await delay(20);
		}
	}
	async function select(label) {
		await tab('Media Manager').click();
		await manager
			.getByRole('treeitem', { name: label, exact: true })
			.getByRole('button', { name: label, exact: true })
			.click();
	}
	async function create() {
		const before = await inspectMedia();
		await tab('Media Manager').click();
		await manager.getByRole('button', { name: 'Add child item', exact: true }).first().click();
		await menu('Composition').click();
		const after = await wait(inspectMedia, (value) =>
			value.media.some(
				(item) =>
					item.media_kind === 'Composition' && !before.media.some((old) => old.id === item.id)
			)
		);
		const root = after.media.find(
			(item) => item.media_kind === 'Composition' && !before.media.some((old) => old.id === item.id)
		);
		await select(root.label);
		await tab('Composition').click();
		await panel
			.getByRole('group', { name: 'Composition transform controls', exact: true })
			.waitFor();
		return root.channels[0].reference;
	}
	async function newProject(shortcut) {
		if (shortcut) await page.keyboard.press('Control+n');
		else {
			await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
			await menu('New').click();
		}
		await wait(inspectMedia, (value) => value.media.length === 0 && value.assets.length === 0);
		await wait(
			() =>
				panel.getByRole('group', { name: 'Composition transform controls', exact: true }).count(),
			(value) => value === 0
		);
		await wait(() => page.getByTitle('Ctrl/Cmd+Z', { exact: true }).isDisabled(), Boolean);
		await wait(
			() => page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).isDisabled(),
			Boolean
		);
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
	}
	async function canvas(width, height) {
		const resolution = widgets.parameter('Resolution');
		if (
			(await resolution
				.getByRole('switch', { name: 'Toggle node enabled' })
				.getAttribute('aria-checked')) === 'false'
		)
			await resolution.getByRole('switch', { name: 'Toggle node enabled' }).click();
		await widgets.setNumber('Resolution', width, 0);
		await widgets.setNumber('Resolution', height, 1);
		await wait(
			() =>
				panel
					.getByRole('group', { name: 'Composition transform controls', exact: true })
					.getAttribute('viewBox'),
			(value) => value === '0 0 ' + width + ' ' + height
		);
		await wait(
			() => panel.getAttribute('data-composition-busy'),
			(value) => value === 'false'
		);
	}
	try {
		await page.goto(base);
		const media = await inspectMedia(),
			solid = media.media.find((item) => item.media_kind === 'SolidColor');
		const first = await create();
		await canvas(400, 300);
		const drop = panel.locator('[data-composition-drop-target]');
		const source = manager
			.getByRole('treeitem', { name: solid.label, exact: true })
			.first()
			.getByRole('button', { name: solid.label, exact: true });
		const transfer = await page.evaluateHandle(() => new DataTransfer());
		try {
			await source.dispatchEvent('dragstart', { dataTransfer: transfer });
			const box = await drop.boundingBox();
			assert(box);
			const event = {
				dataTransfer: transfer,
				clientX: box.x + box.width / 2,
				clientY: box.y + box.height / 2
			};
			await wait(
				async () => {
					await drop.dispatchEvent('dragover', event);
					return drop.locator('.drop-hint').count();
				},
				(count) => count === 1
			);
			await drop.dispatchEvent('drop', event);
			await source.dispatchEvent('dragend', { dataTransfer: transfer });
		} finally {
			await transfer.dispose();
		}
		const initial = await wait(
			() => inspect(first),
			(value) => value.instances.length === 1
		);
		assert.equal(
			initial.instances[0].definition.source.channel,
			solid.channels[0].reference.channel
		);
		stage = 'actual New during accepted drag';
		await wait(
			() => panel.getAttribute('data-composition-loading'),
			(value) => value === 'false'
		);
		const svg = panel.getByRole('group', { name: 'Composition transform controls', exact: true });
		const pivot = await panel.locator('[data-instance-handle="Translate"]').boundingBox(),
			rect = await svg.boundingBox();
		assert(pivot && rect);
		await page.mouse.move(pivot.x + pivot.width / 2, pivot.y + pivot.height / 2);
		await page.mouse.down();
		await page.mouse.move(
			pivot.x + pivot.width / 2 + (20 * rect.width) / 400,
			pivot.y + pivot.height / 2 + (10 * rect.height) / 300,
			{ steps: 12 }
		);
		await wait(
			() => inspect(first),
			(value) =>
				Math.abs(
					value.instances[0].definition.transform.position[0] -
						initial.instances[0].definition.transform.position[0] -
						20
				) < 2
		);
		await newProject(true);
		await page.mouse.move(rect.x + rect.width * 0.8, rect.y + rect.height * 0.8, { steps: 8 });
		await page.mouse.up();
		assert.deepEqual(await inspectMedia(), { assets: [], media: [] });
		stage = 'fresh generic canvas history';
		const fresh = await create();
		assert.notDeepEqual(fresh, first);
		await canvas(512, 288);
		const resized = await wait(
			() => inspect(fresh),
			(value) => value.canvas.width === 512 && value.canvas.height === 288
		);
		await undo();
		await wait(
			() => inspect(fresh),
			(value) => value.canvas.width === 512 && value.canvas.height === 1080
		);
		await redo();
		await wait(
			() => inspect(fresh),
			(value) => isDeepStrictEqual(value, resized)
		);
		stage = 'actual File Open upload';
		await page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		await menu('Open').hover();
		const choosing = page.waitForEvent('filechooser');
		await menu('Load From...').click();
		const uploadName = 'CompositionReview-' + randomUUID() + '.mapgyver';
		await (
			await choosing
		).setFiles({
			name: uploadName,
			mimeType: 'application/octet-stream',
			buffer: await readFile(project)
		});
		await wait(inspectMedia, (value) => value.media.some((item) => item.id === review.root.media));
		const loaded = await inspect(review.root);
		assert.deepEqual(
			loaded.instances.map((item) => item.reference),
			review.instances
		);
		assert.deepEqual(loaded.canvas, { width: 320, height: 240 });
		await select('Review Composition');
		const root = manager.getByRole('treeitem', { name: 'Review Composition', exact: true });
		if ((await root.getAttribute('aria-expanded')) === 'false') {
			await root.focus();
			await root.press('ArrowRight');
		}
		await select(loaded.instances[0].label);
		stage = 'loaded generic rename/history';
		await widgets.rename(loaded.instances[0].label, 'After upload');
		const renamed = await wait(
			() => inspect(review.root),
			(value) => value.instances[0].label === 'After upload'
		);
		await undo();
		await wait(
			() => inspect(review.root),
			(value) => isDeepStrictEqual(value, loaded)
		);
		await redo();
		await wait(
			() => inspect(review.root),
			(value) => isDeepStrictEqual(value, renamed)
		);
		await newProject(false);
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		assert.equal(await hash(), originalHash);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual New during accepted held pointer, fresh generic canvas/history, File Open review upload/stable IDs, generic child rename/history, final File New',
				review_sha256: originalHash,
				pageErrors: errors,
				rejected
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				stage,
				errors,
				rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-3500)
			})
		);
		await page
			.screenshot({ path: 'target/m06-composition-project-ui-failure.png' })
			.catch(() => {});
		throw error;
	} finally {
		await browser.close();
	}
}
