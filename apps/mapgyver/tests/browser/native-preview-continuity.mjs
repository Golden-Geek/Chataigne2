import assert from 'node:assert/strict';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { verifyPreviewContinuity } from './preview-continuity.mjs';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';

export async function verifyNativePreviewContinuity(options) {
	const directory = path.resolve(process.env.MAPGYVER_CONTINUITY_DIRECTORY);
	await mkdir(directory, { recursive: true });
	const output = await createPreviewOutputObserver(options.post, directory, options.page);
	let reconnect;
	const capture = process.env.MAPGYVER_CONTINUITY_CAPTURE === '1';
	async function waitCapture(name) {
		const deadline = Date.now() + 180_000;
		while (!(await stat(path.join(directory, name)).catch(() => null))) {
			assert(Date.now() < deadline, `native reconnect observer did not signal ${name}`);
			await delay(100);
		}
	}
	try {
		const flow = await verifyPreviewContinuity({
			...options,
			observe: async (phase, sources) => {
				await output.observe(phase, sources);
				if (phase === 'independent-channel-choice') {
					if (capture) {
						assert(
							!(await stat(path.join(directory, 'ready.json')).catch(() => null)),
							'fresh capture required'
						);
						const boxes = await Promise.all(
							(await options.page.locator('.media-preview-panel .native-preview').all()).map(
								(view) => view.boundingBox()
							)
						);
						await writeFile(
							path.join(directory, 'ready.json'),
							JSON.stringify({
								pid: options.nativePid,
								boxes,
								dpr: await options.page.evaluate(() => window.devicePixelRatio)
							})
						);
						console.log(
							JSON.stringify({
								action: 'activate-owned-window-and-start-steady-pixel-observer',
								pid: options.nativePid,
								directory
							})
						);
						await waitCapture('capture-started');
					}
					await options.page.evaluate(() => {
						const originals = [
							...document.querySelectorAll('.media-preview-panel .native-preview')
						];
						const state = {
							originals,
							removed: 0,
							missing: 0,
							unavailable: 0,
							samples: 0,
							stopped: false
						};
						window.__previewReconnect = state;
						state.observer = new MutationObserver((records) => {
							for (const record of records)
								for (const node of record.removedNodes)
									if (originals.some((view) => node === view || node.contains?.(view)))
										state.removed++;
						});
						state.observer.observe(document.body, { subtree: true, childList: true });
						function frame() {
							if (state.stopped) return;
							state.samples++;
							if (originals.some((view) => !view.isConnected)) state.missing++;
							if (originals.some((view) => view.querySelector('.unavailable'))) state.unavailable++;
							requestAnimationFrame(frame);
						}
						requestAnimationFrame(frame);
					});
				} else if (phase === 'transport-reconnected') {
					reconnect = await options.page.evaluate(() => {
						const state = window.__previewReconnect;
						state.stopped = true;
						state.observer.disconnect();
						return {
							views: state.originals.length,
							removed: state.removed,
							missing: state.missing,
							unavailable: state.unavailable,
							samples: state.samples
						};
					});
					console.log(JSON.stringify({ reconnect }));
					if (capture) {
						await writeFile(path.join(directory, 'complete'), 'complete');
						await waitCapture('capture-finished');
						reconnect.pixels = JSON.parse(
							await readFile(path.join(directory, 'pixels.json'), 'utf8')
						);
						assert.equal(reconnect.pixels.result, 'pass');
					}
				}
			}
		});
		const result = {
			flow,
			reconnect,
			output: await output.finish(),
			observations: output.observations
		};
		await writeFile(
			path.join(directory, 'native-continuity.json'),
			JSON.stringify(result, null, 2)
		);
		if (process.env.MAPGYVER_CONTINUITY_BASELINE !== '1') {
			assert.equal(reconnect.removed, 0);
			assert.equal(reconnect.missing, 0);
			assert.equal(reconnect.unavailable, 0);
		}
		console.log(JSON.stringify(result));
	} finally {
		await output.finish();
	}
}
