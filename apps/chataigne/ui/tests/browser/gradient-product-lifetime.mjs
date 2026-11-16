import assert from 'node:assert/strict';
import path from 'node:path';

export async function verifyGradientProductLifetime({
	page,
	post,
	snapshot,
	intent,
	wait,
	emit,
	evidence,
	primary,
	formula
}) {
	let stage = 'two real locked Gradient Inspectors';
	const outliner = page.locator('.outliner-panel');
	const panel = (id) => page.locator(`.inspector[data-panel-id="${id}"]`);
	const tab = (id) => page.locator(`.gc-dv-tab[data-content-panel-id="${id}"]`);
	const editor = (current) =>
		current.getByRole('application', { name: 'Gradient editor', exact: true });
	const selected = (id) =>
		wait(
			() => outliner.locator(`[data-tree-node-id="${id}"]`).getAttribute('aria-selected'),
			(value) => value === 'true'
		);
	async function target(id, uuid) {
		await tab(id).click();
		await wait(
			() => panel(id).getAttribute('data-inspected-uuid'),
			(value) => value === uuid
		);
	}
	async function lock(id) {
		await tab(id).getByRole('button', { name: 'Lock panel content', exact: true }).click();
		assert.equal(
			await tab(id)
				.getByRole('button', { name: 'Unlock panel content', exact: true })
				.getAttribute('aria-pressed'),
			'true'
		);
	}
	async function contents(gradient) {
		const { nodes, history } = await snapshot(gradient.node_id);
		const parents = new Map(nodes.flatMap((node) => (node.children ?? []).map((id) => [id, node])));
		const byId = new Map(nodes.map((node) => [node.node_id, node]));
		const key = (node) =>
			node.node_id === gradient.node_id
				? '$gradient'
				: `${key(parents.get(node.node_id))}/${node.node_type === 'gradient_stop' ? node.uuid : node.decl_id}`;
		return {
			history,
			state: nodes
				.map((node) => ({
					path: key(node),
					type: node.node_type,
					// Type/declaration help is projected schema data; this fixture authors no descriptions.
					meta: {
						...node.meta,
						description: undefined,
						declared_description_key: undefined,
						presentation: {
							...node.meta.presentation,
							collapsed: node.meta.presentation?.collapsed ?? false
						}
					},
					value: node.data.param?.value ?? null,
					children: (node.children ?? []).map((id) => key(byId.get(id)))
				}))
				.sort((a, b) => a.path.localeCompare(b.path))
		};
	}
	async function down(id, stop) {
		const surface = editor(panel(id));
		await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
		const marker = surface.locator(`.gradient-stop-handle[data-node-id="${stop.node_id}"]`);
		await marker.scrollIntoViewIfNeeded();
		const box = await marker.boundingBox();
		assert(box);
		const original = await surface.elementHandle();
		await surface.evaluate((node) =>
			node.addEventListener(
				'pointerdown',
				(event) => {
					node.dataset.testLifetimePointer = String(event.pointerId);
				},
				{ once: true, capture: true }
			)
		);
		await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
		await page.mouse.down();
		await selected(stop.node_id);
		assert(await surface.evaluate((node, original) => node === original, original));
		assert(
			await surface.evaluate((node) =>
				node.hasPointerCapture(Number(node.dataset.testLifetimePointer))
			)
		);
		await surface
			.locator(`.gradient-stop-editor .node-inspector[data-node-id="${stop.node_id}"]`)
			.waitFor();
		return surface;
	}
	async function press(id, stop) {
		try {
			await down(id, stop);
		} finally {
			await page.mouse.up();
		}
	}
	try {
		const mainId = await page.locator('.inspector').getAttribute('data-panel-id');
		assert(mainId);
		await target(mainId, primary.gradient.uuid);
		await lock(mainId);
		const created = (
			await intent({
				kind: 'createUserItem',
				parent: formula.node_id,
				node_type: 'alchemist_anode:gradient_sampler',
				label: 'Peer Gradient Sampler'
			})
		).created_node;
		assert(created);
		const fields = (await snapshot(created.node_id)).nodes;
		const gradient = fields.find((node) => node.node_type === 'gradient');
		assert(gradient);
		const stop = fields.find((node) => node.node_type === 'gradient_stop');
		assert(stop);
		const group = tab('formula-library').locator(
			'xpath=ancestor::*[contains(@class,"dv-groupview")][1]'
		);
		await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
		await page
			.getByRole('menuitem')
			.filter({ has: page.getByText('Inspector', { exact: true }) })
			.click();
		const tabs = page.locator('.gc-dv-tab[data-panel-type="inspector"]');
		await wait(
			() => tabs.count(),
			(count) => count === 2
		);
		const peerId = await tabs.evaluateAll(
			(nodes, main) =>
				nodes.find((node) => node.dataset.contentPanelId !== main)?.dataset.contentPanelId,
			mainId
		);
		assert(peerId);
		await outliner.locator('input.outliner-search').fill('Peer Gradient Sampler');
		await outliner.getByRole('button', { name: 'Peer Gradient Sampler', exact: true }).click();
		const chain = [gradient];
		for (let child = gradient; child.node_id !== created.node_id;) {
			child = fields.find((node) => node.children?.includes(child.node_id));
			assert(child);
			chain.unshift(child);
		}
		for (const node of chain) {
			const title = panel(peerId)
				.locator(`.node-inspector[data-node-id="${node.node_id}"]`)
				.first()
				.locator('.node-title')
				.first();
			if ((await title.count()) && (await title.getAttribute('aria-checked')) === 'false')
				await title.click();
		}
		await panel(peerId)
			.locator(`.node-inspector[data-node-id="${gradient.node_id}"]`)
			.first()
			.getByTitle('Select Node in Outliner', { exact: true })
			.first()
			.click();
		await outliner.locator('input.outliner-search').fill('');
		await target(peerId, gradient.uuid);
		await lock(peerId);
		await target(mainId, primary.gradient.uuid);
		const beforeMain = await contents(primary.gradient),
			beforePeer = await contents(gradient);
		await press(mainId, primary.stop);
		await press(peerId, stop);
		assert.equal(await editor(panel(mainId)).locator('.gradient-stop-handle.selected').count(), 1);
		await editor(panel(mainId)).focus();
		await page.keyboard.press('Escape');
		await selected(primary.gradient.node_id);
		assert.equal(await editor(panel(mainId)).locator('.gradient-stop-handle.selected').count(), 0);
		assert.equal(await editor(panel(peerId)).locator('.gradient-stop-handle.selected').count(), 1);
		await editor(panel(peerId)).focus();
		await page.keyboard.press('Escape');
		await selected(gradient.node_id);
		assert.equal(await editor(panel(peerId)).locator('.gradient-stop-handle.selected').count(), 0);
		stage = 'local color-tool Escape across locked Inspectors';
		await editor(panel(peerId))
			.locator(`.gradient-stop-handle[data-node-id="${stop.node_id}"]`)
			.dblclick();
		const popup = panel(peerId).getByRole('dialog', { name: 'Stop color', exact: true });
		await popup.waitFor();
		await editor(panel(mainId)).focus();
		await page.keyboard.press('Escape');
		await selected(primary.gradient.node_id);
		assert(await popup.isVisible());
		await editor(panel(peerId)).focus();
		await page.keyboard.press('Escape');
		await popup.waitFor({ state: 'detached' });
		await selected(primary.gradient.node_id);
		assert.deepEqual(await contents(primary.gradient), beforeMain);
		assert.deepEqual(await contents(gradient), beforePeer);
		stage = 'connection replacement retires captured input';
		const surface = await down(mainId, primary.stop);
		const track = await surface.locator('.gradient-track').boundingBox();
		assert(track);
		const sockets = await page.evaluate(() => window.__gradientProductSockets.length);
		await page.evaluate(() => {
			const socket = window.__gradientProductSockets.findLast(
				(socket) => socket.readyState === WebSocket.OPEN && socket.url.includes('/api/ui/ws')
			);
			if (!socket) throw Error('The current workbench socket is missing');
			socket.close(4001, 'Gradient owned reconnect qualification');
		});
		await page.waitForFunction(
			(count) =>
				window.__gradientProductSockets.length > count &&
				window.__gradientProductSockets.some((socket) => socket.readyState === WebSocket.OPEN),
			sockets
		);
		await wait(
			() =>
				surface.evaluate((node) =>
					node.hasPointerCapture(Number(node.dataset.testLifetimePointer))
				),
			(value) => value === false
		);
		await page.mouse.move(track.x + track.width * 0.8, track.y + track.height / 2, { steps: 5 });
		await page.mouse.up();
		await target(mainId, primary.gradient.uuid);
		await target(peerId, gradient.uuid);
		assert.deepEqual(await contents(primary.gradient), beforeMain);
		assert.deepEqual(await contents(gradient), beforePeer);
		stage = 'layout reload retains each locked owner';
		await page.waitForFunction(
			({ mainId, peerId, first, second }) =>
				Object.entries(localStorage).some(([key, value]) => {
					if (!key.endsWith('layout.v1')) return false;
					const panels = JSON.parse(value).payload?.panels;
					return (
						panels?.[mainId]?.params?.__gc_panel_state?.inspectorLockedNodeUuid === first &&
						panels?.[peerId]?.params?.__gc_panel_state?.inspectorLockedNodeUuid === second
					);
				}),
			{ mainId, peerId, first: primary.gradient.uuid, second: gradient.uuid }
		);
		await page.reload();
		await target(mainId, primary.gradient.uuid);
		await target(peerId, gradient.uuid);
		assert.deepEqual(await contents(primary.gradient), beforeMain);
		assert.deepEqual(await contents(gradient), beforePeer);
		const project = path.join(evidence, 'independent-gradients.noisette');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		stage = 'New during accepted live input fences both locked documents';
		const active = await down(mainId, primary.stop);
		const live = await active.locator('.gradient-track').boundingBox();
		assert(live);
		await page.mouse.move(live.x + live.width * 0.6, live.y + live.height / 2, { steps: 6 });
		await wait(
			async () =>
				(await snapshot(primary.position.node_id)).nodes.find(
					(node) => node.node_id === primary.position.node_id
				).data.param.value.Float,
			(value) => Math.abs(value - 0.6) < 0.015
		);
		assert.equal((await post('/api/ui/project-new', {})).ok, true);
		for (const id of [mainId, peerId]) {
			await wait(
				() => panel(id).getAttribute('data-inspected-uuid'),
				(value) => value === ''
			);
			assert.equal(await editor(panel(id)).count(), 0);
		}
		await page.mouse.move(live.x + live.width * 0.9, live.y + live.height / 2, { steps: 5 });
		await page.mouse.up();
		const fresh = await snapshot();
		assert(
			!fresh.nodes.some((node) => node.uuid === primary.anode.uuid || node.uuid === created.uuid)
		);
		assert.equal(fresh.history.undo_len, 0);
		assert.equal(fresh.history.active_edit_session, false);
		stage = 'Load restores authored gradients without stale locked-node reuse';
		const loaded = await post('/api/ui/project-load', { path: project });
		assert.equal(path.resolve(loaded.path), project);
		assert.equal(loaded.recovery ?? null, null);
		const graph = await snapshot();
		for (const [id, anodeUuid, previousGradient, before] of [
			[mainId, primary.anode.uuid, primary.gradient, beforeMain],
			[peerId, created.uuid, gradient, beforePeer]
		]) {
			const owner = graph.nodes.find((node) => node.uuid === anodeUuid);
			assert(owner);
			const current = (await snapshot(owner.node_id)).nodes.find(
				(node) => node.node_type === 'gradient'
			);
			assert(current);
			assert.deepEqual((await contents(current)).state, before.state);
			// Declared UUIDs may rematerialize; a lock may only resolve its exact document/UUID.
			await target(id, current.uuid === previousGradient.uuid ? current.uuid : '');
			assert.equal(
				await tab(id).getByRole('button', { name: 'Unlock panel content', exact: true }).count(),
				1
			);
		}
		assert.equal(graph.history.undo_len, 0);
		assert.equal(graph.history.redo_len, 0);
		assert.equal(graph.history.active_edit_session, false);
		emit({
			kind: 'gradient-product-lifetime',
			passed: true,
			twoRealLockedPanels: true,
			sharedSelectionBeforeRelease: true,
			localColorEscape: true,
			captureRetiredOnReconnect: true,
			layoutReload: true,
			newDuringAcceptedDrag: true,
			exactAuthoredLoad: true,
			lockedDeclaredIdentity: 'exact document/UUID or unavailable; no positional retarget',
			nativePixels: 'unqualified'
		});
	} catch (error) {
		throw new Error(`${stage}: ${error.message}`, { cause: error });
	}
}
