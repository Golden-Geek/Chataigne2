// Existing public native editor, physical capture guard and accepted-frame pointer API.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { captureAlignment } from './capture-alignment.mjs';

export async function verifyNativeMotion({ native, view, directory, editor, expectedHit }) {
	const start = await captureAlignment({ directory, stage: 'motion-start', page: native.page });
	const capture = spawn(
		'python',
		[
			'apps/mapgyver/tests/native/motion_frames.py',
			String(start.hwnd),
			String(native.app.pid),
			directory,
			editor
		],
		{ windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] }
	);
	let output = '',
		errors = '';
	const complete = new Promise((resolve, reject) => {
		capture.once('error', reject);
		capture.once('close', (code) =>
			code === 0 ? resolve() : reject(new Error(`motion capture ${code}: ${errors}`))
		);
	});
	capture.stderr.on('data', (chunk) => {
		errors += chunk;
	});
	const timeout = setTimeout(() => capture.kill(), 15000);
	try {
		await new Promise((resolve, reject) => {
			capture.stdout.on('data', (chunk) => {
				output += chunk;
				if (output.includes('"capture_ready": true')) resolve();
			});
			complete.then(() => reject(new Error('Motion capture ended before readiness')), reject);
		});
		const preview = view.locator('.native-preview');
		const identity = await preview.evaluate((element) => ({
			session: element.dataset.previewSession,
			view: element.dataset.previewView,
			source: element.dataset.previewSource
		}));
		assert(identity.session && identity.view && identity.source);
		const metadata = native.page.evaluate(
			async ({ identity, expectedHit }) => {
				const samples = [];
				const query = async (point) =>
					JSON.parse(
						await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
							request: JSON.stringify({ kind: 'Pointer', ...identity, point })
						})
					);
				window.__motionCaptureFinished = false;
				window.__motionSamplesReady = true;
				for (
					let index = 0;
					index < 128 && (index < 16 || !window.__motionCaptureFinished);
					index++
				) {
					const begin = performance.now();
					const initial = (await query([0, 0])).sample;
					if (initial) {
						const point = [
							initial.rect.x + initial.rect.width * 0.2,
							initial.rect.y + initial.rect.height * 0.2
						];
						const hit = (await query(point)).sample;
						const handle = document.querySelector(
							`[data-native-hit-id="${CSS.escape(expectedHit)}"]`
						);
						const rect = handle.getBoundingClientRect();
						samples.push({
							begin,
							end: performance.now(),
							initialRect: initial.rect,
							point,
							hit,
							dom: { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 }
						});
					}
					await new Promise(requestAnimationFrame);
				}
				return samples;
			},
			{ identity, expectedHit }
		);
		const finishMetadata = complete.then(
			() =>
				native.page.evaluate(() => {
					window.__motionCaptureFinished = true;
				}),
			async (error) => {
				await native.page
					.evaluate(() => {
						window.__motionCaptureFinished = true;
					})
					.catch(() => {});
				throw error;
			}
		);
		const completionResults = Promise.allSettled([complete, metadata, finishMetadata]);
		await native.page.waitForFunction(() => window.__motionSamplesReady);
		const box = await view.locator('.active-zone').boundingBox();
		await native.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await native.page.mouse.wheel(0, 120);
		const results = await completionResults;
		for (const result of results) if (result.status === 'rejected') throw result.reason;
		const samples = results[1].value;
		const frames = JSON.parse(
			await readFile(path.join(directory, 'motion', 'frames.json'), 'utf8')
		);
		const hits = samples.filter((s) => s.hit?.hit === expectedHit);
		const report = {
			editor,
			expectedHit,
			physical: frames,
			native: {
				samples,
				correctHits: hits.length,
				maximum_query_point_dom_separation_css: Math.max(
					...hits.map((s) => Math.hypot(s.point[0] - s.dom.x, s.point[1] - s.dom.y))
				)
			},
			scope:
				'All24 sampled physical images with independent landmark/overlay measurements and concurrent native accepted-frame hit/DOM metadata. No exact scanout-frame-ID join or latency acceptance.'
		};
		await writeFile(path.join(directory, 'native-motion.json'), JSON.stringify(report, null, 2));
		assert(
			hits.length >= 3,
			'Existing native moving-hit acceptance requires at least three correct hits'
		);
		assert(new Set(samples.map((s) => Math.round(s.hit?.rect.width ?? 0))).size >= 3);
		console.log(
			JSON.stringify({
				nativeMotion: 'pass',
				samples: frames.samples.length,
				hits: hits.length,
				maximum_image_overlay_error_px: frames.maximum_image_overlay_error_px
			})
		);
	} finally {
		clearTimeout(timeout);
		if (capture.exitCode === null) capture.kill();
		await complete.catch(() => {});
	}
}
