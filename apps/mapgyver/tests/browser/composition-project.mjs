// Real New during a drag and browser review upload. Backend requests only inspect committed state.
import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { isDeepStrictEqual } from 'node:util';
import { observeRejectedRequests } from './intent-observer.mjs';
import { selectMediaChannel } from './media-tree.mjs';

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
	const uploadName = `CompositionReview-${randomUUID()}.mapgyver`;
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	let stage = 'startup';
	const rejected = observeRejectedRequests(page, () => stage);
	try {
		async function wait(read, predicate) {
			const deadline = Date.now() + 5000;
			while (true) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		const inspect = async (reference) =>
			(await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } })).snapshot;
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition$/ })
			.click();
		const panel = page.locator('section[aria-label="Composition"]');
		const properties = page.locator('.properties');
		const add = panel.getByRole('button', { name: 'Add Composition', exact: true });
		const undo = page.getByTitle('Ctrl/Cmd+Z', { exact: true });
		const redo = page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true });
		const menuItem = (label) =>
			page.getByRole('menuitem').filter({ has: page.getByText(label, { exact: true }) });
		const openFile = () =>
			page.getByRole('button', { name: 'Open File menu', exact: true }).click();
		async function newProject(shortcut = false) {
			if (shortcut) await page.keyboard.press('Control+n');
			else {
				await openFile();
				await menuItem('New').click();
			}
			await wait(inspectMedia, (value) => value.media.length === 0 && value.assets.length === 0);
			await wait(() => add.isEnabled(), Boolean);
			await wait(
				() =>
					panel.getByRole('group', { name: 'Composition transform controls', exact: true }).count(),
				(value) => value === 0
			);
			await wait(
				() => properties.getByRole('region', { name: 'Selected instance', exact: true }).count(),
				(value) => value === 0
			);
			await wait(() => undo.isDisabled(), Boolean);
			await wait(() => redo.isDisabled(), Boolean);
			await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		}
		async function create() {
			const before = await inspectMedia();
			await add.click();
			const catalog = await wait(inspectMedia, (value) =>
				value.media.some(
					(entry) =>
						entry.media_kind === 'Composition' && !before.media.some((old) => old.id === entry.id)
				)
			);
			const root = catalog.media.find(
				(entry) =>
					entry.media_kind === 'Composition' && !before.media.some((old) => old.id === entry.id)
			);
			return root.channels[0].reference;
		}
		async function canvas(width, height) {
			await properties.getByLabel('Canvas width', { exact: true }).fill(String(width));
			await properties.getByLabel('Canvas height', { exact: true }).fill(String(height));
			await properties.getByRole('button', { name: 'Apply channel settings', exact: true }).click();
		}
		stage = 'accepted drag replaced through actual New shortcut';
		const media = await inspectMedia();
		const solid = media.media.find((entry) => entry.media_kind === 'SolidColor');
		const first = await create();
		await canvas(400, 300);
		await wait(
			() => inspect(first),
			(value) => value.canvas.width === 400 && value.canvas.height === 300
		);
		const source = properties
			.locator('form')
			.filter({ has: page.getByRole('button', { name: 'Add instance', exact: true }) });
		await source
			.getByLabel('Source channel', { exact: true })
			.selectOption({ label: `${solid.label} / ${solid.channels[0].label}` });
		await source.getByRole('button', { name: 'Add instance', exact: true }).click();
		const initial = await wait(
			() => inspect(first),
			(value) => value.instances.length === 1
		);
		await wait(() => add.isEnabled(), Boolean);
		const svg = panel.getByRole('group', { name: 'Composition transform controls', exact: true });
		await svg.scrollIntoViewIfNeeded();
		const rect = await svg.boundingBox(),
			pivot = await svg.locator('circle.selected-pivot').boundingBox();
		assert(rect && pivot);
		await page.mouse.move(pivot.x + pivot.width / 2, pivot.y + pivot.height / 2);
		await page.mouse.down();
		await page.mouse.move(
			pivot.x + pivot.width / 2 + (20 * rect.width) / 400,
			pivot.y + pivot.height / 2 + (10 * rect.height) / 300,
			{ steps: 12 }
		);
		const dragged = await wait(
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
		assert.deepEqual(
			await inspectMedia(),
			{ assets: [], media: [] },
			'held old pointer cannot author into new project'
		);
		stage = 'fresh Composition controls and history';
		const fresh = await create();
		assert.notDeepEqual(fresh, first);
		const beforeCanvas = await inspect(fresh);
		assert.deepEqual(beforeCanvas.instances, []);
		await canvas(512, 288);
		const afterCanvas = await wait(
			() => inspect(fresh),
			(value) => value.canvas.width === 512 && value.canvas.height === 288
		);
		await undo.click();
		await wait(
			() => inspect(fresh),
			(value) => isDeepStrictEqual(value, beforeCanvas)
		);
		await redo.click();
		await wait(
			() => inspect(fresh),
			(value) => isDeepStrictEqual(value, afterCanvas)
		);
		stage = 'actual File Open review upload';
		await openFile();
		await menuItem('Open').hover();
		const choosing = page.waitForEvent('filechooser');
		await menuItem('Load From...').click();
		await (
			await choosing
		).setFiles({
			name: uploadName,
			mimeType: 'application/octet-stream',
			buffer: await readFile(project)
		});
		await wait(inspectMedia, (value) =>
			value.media.some((entry) => entry.id === review.root.media)
		);
		const loaded = await inspect(review.root);
		assert.deepEqual(
			loaded.instances.map((value) => value.reference),
			review.instances
		);
		assert.deepEqual(loaded.canvas, { width: 320, height: 240 });
		assert.notDeepEqual(loaded.reference, fresh);
		await selectMediaChannel(page, await inspectMedia(), review.root);
		await properties
			.getByRole('navigation', { name: 'Ordered instances', exact: true })
			.getByRole('button', { name: loaded.instances[0].label, exact: true })
			.click();
		await wait(
			() => properties.getByLabel('Instance name', { exact: true }).inputValue(),
			(value) => value === loaded.instances[0].label
		);
		await wait(() => undo.isDisabled(), Boolean);
		await wait(() => redo.isDisabled(), Boolean);
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		stage = 'loaded instance edit and history';
		await properties.getByLabel('Instance name', { exact: true }).fill('After upload');
		await properties
			.getByRole('region', { name: 'Selected instance', exact: true })
			.getByRole('button', { name: 'Rename', exact: true })
			.click();
		const renamed = await wait(
			() => inspect(review.root),
			(value) => value.instances[0].label === 'After upload'
		);
		await undo.click();
		await wait(
			() => inspect(review.root),
			(value) => isDeepStrictEqual(value, loaded)
		);
		await redo.click();
		await wait(
			() => inspect(review.root),
			(value) => isDeepStrictEqual(value, renamed)
		);
		stage = 'actual File New clears loaded instance';
		await newProject();
		assert.deepEqual(await inspectMedia(), { assets: [], media: [] });
		assert.deepEqual(rejected, []);
		assert.deepEqual(errors, []);
		assert(!(await page.locator('body').innerText()).includes('media reference is missing'));
		assert.equal(await hash(), originalHash);
		await page.screenshot({ path: 'target/m06-composition-project-ui.png' });
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'actual New during accepted drag, old held-pointer disposal, fresh canvas history, browser review upload, saved IDs and post-load instance history; native imagery/extents unverified',
				browser: browser.version(),
				replaced: first,
				fresh,
				loaded: review.root,
				accepted_drag: dragged.instances[0].definition.transform,
				upload_name: uploadName,
				review_sha256: originalHash,
				rejected_requests: rejected.length,
				page_errors: errors.length,
				screenshot: 'target/m06-composition-project-ui.png'
			})
		);
	} catch (error) {
		console.error(
			JSON.stringify({
				failed_stage: stage,
				rejected_requests: rejected,
				body: (
					await page
						.locator('body')
						.innerText()
						.catch(() => '')
				).slice(-4500)
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
