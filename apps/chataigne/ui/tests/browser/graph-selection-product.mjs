// Ordinary bundled workbench. Use <binary> --state-machine; add --independent-panels for that gate.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { nativeWorkbench } from './native-workbench.mjs';

const rejected = [];
const stateMachine = process.argv.includes('--state-machine');
const independentPanels = process.argv.includes('--independent-panels');
assert(!independentPanels || stateMachine, 'Independent graph checks require --state-machine');
const domain = stateMachine ? 'state-machine' : 'alchemist';
await nativeWorkbench({
	name: `m14-${domain}-${independentPanels ? 'independent-' : ''}selection-product`,
	async observePage({ page }) {
		page.on('websocket', (socket) =>
			socket.on('framereceived', ({ payload }) => {
				try {
					const message = JSON.parse(String(payload));
					if (message.kind === 'control' && message.update?.phase === 'rejected')
						rejected.push(message.update);
				} catch {}
			})
		);
	},
	async exercise({ page, backend, evidence, emit }) {
		page.setDefaultTimeout(7000);
		let token,
			stage = `create actual ${domain} nodes`;
		async function post(route, body) {
			const response = await fetch(backend + route, {
				method: 'POST',
				headers: {
					'Content-Type': 'application/json',
					...(token ? { Authorization: `Bearer ${token}` } : {})
				},
				body: JSON.stringify(body),
				signal: AbortSignal.timeout(7000)
			});
			const result = await response.json();
			assert(response.ok, `${stage}: ${route}: ${JSON.stringify(result)}`);
			return result;
		}
		const snapshot = () => post('/api/ui/snapshot', { scope: 'wholeGraph' });
		async function intent(value) {
			const ack = await post('/api/ui/intent', value);
			assert(ack.success, `${stage}: ${JSON.stringify(ack)}`);
			return ack;
		}
		async function wait(read, predicate) {
			const deadline = Date.now() + 7000;
			for (;;) {
				const value = await read();
				if (predicate(value)) return value;
				assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
				await delay(20);
			}
		}
		token = (await post('/api/ui/session', {})).token;
		const manager = (await snapshot()).nodes.find(
			(node) =>
				node.node_type === (stateMachine ? 'state_machine_manager' : 'alchemist_formula_library')
		);
		assert(manager);
		const owner = stateMachine
			? manager
			: (
					await intent({
						kind: 'createUserItem',
						parent: manager.node_id,
						node_type: 'alchemist_formula',
						label: 'Graph selection acceptance'
					})
				).created_node;
		assert(owner);
		const ownerLabel = stateMachine ? manager.meta.label : 'Graph selection acceptance';
		const nodes = [];
		for (const [label, x] of [
			['Graph A', 2],
			['Graph B', 30]
		]) {
			const created = (
				await intent({
					kind: 'createUserItem',
					parent: owner.node_id,
					node_type: stateMachine ? 'state' : 'alchemist_anode:gradient_sampler',
					label
				})
			).created_node;
			assert(created);
			const fields = (await snapshot()).nodes;
			const definition = fields.find((node) => node.node_id === created.node_id);
			const position = fields.find(
				(node) => definition.children.includes(node.node_id) && node.decl_id === 'position'
			);
			assert(position);
			await intent({
				kind: 'setParam',
				node: position.node_id,
				value: { Vec2: [x, 2] },
				behaviour: 'Append'
			});
			nodes.push({ ...created, label });
		}
		stage = `open ordinary ${domain} workbench panel`;
		const outliner = page.locator('.outliner-panel');
		await outliner.locator('input.outliner-search').fill(ownerLabel);
		await outliner.getByRole('button', { name: ownerLabel, exact: true }).click();
		if (stateMachine) {
			await page.locator('.gc-dv-tab[data-content-panel-id="state-machine"]').click();
		} else {
			const group = page
				.locator('.gc-dv-tab[data-content-panel-id="formula-library"]')
				.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
			await group.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
			await page
				.getByRole('menuitem')
				.filter({ has: page.getByText('Alchemist Editor', { exact: true }) })
				.click();
		}
		const graph = page.locator(
			stateMachine ? '.state-machine-panel .graph-canvas' : '.alchemist-editor-panel .graph-canvas'
		);
		await graph.waitFor();
		if (!stateMachine) await page.getByTitle('Hide properties', { exact: true }).click();
		await graph.getByRole('button', { name: 'Home', exact: true }).click();
		const inspector = page.locator('.inspector[data-panel-id="inspector"]');
		const inspected = (uuid) =>
			wait(
				() => inspector.getAttribute('data-inspected-uuid'),
				(value) => value === uuid
			);
		await inspected(owner.uuid);
		const original = await graph.elementHandle();
		assert(original);
		const node = (current) => graph.locator(`article.node[data-node-id="${current.node_id}"]`);
		const [first, second] = nodes;
		await node(first).waitFor();
		await node(second).waitFor();
		const project = path.join(evidence, 'graph-selection.noisette');
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		const authored = await readFile(project, 'utf8');
		const history = (await snapshot()).history;
		const checks = [];
		async function press(current, canvas = graph, previous = original) {
			const target = canvas.locator(`article.node[data-node-id="${current.node_id}"]`);
			const box = await target.locator('.node-title-label').boundingBox();
			assert(box);
			await canvas.evaluate((element) =>
				element.addEventListener(
					'pointerdown',
					(event) => {
						element.dataset.selectionPointer = String(event.pointerId);
					},
					{ once: true, capture: true }
				)
			);
			await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
			await page.mouse.down();
			await inspected(current.uuid);
			assert(await target.evaluate((element) => element.classList.contains('selected')));
			assert(await canvas.evaluate((element, old) => element === old, previous));
			assert(
				await canvas.evaluate((element) =>
					element.hasPointerCapture(Number(element.dataset.selectionPointer))
				),
				`${stage}: selection preserves owned capture before release`
			);
		}
		stage = 'hover and actual Inspector selection before release';
		await node(first).hover();
		await inspected(owner.uuid);
		assert.equal(await graph.locator('.resize-handle').count(), 0);
		await press(first);
		assert.equal(await node(first).locator('.resize-handle').count(), 1);
		await page.mouse.up();
		checks.push({ stage, node: first.uuid, canvasRetained: true, capturedBeforeRelease: true });
		stage = 'background down inspects the edited owner';
		const background = await graph.evaluate((element) => {
			const box = element.getBoundingClientRect();
			for (const fy of [0.85, 0.7, 0.5])
				for (const fx of [0.85, 0.65, 0.45]) {
					const x = box.x + box.width * fx,
						y = box.y + box.height * fy,
						target = document.elementFromPoint(x, y);
					if (
						target &&
						element.contains(target) &&
						!target.closest('article.node, button, input, [role="toolbar"]')
					)
						return { x, y };
				}
			return null;
		});
		assert(background);
		await page.mouse.move(background.x, background.y);
		await page.mouse.down();
		await inspected(owner.uuid);
		assert.equal(await graph.locator('article.node.selected, .resize-handle').count(), 0);
		await page.mouse.up();
		await inspected(owner.uuid);
		checks.push({ stage, owner: owner.uuid, clearedBeforeRelease: true });
		stage = 'captured Escape stays tool-owned; idle Escape inspects owner';
		await press(second);
		await page.keyboard.press('Escape');
		await inspected(second.uuid);
		assert(
			await graph.evaluate(
				(element) => !element.hasPointerCapture(Number(element.dataset.selectionPointer))
			)
		);
		await page.mouse.move(background.x, background.y);
		await page.mouse.up();
		await page.keyboard.press('Escape');
		await inspected(owner.uuid);
		assert.equal(await graph.locator('article.node.selected, .resize-handle').count(), 0);
		checks.push({ stage, toolThenOwner: true });
		stage = 'selection preserves authored state and history';
		const after = (await snapshot()).history;
		assert.equal(after.undo_len, history.undo_len);
		assert.equal(after.redo_len, history.redo_len);
		assert.equal(after.active_edit_session, false);
		assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
		assert.equal(await readFile(project, 'utf8'), authored);
		assert.deepEqual(rejected, []);
		checks.push({ stage, exactSavedDocument: true, unchangedHistory: true });
		if (independentPanels) {
			stage = 'independent graph panels retain their domain owners';
			const library = (await snapshot()).nodes.find(
				(current) => current.node_type === 'alchemist_formula_library'
			);
			assert(library);
			const peerFormula = (
				await intent({
					kind: 'createUserItem',
					parent: library.node_id,
					node_type: 'alchemist_formula',
					label: 'Independent graph Formula'
				})
			).created_node;
			const peer = (
				await intent({
					kind: 'createUserItem',
					parent: peerFormula.node_id,
					node_type: 'alchemist_anode:gradient_sampler',
					label: 'Independent graph node'
				})
			).created_node;
			await outliner.locator('input.outliner-search').fill('Independent graph Formula');
			await outliner
				.getByRole('button', { name: 'Independent graph Formula', exact: true })
				.click();
			const peerGroup = page
				.locator('.gc-dv-tab[data-content-panel-id="logger"]')
				.locator('xpath=ancestor::*[contains(@class,"dv-groupview")][1]');
			await peerGroup.getByRole('button', { name: 'Add panel in this group', exact: true }).click();
			await page
				.getByRole('menuitem')
				.filter({ has: page.getByText('Alchemist Editor', { exact: true }) })
				.click();
			const peerGraph = page.locator('.alchemist-editor-panel .graph-canvas');
			await peerGraph.waitFor();
			await page.getByTitle('Hide properties', { exact: true }).click();
			await peerGraph.getByRole('button', { name: 'Home', exact: true }).click();
			const peerOriginal = await peerGraph.elementHandle();
			assert(peerOriginal);
			const independentHistory = (await snapshot()).history;
			assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
			const independentDocument = await readFile(project, 'utf8');
			await press(first);
			await page.mouse.up();
			assert.equal(await peerGraph.locator('article.node.selected, .resize-handle').count(), 0);
			assert(await peerGraph.locator(`article.node[data-node-id="${peer.node_id}"]`).isVisible());
			await press(peer, peerGraph, peerOriginal);
			await page.mouse.up();
			assert.equal(await graph.locator('article.node.selected, .resize-handle').count(), 0);
			assert(await node(first).isVisible());
			const focus = await peerGraph.evaluate((element) => ({
				owned: element.contains(document.activeElement),
				tag: document.activeElement?.tagName,
				className: document.activeElement?.className
			}));
			assert(focus.owned, `normal release retains peer keyboard focus: ${JSON.stringify(focus)}`);
			await page.keyboard.press('Escape');
			await inspected(peerFormula.uuid);
			await press(first);
			await page.mouse.up();
			await page.keyboard.press('Escape');
			await inspected(owner.uuid);
			assert(await peerGraph.evaluate((element, old) => element === old, peerOriginal));
			checks.push({
				stage,
				independentOwners: true,
				sharedInspector: true,
				retainedCanvases: true
			});
			stage = 'offscreen State remains available to its actual Inspector';
			await press(first);
			await page.mouse.up();
			const viewport = await graph.boundingBox();
			assert(viewport);
			await page.mouse.move(viewport.x + viewport.width * 0.9, viewport.y + viewport.height * 0.9);
			await page.mouse.down({ button: 'middle' });
			await page.mouse.move(viewport.x - viewport.width * 2, viewport.y - viewport.height * 2, {
				steps: 4
			});
			await page.mouse.up({ button: 'middle' });
			await wait(
				async () => {
					if ((await node(first).count()) === 0) return true;
					const box = await node(first).boundingBox();
					return (
						box === null ||
						box.x + box.width < viewport.x ||
						box.y + box.height < viewport.y ||
						box.x > viewport.x + viewport.width ||
						box.y > viewport.y + viewport.height
					);
				},
				(value) => value
			);
			await inspected(first.uuid);
			const afterNavigation = (await snapshot()).history;
			assert.equal(afterNavigation.undo_len, independentHistory.undo_len);
			assert.equal(afterNavigation.redo_len, independentHistory.redo_len);
			assert.equal(afterNavigation.active_edit_session, false);
			assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
			assert.equal(await readFile(project, 'utf8'), independentDocument);
			checks.push({ stage, offscreen: true, sameInspector: true, noAuthoredNavigation: true });
			stage = 'offscreen Inspector honors rename permission';
			const definition = (await snapshot()).nodes.find((current) => current.uuid === first.uuid);
			await intent({
				kind: 'patchMeta',
				node: first.node_id,
				patch: { user_permissions: { ...definition.meta.user_permissions, can_edit_name: false } }
			});
			const heading = inspector.locator('.inspector-header .title-text');
			await wait(
				() => heading.getAttribute('class'),
				(value) => !value.includes('name-changeable')
			);
			const protectedHistory = (await snapshot()).history;
			await heading.dblclick();
			assert.equal(await inspector.locator('.title-input').count(), 0);
			assert.equal((await snapshot()).history.undo_len, protectedHistory.undo_len);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(
				() => heading.getAttribute('class'),
				(value) => value.includes('name-changeable')
			);
			checks.push({ stage, rejectedWithoutWrite: true });
			stage = 'offscreen typed Inspector edit and exact Undo';
			await heading.dblclick();
			const rename = inspector.locator('.title-input');
			await rename.fill('Offscreen State edit');
			await rename.press('Enter');
			await wait(snapshot, (value) =>
				value.nodes.some(
					(current) => current.uuid === first.uuid && current.meta.label === 'Offscreen State edit'
				)
			);
			assert.equal((await snapshot()).history.undo_len, independentHistory.undo_len + 1);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(snapshot, (value) =>
				value.nodes.some(
					(current) => current.uuid === first.uuid && current.meta.label === first.label
				)
			);
			assert.equal((await snapshot()).history.undo_len, independentHistory.undo_len);
			assert.equal((await post('/api/ui/project-save', { path: project })).ok, true);
			assert.equal(await readFile(project, 'utf8'), independentDocument);
			assert.deepEqual(rejected, []);
			checks.push({ stage, oneHistoryStep: true, exactRestoration: true });
		}
		emit({ kind: 'graph-domain-inspector-selection', domain, checks, nativePixels: false });
	}
});
