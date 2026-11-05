// Owned current-build reproduction of M12/1's Matrix and dock-resize recordings.
import assert from 'node:assert/strict';
import { mkdir, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
await mkdir(directory, { recursive: true });
const exists = async (name) => !!(await stat(path.join(directory, name)).catch(() => null));
assert(!(await exists('ready.json')), 'Use a fresh reproduction directory');
const native = await openNativeEditor({ appDataDirectory: process.env.MAPGYVER_TEST_APP_DATA });
let token;
async function post(route, body) {
	const response = await fetch(`${native.base}${route}`, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	assert(response.ok);
	return response.json();
}
async function edit(payload) {
	const ack = await post('/api/ui/intent', {
		kind: 'invokeAppOperation',
		namespace: 'mapgyver.media',
		operation: 'edit',
		payload
	});
	assert(ack.success, JSON.stringify(ack));
	return ack.result.value;
}
try {
	token = (await post('/api/ui/session', {})).token;
	const { reference: source } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Stability red'
	});
	await edit({
		kind: 'ConfigureChannel',
		reference: source,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	const { reference: matrix } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'Stability Matrix'
	});
	const matrixEdit = (value) => edit({ kind: 'Matrix', media: matrix.media, edit: value });
	const { snapshot } = await matrixEdit({ kind: 'Inspect' });
	assert.equal(snapshot.columns.length, 3);
	assert.equal(snapshot.items.length, 3);
	await matrixEdit({
		kind: 'SetCells',
		assignments: [{ layer: snapshot.items[1].id.id, column: snapshot.columns[0].id, source }]
	});
	await matrixEdit({ kind: 'LaunchColumn', column: snapshot.columns[0].id, mode: 'Configured' });
	const page = native.page;
	page.on('pageerror', (error) => console.error(`PAGE_ERROR ${error}`));
	const tab = (label) => page.getByRole('tab').filter({ hasText: new RegExp(`^${label}(?::|$)`) });
	await tab('Medias').click();
	await page
		.locator('section[aria-label="Medias"]')
		.getByRole('button', { name: 'Stability Matrix', exact: true })
		.click();
	await tab('Matrix').click();
	const inspectorGroup = tab('Inspector').locator(
		'xpath=ancestor::*[contains(@class,"dv-groupview")][1]'
	);
	await inspectorGroup
		.getByRole('button', { name: 'Add panel in this group', exact: true })
		.click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Media Preview', { exact: true }) })
		.click();
	const preview = page
		.locator('.media-preview-panel')
		.filter({ has: page.getByText('Stability Matrix', { exact: true }) });
	await preview.getByRole('button', { name: 'Lock media', exact: true }).click();
	await preview.locator('.native-preview').waitFor({ state: 'attached' });
	const metadata = await page.evaluate(async () => ({
		viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio },
		background: getComputedStyle(document.documentElement)
			.getPropertyValue('--gc-color-background')
			.trim(),
		views: [...document.querySelectorAll('.native-preview')].map((view) => ({
			state: view.dataset.previewState,
			installed: view.dataset.previewInstalled,
			id: view.dataset.previewView,
			rect: view.getBoundingClientRect().toJSON(),
			parent: view.parentElement.getBoundingClientRect().toJSON(),
			text: view.innerText.slice(0, 200)
		})),
		runtime: JSON.parse(
			await window.__TAURI_INTERNALS__.invoke('native_presentation_control', {
				request: JSON.stringify({ kind: 'mapgyverRuntimeStatus' })
			})
		),
		sashes: [...document.querySelectorAll('.dv-sash')].map((element) => ({
			class: element.className,
			rect: element.getBoundingClientRect().toJSON()
		}))
	}));
	await writeFile(
		path.join(directory, 'ready.json'),
		JSON.stringify({ pid: native.app.pid, matrix, source, ...metadata }, null, 2)
	);
	console.log(JSON.stringify({ ready: true, pid: native.app.pid, directory }));
	const deadline = Date.now() + 480000;
	while (!(await exists('begin'))) {
		assert(Date.now() < deadline, 'foreground preparation timeout');
		await delay(50);
	}
	await page.evaluate(() => {
		const matrix = document.querySelector('[data-panel-id="matrix"]');
		const preview = document.querySelector('.media-preview-panel');
		const views = [...document.querySelectorAll('.native-preview')];
		const state = {
			stopped: false,
			samples: [],
			overflow: false,
			matrix,
			preview,
			views,
			removed: 0,
			missing: 0,
			events: []
		};
		window.__panelStability = state;
		const observer = new MutationObserver((records) => {
			for (const record of records)
				for (const node of record.removedNodes)
					if (
						[matrix, preview, ...views].some(
							(view) => view && (node === view || node.contains?.(view))
						)
					)
						state.removed++;
		});
		observer.observe(document.body, { subtree: true, childList: true });
		state.observer = observer;
		for (const phase of ['pointerdown', 'pointerup', 'keydown', 'keyup'])
			window.addEventListener(
				phase,
				(event) => {
					if (state.events.length < 512)
						state.events.push({ phase, at: performance.now(), trusted: event.isTrusted });
				},
				{ capture: true, passive: true }
			);
		let last = -Infinity;
		function frame(now) {
			if (state.stopped) return;
			if (now - last >= 16) {
				last = now;
				if (
					!matrix?.isConnected ||
					!preview?.isConnected ||
					views.some((view) => !view.isConnected)
				)
					state.missing++;
				if (state.samples.length >= 4096) state.overflow = true;
				else
					state.samples.push({
						at: now,
						timeOrigin: performance.timeOrigin,
						grid: matrix?.getBoundingClientRect().toJSON(),
						headers: [...matrix.querySelectorAll('[role="columnheader"]')].map((element) => ({
							id: element.dataset.columnId,
							rect: element.getBoundingClientRect().toJSON()
						})),
						preview: preview?.getBoundingClientRect().toJSON(),
						views: views.map((view) => ({
							id: view.dataset.previewView,
							state: view.dataset.previewState,
							installed: view.dataset.previewInstalled,
							rect: view.getBoundingClientRect().toJSON(),
							unavailable: !!view.querySelector('.unavailable')
						})),
						covers: [...document.querySelectorAll('[data-native-preview-retirement]')].map(
							(element) => element.getBoundingClientRect().toJSON()
						)
					});
			}
			requestAnimationFrame(frame);
		}
		requestAnimationFrame(frame);
	});
	await writeFile(path.join(directory, 'observing'), 'observing');
	while (!(await exists('finish'))) {
		assert(Date.now() < deadline, 'interaction timeout');
		await delay(50);
	}
	const result = await page.evaluate(() => {
		const state = window.__panelStability;
		state.stopped = true;
		state.observer.disconnect();
		return {
			removed: state.removed,
			missing: state.missing,
			samples: state.samples,
			overflow: state.overflow,
			events: state.events
		};
	});
	await writeFile(path.join(directory, 'observations.json'), JSON.stringify(result, null, 2));
	console.log(
		JSON.stringify({
			complete: true,
			samples: result.samples.length,
			removed: result.removed,
			missing: result.missing,
			overflow: result.overflow
		})
	);
} catch (error) {
	const views = await native.page
		.evaluate(() =>
			[...document.querySelectorAll('.native-preview')].map((view) => ({
				state: view.dataset.previewState,
				installed: view.dataset.previewInstalled,
				rect: view.getBoundingClientRect().toJSON(),
				text: view.innerText.slice(0, 200)
			}))
		)
		.catch(() => null);
	await writeFile(
		path.join(directory, 'failure.json'),
		JSON.stringify({ error: String(error), views, log: native.log() }, null, 2)
	);
	throw error;
} finally {
	const observations = await native.page
		.evaluate(() => {
			const state = window.__panelStability;
			if (!state) return null;
			state.stopped = true;
			state.observer.disconnect();
			return {
				removed: state.removed,
				missing: state.missing,
				samples: state.samples,
				overflow: state.overflow,
				events: state.events
			};
		})
		.catch(() => null);
	if (observations && !(await exists('observations.json')))
		await writeFile(
			path.join(directory, 'observations.json'),
			JSON.stringify(observations, null, 2)
		);
	await native.close();
}
