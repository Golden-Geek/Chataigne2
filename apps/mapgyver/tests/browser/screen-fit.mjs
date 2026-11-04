import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { crc32, deflateSync } from 'node:zlib';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

async function portraitPng(directory) {
	const chunk = (name, bytes) => {
		const body = Buffer.concat([Buffer.from(name), bytes]),
			header = Buffer.alloc(4),
			tail = Buffer.alloc(4);
		header.writeUInt32BE(bytes.length);
		tail.writeUInt32BE(crc32(body));
		return Buffer.concat([header, body, tail]);
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(2, 0);
	header.writeUInt32BE(4, 4);
	header[8] = 8;
	header[9] = 6;
	const pixels = Buffer.alloc(4 * 9);
	for (let y = 0; y < 4; y++)
		for (let x = 0; x < 2; x++) {
			pixels[y * 9 + 1 + x * 4] = 255;
			pixels[y * 9 + 4 + x * 4] = 255;
		}
	const file = path.join(directory, 'fit-portrait.png');
	await writeFile(
		file,
		Buffer.concat([
			Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
			chunk('IHDR', header),
			chunk('IDAT', deflateSync(pixels)),
			chunk('IEND', Buffer.alloc(0))
		])
	);
	return file;
}

export async function verifyScreenFit({ base, post, edit, directory, page: nativePage = null }) {
	let history,
		stage = 'setup';
	async function screens(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value;
	}
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Fit source'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: source.media,
		size: { kind: 'Fixed', extent: { width: 1600, height: 900 } }
	});
	const { screen, surface: reference } = await screens({
		kind: 'CreateScreen',
		label: 'Fit screen'
	});
	await screens({
		kind: 'ConfigureScreen',
		screen,
		definition: {
			enabled: true,
			target: { kind: 'Spout', sender: `Fit-${screen}`, extent: { width: 800, height: 600 } }
		}
	});
	const { snapshot } = await screens({ kind: 'InspectSurface', reference });
	await screens({
		kind: 'ConfigureSurface',
		reference,
		definition: {
			...snapshot.definition,
			source,
			opacity: 0.65,
			crop: { x: 0.25, y: 0, width: 0.5, height: 1 },
			quad: {
				corners: [
					{ x: 0.1, y: 0.2 },
					{ x: 0.8, y: 0.3 },
					{ x: 0.7, y: 0.8 },
					{ x: 0.2, y: 0.9 }
				]
			}
		}
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(6000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => stage);
	const main = page.locator('[data-panel-id="screen-warp"]');
	const view = main.getByRole('application', { name: 'Screen view', exact: true });
	const fill = main.getByRole('button', { name: 'Fill Screen', exact: true });
	const fit = main.getByRole('button', { name: 'Fit to Screen', exact: true });
	const inspect = async () => (await screens({ kind: 'InspectSurface', reference })).snapshot;
	const near = (a, b) => Math.abs(a - b) < 1e-6;
	async function wait(test) {
		const until = Date.now() + 6000;
		for (;;) {
			const result = await inspect();
			if (test(result, history)) return result;
			assert(Date.now() < until, JSON.stringify({ stage, result, history }));
			await delay(20);
		}
	}
	async function ready() {
		await page.waitForFunction(
			() =>
				document.querySelector('[data-panel-id="screen-warp"]')?.getAttribute('aria-busy') ===
				'false'
		);
	}
	async function visible(expected) {
		await page.waitForFunction(
			({ id, corners }) => {
				const points = document
					.querySelector(`polygon[data-surface-drop-target="${id}"]`)
					?.getAttribute('points')
					?.split(' ')
					.map((pair) => pair.split(',').map(Number));
				return (
					points?.length === 4 &&
					points.every(
						([x, y], i) => Math.abs(x - corners[i].x) < 1e-6 && Math.abs(y - corners[i].y) < 1e-6
					)
				);
			},
			{ id: reference.surface, corners: expected.definition.quad.corners }
		);
		await ready();
	}
	async function undo(expected) {
		await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
		await wait((s) => JSON.stringify(s.definition) === JSON.stringify(expected.definition));
		await visible(expected);
	}
	async function selectSurface() {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screens$/ })
			.click();
		await page
			.locator('section[aria-label="Screens"]')
			.getByRole('button', { name: snapshot.label, exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Screen Warping(?::|$)/ })
			.click();
		await ready();
	}
	try {
		if (!nativePage) await page.goto(base);
		await selectSurface();
		if (nativePage)
			await main
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
		const before = await inspect();
		const beforeUndo = history.undo_len;
		stage = 'Fit preserves cropped-source aspect';
		await fit.click();
		const fitted = await wait(
			(s, h) =>
				!h.active_edit_session &&
				near(s.definition.quad.corners[0].x, 1 / 6) &&
				near(s.definition.quad.corners[2].x, 5 / 6)
		);
		assert.equal(history.undo_len, beforeUndo + 1);
		assert(
			near(fitted.definition.quad.corners[0].y, 0) && near(fitted.definition.quad.corners[2].y, 1)
		);
		assert.deepEqual({ ...fitted.definition, quad: null }, { ...before.definition, quad: null });
		await visible(fitted);
		await undo(before);
		await selectSurface();
		stage = 'Fill stretches while preserving settings';
		await fill.click();
		const filled = await wait(
			(s) =>
				near(s.definition.quad.corners[0].x, 0) &&
				near(s.definition.quad.corners[0].y, 0) &&
				near(s.definition.quad.corners[2].x, 1) &&
				near(s.definition.quad.corners[2].y, 1)
		);
		assert.equal(history.undo_len, beforeUndo + 1);
		assert.deepEqual({ ...filled.definition, quad: null }, { ...before.definition, quad: null });
		await visible(filled);
		await undo(before);
		await selectSurface();
		stage = 'selection controls action availability';
		const rect = await main.locator('.active-zone').boundingBox();
		assert(rect);
		await page.mouse.click(rect.x + rect.width * 0.95, rect.y + rect.height * 0.05);
		await page.waitForFunction((element) => element.disabled, await fill.elementHandle());
		assert(await fill.isDisabled());
		assert(await fit.isDisabled());
		assert.deepEqual(await inspect(), before);
		if (nativePage) {
			stage = 'native automatic Picture resolution';
			const file = await portraitPng(directory);
			const { asset } = await edit({
				kind: 'ImportAsset',
				locator: { kind: 'absolute', path: file },
				label: null
			});
			const { reference: picture } = await edit({
				kind: 'CreateMedia',
				media_kind: 'Picture',
				label: 'Fit portrait'
			});
			await edit({
				kind: 'ConfigureChannel',
				reference: picture,
				source: { kind: 'Picture', asset }
			});
			await screens({
				kind: 'ConfigureSurface',
				reference,
				definition: {
					...before.definition,
					source: picture,
					crop: { x: 0, y: 0, width: 1, height: 1 }
				}
			});
			await selectSurface();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Surface Source(?::|$)/ })
				.click();
			await page.waitForFunction(() => {
				const active = document
					.querySelector('[aria-label="Surface Source preview"] .active-zone')
					?.getBoundingClientRect();
				return active && Math.abs(active.width / active.height - 0.5) < 0.001;
			});
			await page
				.getByRole('application', { name: 'Surface source view', exact: true })
				.locator('.native-preview[data-preview-state="available"][data-preview-installed="true"]')
				.waitFor();
			await page
				.getByRole('tab')
				.filter({ hasText: /^Screen Warping(?::|$)/ })
				.click();
			await ready();
			const portrait = await inspect();
			const count = history.undo_len;
			await fit.click();
			const result = await wait(
				(s) =>
					near(s.definition.quad.corners[0].x, 0.3125) &&
					near(s.definition.quad.corners[2].x, 0.6875)
			);
			assert.equal(history.undo_len, count + 1);
			assert.deepEqual(
				{ ...result.definition, quad: null },
				{ ...portrait.definition, quad: null }
			);
			await visible(result);
			await undo(portrait);
		}
		await post('/api/ui/project-new', {});
		await view.waitFor({ state: 'detached' });
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		console.log(
			JSON.stringify({
				screenFit:
					'Fit preserves cropped-source aspect, Fill restores bounds, unchanged settings, one-step Undo, selection availability',
				automaticPictureNative: !!nativePage,
				pageErrors: []
			})
		);
	} catch (error) {
		console.error(JSON.stringify({ stage, errors, rejected }));
		throw error;
	} finally {
		await browser?.close();
	}
}
