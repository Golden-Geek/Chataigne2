import assert from 'node:assert/strict';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyPreviewSelectionStability({ page, edit, nativePid }) {
	const sources = [];
	for (const [label, rgba] of [
		['Selection red', [1, 0, 0, 1]],
		['Selection blue', [0, 0, 1, 1]]
	]) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
		await edit({ kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
		const { reference: peer } = await edit({
			kind: 'AddChannel',
			media: reference.media,
			label: 'Peer'
		});
		await edit({ kind: 'ConfigureChannel', reference: peer, source: { kind: 'SolidColor', rgba } });
		sources.push({ label, reference });
	}
	const panel = page.locator('.media-preview-panel[data-panel-id="media-preview"]');
	const grid = process.env.MAPGYVER_STABILITY_GRID === '1';
	async function select(source) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await page
			.getByRole('region', { name: 'Medias', exact: true })
			.getByRole('button', { name: source.label, exact: true })
			.click();
	}
	async function settled(source) {
		await page.waitForFunction(
			({ media }) => {
				const panel = document.querySelector('.media-preview-panel[data-panel-id="media-preview"]');
				const views = [...(panel?.querySelectorAll('.native-preview') ?? [])];
				return (
					panel?.dataset.previewMedia === media &&
					views.length > 0 &&
					views.every(
						(view) =>
							view.dataset.previewInstalled === 'true' &&
							view.dataset.previewState === 'available' &&
							!view.querySelector('.unavailable')
					)
				);
			},
			{ media: source.reference.media }
		);
	}
	await select(sources[0]);
	await page
		.getByRole('tab')
		.filter({ hasText: /^Media Preview$/ })
		.click();
	if (grid) await panel.getByRole('button', { name: 'All-channel grid', exact: true }).click();
	await settled(sources[0]);
	const capture = process.env.MAPGYVER_STABILITY_CAPTURE_DIRECTORY;
	async function waitCapture(name) {
		const deadline = Date.now() + 180_000;
		while (!(await stat(path.join(capture, name)).catch(() => null))) {
			assert(Date.now() < deadline, `native pixel observer did not signal ${name}`);
			await delay(100);
		}
	}
	if (capture) {
		await mkdir(capture, { recursive: true });
		assert(
			!(await stat(path.join(capture, 'ready.json')).catch(() => null)),
			'capture requires a fresh directory'
		);
		const boxes = await Promise.all(
			(await panel.locator('.native-preview').all()).map((view) => view.boundingBox())
		);
		const dpr = await page.evaluate(() => window.devicePixelRatio);
		await writeFile(
			path.join(capture, 'ready.json'),
			JSON.stringify({ pid: nativePid, boxes, dpr })
		);
		console.log(
			JSON.stringify({
				action: 'activate-owned-window-and-start-native-pixel-observer',
				pid: nativePid,
				capture
			})
		);
		await waitCapture('capture-started');
	}
	await panel.evaluate((panel) => {
		const originals = [...panel.querySelectorAll('.native-preview')];
		const original = originals[0];
		const state = {
			original,
			removed: 0,
			loading: 0,
			missingFrames: 0,
			totalFrames: 0,
			stopped: false
		};
		window.__previewSelectionStability = state;
		state.observer = new MutationObserver((records) => {
			for (const record of records)
				for (const node of record.removedNodes) {
					if (originals.some((original) => node === original || node.contains?.(original)))
						state.removed++;
				}
			if (panel.querySelector('.editor-empty')?.textContent.includes('Loading')) state.loading++;
		});
		state.observer.observe(panel, { childList: true, subtree: true });
		function frame() {
			if (state.stopped) return;
			state.totalFrames++;
			if (
				panel.querySelectorAll('.native-preview').length !== originals.length ||
				originals.some((original) => !original.isConnected)
			)
				state.missingFrames++;
			requestAnimationFrame(frame);
		}
		requestAnimationFrame(frame);
	});
	const switches = capture ? 16 : 8;
	for (let index = 0; index < switches; index++) {
		const source = sources[(index + 1) % sources.length];
		await select(source);
		await settled(source);
		if (capture) await delay(100);
	}
	const result = await page.evaluate(() => {
		const state = window.__previewSelectionStability;
		state.stopped = true;
		state.observer.disconnect();
		return {
			removed: state.removed,
			loading: state.loading,
			missingFrames: state.missingFrames,
			totalFrames: state.totalFrames,
			originalConnected: state.original.isConnected
		};
	});
	console.log(
		JSON.stringify({
			scope: 'native Media Preview follow selection without panel/view teardown',
			switches,
			mode: grid ? 'grid' : 'single',
			...result
		})
	);
	if (capture) {
		await writeFile(path.join(capture, 'complete'), 'complete');
		await waitCapture('capture-finished');
		const pixels = JSON.parse(await readFile(path.join(capture, 'pixels.json'), 'utf8'));
		console.log(JSON.stringify({ native_pixels: pixels }));
		assert.equal(pixels.result, 'pass', 'actual composited native pixels must remain red or blue');
	}
	assert.equal(result.removed, 0, 'selection must preserve the existing native view registration');
	assert.equal(result.loading, 0, 'selection must not insert a loading/blank panel');
	assert.equal(result.missingFrames, 0, 'selection must preserve a valid native view throughout');
}
