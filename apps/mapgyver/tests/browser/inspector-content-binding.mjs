import assert from 'node:assert/strict';
import path from 'node:path';
import { mkdir } from 'node:fs/promises';
import { contentLock } from './content-lock.mjs';

export async function verifyInspectorContentBinding({ page, edit, post, directory }) {
	const refs = [];
	for (const label of ['Binding A', 'Binding B']) {
		refs.push((await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label })).reference);
	}
	const inspector = (id) => page.locator(`.inspector[data-panel-id="${id}"]`);
	const main = inspector('inspector');
	const manager = page.getByRole('region', { name: 'Medias', exact: true });
	async function select(label) {
		await page
			.getByRole('tab')
			.filter({ hasText: /^Medias$/ })
			.click();
		await manager.getByRole('button', { name: label, exact: true }).click();
	}
	async function target(panel, uuid) {
		await page.waitForFunction(
			({ id, uuid }) =>
				document
					.querySelector(`.inspector[data-panel-id="${id}"]`)
					?.getAttribute('data-inspected-uuid') === uuid,
			{ id: await panel.getAttribute('data-panel-id'), uuid }
		);
	}
	await select('Binding A');
	await target(main, refs[0].media);
	await (await contentLock(main)).press('Space');
	await select('Binding B');
	await target(main, refs[0].media);
	const group = page
		.getByRole('tab')
		.filter({ hasText: /^Composition(?::|$)/ })
		.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
	await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
	await page
		.getByRole('menuitem')
		.filter({ has: page.getByText('Inspector', { exact: true }) })
		.click();
	const peerId = await page
		.locator('.inspector[data-panel-id]')
		.evaluateAll(
			(nodes) => nodes.find((node) => node.dataset.panelId !== 'inspector')?.dataset.panelId
		);
	assert(peerId);
	const peer = inspector(peerId);
	await target(peer, refs[1].media);
	await (await contentLock(peer)).click();
	if (process.env.MAPGYVER_BINDING_DIRECTORY) {
		await mkdir(process.env.MAPGYVER_BINDING_DIRECTORY, { recursive: true });
		await page.screenshot({
			path: path.join(process.env.MAPGYVER_BINDING_DIRECTORY, 'independent-inspectors.png')
		});
	}
	await select('Binding A');
	await target(main, refs[0].media);
	await target(peer, refs[1].media);
	await edit({ kind: 'RemoveMedia', media: refs[0].media });
	await target(main, '');
	await contentLock(main, true);
	await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
	await target(main, refs[0].media);
	const project = path.join(directory, 'content-bindings.mapgyver');
	await post('/api/ui/project-save', { path: project });
	await page.waitForFunction(
		({ first, second, peerId }) =>
			Object.entries(localStorage).some(([key, value]) => {
				if (!key.endsWith('layout.v1')) return false;
				const panels = JSON.parse(value).payload?.panels;
				const a = panels?.inspector?.params?.__gc_panel_state;
				const b = panels?.[peerId]?.params?.__gc_panel_state;
				return (
					a?.inspectorLocked === true &&
					a.inspectorLockedNodeUuid === first &&
					b?.inspectorLocked === true &&
					b.inspectorLockedNodeUuid === second
				);
			}),
		{ first: refs[0].media, second: refs[1].media, peerId }
	);
	await page.reload();
	await target(main, refs[0].media);
	await target(peer, refs[1].media);
	await post('/api/ui/project-new', {});
	await target(main, '');
	await target(peer, '');
	await edit({ kind: 'CreateMedia', media_kind: 'SolidColor', label: 'New document identity' });
	await select('New document identity');
	await target(main, '');
	await target(peer, '');
	await post('/api/ui/project-load', { path: project });
	await target(main, refs[0].media);
	await target(peer, refs[1].media);
	await (await contentLock(main, true)).press('Enter');
	await select('Binding B');
	await target(main, refs[1].media);
	await page
		.locator(`.gc-dv-tab[data-content-panel-id="${peerId}"]`)
		.getByRole('button', { name: 'Close panel', exact: true })
		.click();
	for (const type of ['mediaManager', 'screenManager']) {
		const fixed = page.locator(`.gc-dv-tab[data-panel-type="${type}"]`);
		assert.equal(await fixed.count(), 1);
		assert.equal(await fixed.locator('.gc-dv-tab-binding:visible').count(), 0);
	}
	for (const name of ['Logger', 'Dashboard']) {
		await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText(name, { exact: true }) })
			.click();
		const fixed = page.locator(`.gc-dv-tab[data-panel-type="${name.toLowerCase()}"]`);
		await fixed.waitFor();
		assert.equal(await fixed.count(), 1);
		assert.equal(await fixed.locator('.gc-dv-tab-binding:visible').count(), 0);
		await fixed.getByRole('button', { name: 'Close panel', exact: true }).click();
	}
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'Inspector tab keyboard locks, independent targets, delete/Undo, layout reload, New/Load and reused-ID project fencing'
		})
	);
}
