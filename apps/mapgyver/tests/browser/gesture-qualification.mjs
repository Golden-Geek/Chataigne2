// Optional physical/native and independent-output checks for the owned gesture fixture.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { adapterKey } from './native-adapter.mjs';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { captureMatrixCellPixels } from './matrix-cell-previews.mjs';

export async function createGestureQualification({
	native,
	directory,
	post,
	source,
	hardware,
	gpuIndex
}) {
	const page = native.page;
	// Prepare this owned window with supported desktop control before the marker.
	// Park the desktop pointer outside image probes so cursor halos cannot alter pixels.
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
	const grids = [];
	let finished = false;
	try {
		await output.observe('first-grid', { first: source });
		assert.equal(output.adapter.key, adapterKey(hardware.adapters[gpuIndex].id));
	} catch (error) {
		await output.finish().catch(() => {});
		throw error;
	}
	return {
		async grid({ phase, snapshot, colors }) {
			const panel = page.locator('[data-panel-id="matrix"]');
			const surface = panel.locator('.cell-previews');
			await page.waitForFunction(() => {
				const surface = document.querySelector('[data-panel-id="matrix"] .cell-previews');
				const views = surface?.querySelectorAll('.native-preview');
				return (
					surface?.getAttribute('data-matrix-preview-source-count') === '9' &&
					views.length === 1 &&
					[...views].every(
						(view) =>
							view.dataset.previewState === 'available' &&
							view.dataset.previewInstalled === 'true' &&
							!view.querySelector('.tile-unavailable')
					)
				);
			});
			assert.equal(await surface.locator('.native-preview').count(), 1);
			const cells = [];
			for (let row = 0; row < 3; row++)
				for (let column = 0; column < 3; column++) {
					const cell = panel
						.locator(`.row[data-row-id="${snapshot.items[row].id.id}"]`)
						.getByRole('gridcell')
						.nth(column);
					cells.push({ box: await cell.boundingBox(), rgb: colors[row * 3 + column].rgb });
				}
			const pixels = await captureMatrixCellPixels({
				directory,
				phase,
				page,
				nativePid: native.app.pid,
				cells
			});
			grids.push({ phase, logical_sources: 9, native_views: 1, pixels });
		},
		async pending({ name, svg, shape, control, initial, latest }) {
			const nativeControls = ['Screen warp corner', 'Surface Source crop'].includes(name);
			let hit = null,
				point = null,
				sample = null;
			if (nativeControls) {
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
				hit = await control.getAttribute('data-native-hit-id');
				assert(hit);
				const box = await control.boundingBox();
				point = [box.x + box.width / 2, box.y + box.height / 2];
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
			}
			// SVG screen CTM includes each editor's actual viewBox and viewport transform;
			// Matrix/Composition coordinates are canvas units, Screen/Source are unit space.
			const geometry = await svg.evaluate(
				(element, { initial, latest, resize }) => {
					const matrix = element.getScreenCTM();
					if (!matrix) throw Error('Visible SVG projection required');
					const project = (v) => {
						const at = element.createSVGPoint();
						// Stay clear of endpoint and midpoint resize handles painted over the edge.
						const along = 0.37;
						// A corner resize preserves the opposite top edge's line. Probe its
						// moved bottom edge so an old point cannot still lie on the new boundary.
						const vertices = resize ? [...v.slice(4), ...v.slice(0, 4)] : v;
						[at.x, at.y] =
							v.length === 4
								? [v[0] + v[2] * along, v[1]]
								: [
										vertices[0] + (vertices[2] - vertices[0]) * along,
										vertices[1] + (vertices[3] - vertices[1]) * along
									];
						const point = at.matrixTransform(matrix);
						return [point.x, point.y];
					};
					return {
						expected: project(latest),
						stale: project(initial),
						initial,
						latest,
						projection: [matrix.a, matrix.b, matrix.c, matrix.d, matrix.e, matrix.f],
						probe_edge: resize ? 'bottom' : 'top',
						dpr: devicePixelRatio
					};
				},
				{ initial, latest, resize: name === 'Composition rotated resize' }
			);
			const stroke = await shape.evaluate((element) => getComputedStyle(element).stroke);
			assert(/^rgb\(\d+, \d+, \d+\)$/.test(stroke), `Explicit RGB edge required: ${stroke}`);
			geometry.color = stroke.match(/\d+/g).map(Number);
			const stage = name.toLowerCase().replaceAll(' ', '-') + '-pending';
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
			await writeFile(path.join(folder, 'geometry.json'), JSON.stringify(geometry));
			const pixels = JSON.parse(
				execFileSync('python', ['apps/mapgyver/tests/native/pending_overlay.py', folder], {
					encoding: 'utf8',
					windowsHide: true
				})
			);
			alignment.push({ name, hit, point, sample, stroke, pixels, native_controls: nativeControls });
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
				grids,
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
