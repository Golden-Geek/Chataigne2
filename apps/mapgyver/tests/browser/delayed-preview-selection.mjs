import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdir, mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { ownedWindowHandle } from './capture-alignment.mjs';
import { observeNativeAdapter } from './native-adapter.mjs';
import { inventory, source } from './native-sender-selection.mjs';

export async function verifyDelayedPreviewSelection({ page, post, edit, nativePid }) {
	const root = path.resolve(process.env.MAPGYVER_DELAYED_PREVIEW_CAPTURE ?? 'target/m165-handoff');
	await mkdir(root, { recursive: true });
	const sender = `MapGyver delayed preview ${nativePid}`;
	const { reference: red } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SolidColor',
		label: 'Handoff red'
	});
	const { reference: blue } = await edit({
		kind: 'CreateMedia',
		media_kind: 'SharedTexture',
		label: 'Handoff blue'
	});
	await edit({ kind: 'ConfigureMediaSenderSelection', media: blue.media, mode: 'Manual' });
	for (const reference of [red, blue]) {
		await edit({
			kind: 'ConfigureMediaResolution',
			media: reference.media,
			size: { kind: 'Fixed', extent: { width: 64, height: 64 } }
		});
	}
	await edit({
		kind: 'ConfigureChannel',
		reference: red,
		source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
	});
	await edit({ kind: 'ConfigureChannel', reference: blue, source: { kind: 'Spout', sender } });
	const { media: referenceMedia } = await edit({
		kind: 'CreateReference',
		target: blue.media,
		label: 'Handoff blue Reference'
	});
	await edit({ kind: 'SelectReferenceChannel', media: referenceMedia, channel: blue.channel });
	const {
		page: { channels }
	} = await edit({ kind: 'InspectChannels', query: { media: referenceMedia, start: 0, count: 1 } });
	assert.deepEqual(channels[0].definition.source, { kind: 'Reference', target: blue });
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
	const { screen } = await screens({ kind: 'CreateScreen', label: 'Handoff screen' });
	const surfaceLabels = [
		'Handoff surface A',
		'Handoff surface B',
		'Handoff surface C',
		'Handoff surface D'
	];
	const surfaceTargets = new Map();
	for (const [index, label] of surfaceLabels.entries()) {
		const { reference } = await screens({ kind: 'AddSurface', screen, label });
		const channel = [red, blue, blue, channels[0].reference][index];
		surfaceTargets.set(label, { id: reference.surface, channel });
		const { snapshot } = await screens({ kind: 'InspectSurface', reference });
		await screens({
			kind: 'ConfigureSurface',
			reference,
			definition: { ...snapshot.definition, source: channel }
		});
	}
	const activate = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`).click();
	let producer;
	try {
		for (const [panel, manager, labels] of [
			['surface-source', 'Screens', surfaceLabels],
			['media-preview', 'Medias', ['Handoff red', 'Handoff blue']]
		]) {
			await activate(manager === 'Screens' ? 'screen-manager' : 'media-manager');
			const tree = page.getByRole('region', { name: manager, exact: true });
			if (manager === 'Screens') await tree.locator('input[type="search"]').fill('Handoff surface');
			const select = (label) => tree.getByRole('button', { name: label, exact: true }).click();
			await select(labels[0]);
			await activate(panel);
			const view = page.locator(`[data-panel-id="${panel}"] .native-preview`);
			await displayed(view, red);
			const viewId = await view.getAttribute('data-preview-view');
			if (!producer) {
				const { index } = await observeNativeAdapter(page, await inventory());
				producer = await source(index, sender);
			}
			const directory = await mkdtemp(path.join(root, `${panel}-`));
			const hwnd = ownedWindowHandle(nativePid);
			await writeFile(
				path.join(directory, 'ready.json'),
				JSON.stringify({
					pid: nativePid,
					boxes: [await view.boundingBox()],
					dpr: await page.evaluate(() => devicePixelRatio)
				})
			);
			const observer = spawn(
				'python',
				['apps/mapgyver/tests/native/preview_selection_frames.py', directory, '--hwnd', hwnd],
				{ windowsHide: true, stdio: 'inherit' }
			);
			const exited = new Promise((resolve, reject) => {
				observer.once('close', resolve);
				observer.once('error', reject);
			});
			try {
				await Promise.race([
					waitForStart(directory),
					exited.then(() => {
						throw Error(`no initial red preview in ${directory}`);
					})
				]);
				await select(labels[1]);
				await view.getByText('Source Unavailable', { exact: true }).waitFor();
				await delay(500);
				await producer.command('start 64 64 0 0 1');
				await displayed(view, blue);
				await view.locator('.message').waitFor({ state: 'hidden' });
				await delay(350);
				if (labels[2]) {
					for (const label of [labels[2], labels[1], labels[3], labels[1]]) {
						await select(label);
						const { id, channel } = surfaceTargets.get(label);
						await page.locator(`[data-panel-id="${panel}"][data-surface-target="${id}"]`).waitFor();
						await displayed(view, channel);
						assert.equal(await view.getAttribute('data-preview-view'), viewId);
					}
				}
				assert.equal(await view.getAttribute('data-preview-view'), viewId);
				await select(labels[0]);
				await displayed(view, red);
				await delay(150);
				await writeFile(path.join(directory, 'complete'), 'complete');
				assert.equal(await exited, 0, `${panel} native handoff failed in ${directory}`);
				const pixels = JSON.parse(await readFile(path.join(directory, 'pixels.json'), 'utf8'));
				assert.equal(pixels.result, 'pass');
				console.log(JSON.stringify({ panel, directory, viewId, pixels }));
			} finally {
				if (observer.exitCode === null) observer.kill();
				await exited;
			}
			await producer.command('stop');
		}
	} finally {
		await producer?.close();
	}
}

async function displayed(view, reference) {
	await view.locator('.unavailable').waitFor({ state: 'hidden' });
	await view.and(view.page().locator('[data-preview-installed="true"]')).waitFor();
	const deadline = Date.now() + 5000;
	while (true) {
		const key = JSON.parse(await view.getAttribute('data-preview-source'));
		if (key.reference?.channel === reference.channel && key.reference?.media === reference.media)
			return;
		assert(Date.now() < deadline, `Preview source did not change to ${reference.channel}`);
		await delay(25);
	}
}

async function waitForStart(directory) {
	const file = path.join(directory, 'capture-started');
	const deadline = Date.now() + 5000;
	while (!(await stat(file).catch(() => null))) {
		assert(Date.now() < deadline, `Native capture did not start in ${directory}`);
		await delay(25);
	}
}
