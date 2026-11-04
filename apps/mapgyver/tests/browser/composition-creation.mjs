import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { createPreviewOutputObserver } from './preview-output-observer.mjs';
import { observeRejectedRequests } from './intent-observer.mjs';
const { chromium } = createRequire(import.meta.url)('playwright-core');

export async function verifyCompositionCreation({
	base,
	edit,
	post,
	directory,
	page: nativePage = null
}) {
	assert((await post('/api/ui/project-new', {})).ok);
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Composition',
		label: 'Canvas owner'
	});
	await edit({
		kind: 'ConfigureMediaResolution',
		media: reference.media,
		size: { kind: 'Fixed', extent: { width: 200, height: 100 } }
	});
	const browser = nativePage ? null : await chromium.launch({ headless: true });
	const page = nativePage ?? (await browser.newPage({ viewport: { width: 1600, height: 1100 } }));
	page.setDefaultTimeout(7000);
	const errors = [];
	page.on('pageerror', (error) => errors.push(String(error)));
	const rejected = observeRejectedRequests(page, () => 'Composition canvas creation');
	const main = page.locator('[data-panel-id="composition"]');
	let history;
	async function inspect() {
		const ack = await post('/api/ui/intent', {
			kind: 'invokeAppOperation',
			namespace: 'mapgyver.media',
			operation: 'edit',
			payload: { kind: 'Composition', reference, edit: { kind: 'Inspect' } }
		});
		assert(ack.success, JSON.stringify(ack));
		history = ack.history;
		return ack.result.value.snapshot;
	}
	async function wait(read, test) {
		const deadline = Date.now() + 8000;
		for (;;) {
			const value = await read();
			if (test(value)) return value;
			assert(Date.now() < deadline, JSON.stringify(value));
			await delay(25);
		}
	}
	async function selectOwner() {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$|^Media Manager$/ })
			.click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Canvas owner', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.click();
		await wait(
			() => main.getAttribute('data-composition-channel'),
			(value) => value === reference.channel
		);
		await wait(
			() => main.getAttribute('data-composition-loading'),
			(value) => value === 'false'
		);
	}
	try {
		if (!nativePage) await page.goto(base);
		await selectOwner();
		const manager = await edit({ kind: 'InspectManager' });
		assert.equal(manager.snapshot.media.length, 1, 'no source catalog object is required');
		const { page: catalog } = await edit({
			kind: 'InspectCompositionEditor',
			query: {
				selected: reference.media,
				document: null,
				locked: null,
				instance: null,
				start: 0,
				count: 128
			}
		});
		assert.equal(catalog.media_types.length, 6);
		const created = [];
		for (const choice of catalog.media_types) {
			await selectOwner();
			const before = await inspect(),
				undo = history.undo_len;
			await wait(
				() => main.getAttribute('data-composition-busy'),
				(value) => value === 'false'
			);
			const zone = await main.locator('.active-zone').boundingBox();
			assert(zone);
			await page.mouse.click(zone.x + zone.width * 0.4, zone.y + zone.height * 0.6, {
				button: 'right'
			});
			for (const type of catalog.media_types)
				await page.getByRole('menuitem', { name: type.label, exact: true }).waitFor();
			await page.getByRole('menuitem', { name: choice.label, exact: true }).click();
			const after = await wait(
				inspect,
				(value) => value.instances.length === before.instances.length + 1
			);
			const child = after.instances.at(-1);
			assert.equal(child.label, choice.label);
			assert(Math.abs(child.definition.transform.position[0] - 80) < 1);
			assert(Math.abs(child.definition.transform.position[1] - 60) < 1);
			assert.equal(history.undo_len, undo + 1, 'one menu action creates one Undo');
			await wait(
				() => main.getAttribute('data-composition-selected'),
				(value) => value === child.reference.instance
			);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			assert.deepEqual(
				await wait(inspect, (value) => value.instances.length === before.instances.length),
				before
			);
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			assert.deepEqual(
				await wait(inspect, (value) => value.instances.length === after.instances.length),
				after
			);
			created.push({ kind: choice.media_kind, ...child });
		}
		const solid = created.find((child) => child.kind === 'SolidColor');
		await edit({
			kind: 'ConfigureChannel',
			reference: solid.definition.source,
			source: { kind: 'SolidColor', rgba: [1, 0, 0, 1] }
		});
		const output = nativePage
			? await createPreviewOutputObserver(post, path.join(directory, 'created-output'), page)
			: null;
		if (output) {
			await output.demand(reference);
			await output.observePixels('owned-solid-red', [255, 0, 0, 255]);
		}
		await selectOwner();
		const beforeCopy = await inspect();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$|^Media Manager$/ })
			.click();
		await page
			.locator('[data-panel-id="media-manager"]')
			.getByRole('button', { name: 'Solid Color', exact: true })
			.click();
		await page
			.getByRole('tab')
			.filter({ hasText: /^Composition(?::|$)/ })
			.click();
		await wait(
			() => main.getAttribute('data-composition-selected'),
			(value) => value === solid.reference.instance
		);
		await main.getByRole('button', { name: 'Copy instance', exact: true }).click();
		const afterCopy = await wait(
			inspect,
			(value) => value.instances.length === beforeCopy.instances.length + 1
		);
		const copied = afterCopy.instances.find(
			(child) =>
				!beforeCopy.instances.some((old) => old.reference.instance === child.reference.instance)
		).reference;
		await edit({
			kind: 'Composition',
			reference,
			edit: { kind: 'ReorderInstance', instance: copied.instance, after: null }
		});
		assert.equal((await inspect()).instances[0].reference.instance, copied.instance);
		await wait(
			() => main.getAttribute('data-composition-selected'),
			(value) => value === copied.instance
		);
		await main.getByRole('button', { name: 'Remove instance', exact: true }).click();
		assert.deepEqual(
			await wait(inspect, (value) => value.instances.length === beforeCopy.instances.length),
			beforeCopy
		);
		const project = path.join(directory, 'canvas-owned.mapgyver');
		assert((await post('/api/ui/project-save', { path: project })).ok);
		assert((await post('/api/ui/project-new', {})).ok);
		const loaded = await post('/api/ui/project-load', { path: project });
		assert.equal(path.resolve(loaded.path), project);
		assert.equal(loaded.recovery ?? null, null);
		await selectOwner();
		assert.deepEqual(await inspect(), beforeCopy);
		if (output) {
			await output.observePixels('reloaded-owned-solid-red', [255, 0, 0, 255]);
			await output.finish();
		}
		assert.deepEqual(errors, []);
		assert.deepEqual(rejected, []);
		await page.screenshot({
			path: `target/m117-composition-creation-${nativePage ? 'native' : 'headless'}.png`
		});
		console.log(
			JSON.stringify({
				result: 'pass',
				native: !!nativePage,
				media_types: created.map((child) => child.kind),
				canvasPlacement: [80, 60],
				singleGestureUndoRedo: true,
				copyReorderDelete: true,
				projectIdentity: true,
				nativeOutputPixels: output?.observations ?? [],
				errors,
				rejected
			})
		);
	} catch (error) {
		await page.screenshot({ path: 'target/m117-composition-creation-failure.png' }).catch(() => {});
		console.error(
			JSON.stringify({
				errors,
				rejected,
				body: (await page.locator('body').innerText()).slice(-3000)
			})
		);
		throw error;
	} finally {
		await browser?.close();
	}
}
