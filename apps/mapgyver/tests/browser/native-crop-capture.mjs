// Bounded trusted-event observation and public history for an owned picture fixture.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';

const directory = path.resolve(process.argv[2]);
const action = process.argv[3];
assert(['arm', 'arm-native', 'snapshot', 'undo', 'redo'].includes(action));
const ready = JSON.parse(await readFile(path.join(directory, 'ready.json'), 'utf8'));
assert.equal(ready.editor, 'source');
const browser = await chromium.connectOverCDP(ready.cdp, { timeout: 5000 });
try {
	const pages = browser
		.contexts()
		.flatMap((c) => c.pages())
		.filter((p) => p.url().startsWith(ready.base + '/'));
	assert.equal(pages.length, 1);
	const page = pages[0];
	if (action === 'arm' || action === 'arm-native') {
		await page.evaluate((native) => {
			const canvas = document.querySelector('svg[aria-label="Source crop controls"]');
			if (!canvas) throw new Error('Mounted source crop canvas required');
			if (native ? window.__nativeCropCapture : window.__cropCapture)
				throw new Error('This finite capture observer is already armed');
			const observed = native ? canvas.closest('[data-native-preview-backdrop]') : canvas;
			if (!observed) throw new Error('Native capture viewport required');
			const journal = {
				bounds: JSON.parse(JSON.stringify(observed.getBoundingClientRect())),
				events: [],
				overflow: false
			};
			if (native) window.__nativeCropCapture = journal;
			else window.__cropCapture = journal;
			for (const type of [
				'pointerdown',
				'pointermove',
				'pointerup',
				'pointercancel',
				'gotpointercapture',
				'lostpointercapture'
			]) {
				window.addEventListener(
					type,
					(event) => {
						if (
							!observed.contains(event.target) &&
							!event.target.closest?.('[data-native-hit-id^="crop:"]')
						)
							return;
						if (journal.events.length >= 256) {
							journal.overflow = true;
							return;
						}
						journal.events.push({
							type,
							trusted: event.isTrusted,
							id: event.pointerId,
							x: event.clientX,
							y: event.clientY,
							captured: observed.hasPointerCapture(event.pointerId),
							time: event.timeStamp
						});
					},
					{ capture: true, passive: true }
				);
			}
		}, action === 'arm-native');
	} else if (action === 'undo' || action === 'redo') {
		await page
			.getByTitle(action === 'undo' ? 'Ctrl/Cmd+Z' : 'Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true })
			.click();
		await delay(300);
	}
	const token = (
		await (
			await fetch(ready.base + '/api/ui/session', {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: '{}'
			})
		).json()
	).token;
	const response = await fetch(ready.base + '/api/ui/intent', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
		body: JSON.stringify({
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload: { kind: 'InspectSurface', reference: ready.reference }
		}),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	const result = await response.json();
	assert(result.success);
	const state = await page.evaluate(() => ({
		journal: window.__cropCapture,
		nativeJournal: window.__nativeCropCapture,
		handles: [...document.querySelectorAll('[data-native-hit-id^="crop:"]')].map((e) => ({
			id: e.dataset.nativeHitId,
			rect: JSON.parse(JSON.stringify(e.getBoundingClientRect()))
		}))
	}));
	const stage = process.argv[4] ?? action;
	assert(/^[a-z][a-z0-9-]{0,40}$/.test(stage));
	const proof = { pid: ready.pid, action, surface: result.result.value.snapshot, ...state };
	await writeFile(path.join(directory, `${stage}.json`), JSON.stringify(proof, null, 2));
	console.log(
		JSON.stringify({
			pid: ready.pid,
			action,
			stage,
			crop: proof.surface.definition.crop,
			nativeEvents: proof.nativeJournal?.events.length ?? 0
		})
	);
} finally {
	await browser.close();
}
