// Optional physical/native and independent-output checks for the owned gesture fixture.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { adapterKey } from './native-adapter.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function createGestureQualification({
	native,
	directory,
	post,
	source,
	hardware,
	gpuIndex
}) {
	const page = native.page;
	await writeFile(
		path.join(directory, 'activate.json'),
		JSON.stringify({ pid: native.app.pid, binary: process.env.MAPGYVER_TEST_BINARY })
	);
	const deadline = Date.now() + 90000;
	while (!(await stat(path.join(directory, 'activated')).catch(() => null))) {
		assert(Date.now() < deadline, 'Owned-window activation marker timed out');
		await delay(100);
	}
	const output = await createPreviewOutputObserver(post, path.join(directory, 'output'), page);
	const alignment = [];
	let finished = false;
	try {
		await output.observe('first-grid', { first: source });
		assert.equal(output.adapter.key, adapterKey(hardware.adapters[gpuIndex].id));
	} catch (error) {
		await output.finish().catch(() => {});
		throw error;
	}
	return {
		async pending({ name, svg, control, initial, latest }) {
			if (!['Screen warp corner', 'Surface Source crop'].includes(name)) return;
			const preview = svg
				.locator('xpath=ancestor::*[@data-native-preview-backdrop][1]')
				.locator('.native-preview');
			// The preview and DOM controls are siblings inside their common viewport.
			const identity = await preview.evaluate((e) => ({
				session: e.dataset.previewSession,
				view: e.dataset.previewView,
				source: e.dataset.previewSource
			}));
			assert(identity.session && identity.view && identity.source);
			const hit = await control.getAttribute('data-native-hit-id');
			assert(hit);
			const box = await control.boundingBox();
			const point = [box.x + box.width / 2, box.y + box.height / 2];
			let sample;
			const until = Date.now() + 5000;
			do {
				sample = await page.evaluate(
					async ({ identity, point }) =>
						JSON.parse(
							await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
								request: JSON.stringify({ kind: 'Pointer', ...identity, point })
							})
						),
					{ identity, point }
				);
				if (sample.sample?.hit === hit) break;
				assert(Date.now() < until, JSON.stringify({ name, hit, sample }));
				await delay(30);
			} while (true);
			const area = await svg.boundingBox();
			const midpoint = (v) =>
				v.length === 4 ? [v[0] + v[2] / 2, v[1]] : [(v[0] + v[2]) / 2, (v[1] + v[3]) / 2];
			const project = (v) => {
				const [x, y] = midpoint(v);
				return [area.x + x * area.width, area.y + y * area.height];
			};
			const stage = name.startsWith('Screen') ? 'screen-pending' : 'source-pending';
			const folder = path.join(directory, stage);
			const hwnd = ownedWindowHandle(native.app.pid);
			execFileSync(
				'python',
				[
					'apps/mapgyver/tests/native/window_capture.py',
					'--visible',
					'--hwnd',
					hwnd,
					'--pid',
					String(native.app.pid),
					folder
				],
				{ windowsHide: true }
			);
			const geometry = {
				expected: project(latest),
				stale: project(initial),
				dpr: await page.evaluate(() => devicePixelRatio)
			};
			await writeFile(path.join(folder, 'geometry.json'), JSON.stringify(geometry));
			const pixels = JSON.parse(
				execFileSync('python', ['apps/mapgyver/tests/native/pending_overlay.py', folder], {
					encoding: 'utf8',
					windowsHide: true
				})
			);
			alignment.push({ name, hit, point, sample, pixels });
		},
		async after(name) {
			await output.observePixels(name.toLowerCase().replaceAll(' ', '-'), [255, 0, 0, 255]);
		},
		async finish() {
			const summary = await output.finish();
			finished = true;
			return {
				adapter: output.adapter,
				alignment,
				output: summary,
				observations: output.observations,
				scope:
					'Pending native-hit/DOM and visible edge pixels plus independent output continuity; no high-refresh timing acceptance'
			};
		},
		async close() {
			if (!finished) await output.finish();
		}
	};
}
