import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import { observeRejectedRequests } from './intent-observer.mjs';

const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyProceduralMediaUi({ base, edit, inspectMedia }) {
	const browser = await chromium.launch({ headless: true });
	const page = await browser.newPage({ viewport: { width: 1440, height: 1100 } });
	page.setDefaultTimeout(5000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'media-owned procedural settings');
	try {
		await page.goto(base);
		await page
			.getByRole('tab')
			.filter({ hasText: /^Media Manager$/ })
			.click();
		for (const kind of ['Composition', 'Matrix']) {
			const label = `${kind} sizing`;
			const { reference } = await edit({ kind: 'CreateMedia', media_kind: kind, label });
			if (kind === 'Matrix')
				await edit({
					kind: 'Matrix',
					media: reference.media,
					edit: { kind: 'AddLayers', parent: null, count: 2 }
				});
			const read = async () =>
				(await inspectMedia()).media.find((media) => media.id === reference.media);
			const canvas = (media) => {
				const source = media.channels.find(
					(channel) => channel.reference.channel === reference.channel
				).definition.source;
				return kind === 'Composition' ? source.canvas : source.output.definition.canvas;
			};
			async function wait(test) {
				const deadline = Date.now() + 5000;
				while (true) {
					const media = await read();
					if (test(media)) return media;
					assert(Date.now() < deadline, JSON.stringify(media));
					await delay(20);
				}
			}
			await page
				.locator('section[aria-label="Media Manager"]')
				.getByRole('treeitem', { name: label, exact: true })
				.getByRole('button', { name: label, exact: true })
				.click();
			const parameter = (label) =>
				page
					.locator('.parameter-inspector')
					.filter({ has: page.getByText(label, { exact: true }) });
			const resolution = parameter('Resolution');
			await resolution.waitFor();
			assert.deepEqual(canvas(await read()), { width: 1920, height: 1080 });
			assert.equal(await page.getByText('Canvas width', { exact: true }).count(), 0);
			assert.equal(await page.getByText('Canvas height', { exact: true }).count(), 0);
			const toggle = resolution.getByRole('switch', { name: 'Toggle node enabled' });
			assert.equal(await toggle.getAttribute('aria-checked'), 'false');
			await toggle.click();
			await wait((media) =>
				media.channels.every((channel) => channel.definition.size.kind === 'Fixed')
			);
			for (const [index, value] of [640, 360].entries()) {
				const input = resolution.locator('input.number-field').nth(index);
				await input.fill(String(value));
				await input.press('Enter');
			}
			await wait((media) => canvas(media).width === 640 && canvas(media).height === 360);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait((media) => canvas(media).height === 1080);
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait((media) => canvas(media).height === 360);
			await toggle.click();
			await wait(
				(media) =>
					canvas(media).width === 1920 &&
					media.channels.every((channel) => channel.definition.size.kind === 'Automatic')
			);
			await toggle.click();
			const media = await wait(
				(media) =>
					canvas(media).width === 640 &&
					media.channels.every((channel) => channel.definition.size.extent?.height === 360)
			);
			if (kind === 'Matrix') {
				const inspect = async () =>
					(await edit({ kind: 'Matrix', media: reference.media, edit: { kind: 'Inspect' } }))
						.snapshot;
				const document = await inspect();
				assert.deepEqual(document.definition.canvas, { width: 640, height: 360 });
				assert(!('transition' in document.definition), 'Transitions belong to Layers');
				assert.equal(await parameter('Transition duration').count(), 0);
				const layer = document.items.find((item) => item.id.kind === 'Layer');
				await page
					.getByRole('tab')
					.filter({ hasText: /^Matrix$/ })
					.click();
				await page
					.getByRole('grid', { name: 'Scrollable Matrix grid', exact: true })
					.getByTitle(`Select ${layer.label}`, { exact: true })
					.click();
				const duration = parameter('Transition duration').locator('input.time-field');
				await duration.fill('00:00:00.750');
				await duration.press('Enter');
				assert.equal(await parameter('Transition curve').count(), 0);
				const deadline = Date.now() + 5000;
				while (true) {
					const current = (await inspect()).items.find(
						(item) => item.id.kind === 'Layer' && item.id.id === layer.id.id
					).layer;
					if (current.duration_ms === 750) break;
					assert(Date.now() < deadline, JSON.stringify(current));
					await delay(20);
				}
			} else {
				const document = (await edit({ kind: 'Composition', reference, edit: { kind: 'Inspect' } }))
					.snapshot;
				assert.deepEqual(document.canvas, { width: 640, height: 360 });
				assert.equal(await parameter('Transition duration').count(), 0);
			}
			assert.equal(media.channels[0].reference.media, reference.media);
		}
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				result: 'pass',
				scope:
					'bundled procedural media Resolution/defaults/history, Matrix exports and media-owned settings'
			})
		);
	} finally {
		await browser.close();
	}
}
