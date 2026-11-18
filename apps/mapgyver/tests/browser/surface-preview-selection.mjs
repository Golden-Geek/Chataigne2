import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { ownedWindowHandle } from './capture-alignment.mjs';

export async function verifySurfacePreviewSelection({ page, post, edit, nativePid }) {
	const sources = [];
	for (const [label, rgba] of [
		['Red selection source', [1, 0, 0, 1]],
		['Blue selection source', [0, 0, 1, 1]]
	]) {
		const { reference } = await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label });
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
		await edit({ kind: 'ConfigureChannel', reference, source: { kind: 'SolidColor', rgba } });
		sources.push(reference);
	}
	async function screens(payload) {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.screens',
			operation: 'edit',
			payload
		});
		assert(ack.success, JSON.stringify(ack));
		return ack.result.value;
	}
	const { screen, surface: first } = await screens({ kind: 'CreateScreen', label: 'Preview test' });
	const surfaces = [];
	for (const [index, source] of [sources[0], sources[0], sources[1]].entries()) {
		const label = `Selection surface ${index + 1}`;
		const reference = index
			? (await screens({ kind: 'AddSurface', screen, label })).reference
			: first;
		if (!index) await screens({ kind: 'RenameSurface', reference, label });
		const { snapshot } = await screens({ kind: 'InspectSurface', reference });
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: { ...snapshot.definition, source }
		});
		surfaces.push({ label, reference, source });
	}
	await page.locator('.gc-dv-tab[data-content-panel-id="screen-manager"]').click();
	const tree = page.getByRole('region', { name: 'Screens', exact: true });
	await tree.locator('input[type="search"]').fill('Selection surface');
	const select = ({ label }) => tree.getByRole('button', { name: label, exact: true }).click();
	await select(surfaces[0]);
	await page.locator('.gc-dv-tab[data-content-panel-id="surface-source"]').click();
	const panel = page.locator('section[data-panel-id="surface-source"]');
	const preview = panel.locator('.native-preview');
	async function settled(item) {
		await page.waitForFunction(
			({ surface, source }) => {
				const panel = document.querySelector('section[data-panel-id="surface-source"]');
				const view = panel?.querySelector('.native-preview');
				if (panel?.dataset.surfaceTarget !== surface || !view?.dataset.previewSource) return false;
				const actual = JSON.parse(view.dataset.previewSource);
				return (
					actual.kind === 'Channel' &&
					actual.reference.media === source.media &&
					actual.reference.channel === source.channel &&
					view.dataset.previewInstalled === 'true' &&
					!view.querySelector('.unavailable')
				);
			},
			{ surface: item.reference.surface, source: item.source }
		);
	}
	await settled(surfaces[0]);
	const original = await preview.getAttribute('data-preview-view');
	assert(original);
	let observer, capture, closed;
	async function findCaptureFile(file) {
		const deadline = Date.now() + 5000;
		while (!(await stat(path.join(capture, file)).catch(() => null))) {
			assert(observer.exitCode === null && Date.now() < deadline, `Capture did not write ${file}`);
			await delay(25);
		}
	}
	const directory = process.env.MAPGYVER_SURFACE_SELECTION_CAPTURE;
	if (directory) {
		await mkdir(directory, { recursive: true });
		capture = await mkdtemp(path.join(path.resolve(directory), 'run-'));
		await writeFile(
			path.join(capture, 'ready.json'),
			JSON.stringify({
				pid: nativePid,
				boxes: [await preview.boundingBox()],
				dpr: await page.evaluate(() => window.devicePixelRatio)
			})
		);
		observer = spawn(
			'python',
			[
				'apps/mapgyver/tests/native/preview_selection_frames.py',
				capture,
				'--hwnd',
				ownedWindowHandle(nativePid)
			],
			{ windowsHide: true, stdio: 'inherit' }
		);
		closed = new Promise((resolve, reject) => {
			observer.once('close', resolve);
			observer.once('error', reject);
		});
	}
	try {
		if (capture) await Promise.race([findCaptureFile('capture-started'), closed]);
		for (const index of [1, 0, 1, 2, 0, 2, 1, 2, 0]) {
			await select(surfaces[index]);
			await settled(surfaces[index]);
			assert.equal(
				await preview.getAttribute('data-preview-view'),
				original,
				`Switching to Surface ${index + 1} must keep the native registration`
			);
			if (capture) await delay(100);
		}
		for (const index of [2, 1, 2, 0]) await select(surfaces[index]);
		await settled(surfaces[0]);
		assert.equal(await preview.getAttribute('data-preview-view'), original);
		if (capture) {
			await delay(100);
			await writeFile(path.join(capture, 'complete'), 'complete');
			assert.equal(await closed, 0, `Surface selection pixel observer failed in ${capture}`);
			const pixels = JSON.parse(await readFile(path.join(capture, 'pixels.json'), 'utf8'));
			assert.equal(pixels.result, 'pass');
		}
	} finally {
		if (observer?.exitCode === null) observer.kill();
		await closed;
	}
	console.log(
		JSON.stringify({ scope: 'Surface Source native selection', switches: 13, original, capture })
	);
}
