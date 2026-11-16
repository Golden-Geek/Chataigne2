// The bundled Chataigne workbench and ordinary public transport own every UI/backend action.
import assert from 'node:assert/strict';
import { setTimeout as delay } from 'node:timers/promises';
import { nativeWorkbench } from './native-workbench.mjs';
import { verifyGradientProductLifetime } from './gradient-product-lifetime.mjs';

const rejected = [];
await nativeWorkbench({
	name: 'm14-gradient-product',
	async observePage({ page }) {
		await page.addInitScript(() => {
			window.__gradientProductSockets = [];
			const Original = window.WebSocket;
			window.WebSocket = class extends Original {
				constructor(...args) {
					super(...args);
					window.__gradientProductSockets.push(this);
				}
			};
		});
		page.on('websocket', (socket) =>
			socket.on('framereceived', ({ payload }) => {
				try {
					const frame = JSON.parse(String(payload));
					if (frame.kind === 'control' && frame.update?.phase === 'rejected')
						rejected.push(frame.update);
				} catch {}
			})
		);
	},
	async exercise({ page, backend, emit, evidence }) {
		page.setDefaultTimeout(7000);
		let stage = 'declared product Gradient';
		let token;
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
			const value = await response.json();
			assert(response.ok, `${stage}: ${route}: ${JSON.stringify(value)}`);
			return value;
		}
		const snapshot = (root) =>
			post('/api/ui/snapshot', {
				scope: root === undefined ? 'wholeGraph' : { subtree: { root, max_depth: 8 } }
			});
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
		try {
			token = (await post('/api/ui/session', {})).token;
			assert.equal(typeof token, 'string');
			const library = (await snapshot()).nodes.find(
				(node) => node.node_type === 'alchemist_formula_library'
			);
			assert(library);
			const formula = (
				await intent({
					kind: 'createUserItem',
					parent: library.node_id,
					node_type: 'alchemist_formula',
					label: 'Product Gradient qualification'
				})
			).created_node;
			assert(formula);
			const anode = (
				await intent({
					kind: 'createUserItem',
					parent: formula.node_id,
					node_type: 'alchemist_anode:gradient_sampler',
					label: 'Product Gradient Sampler'
				})
			).created_node;
			assert(anode);
			const fields = (await snapshot(anode.node_id)).nodes;
			const gradient = fields.find((node) => node.node_type === 'gradient');
			assert(gradient);
			const stops = fields.filter((node) => node.node_type === 'gradient_stop');
			assert.equal(stops.length, 2);
			const first = stops.find((stop) =>
				fields.some(
					(node) =>
						stop.children.includes(node.node_id) &&
						node.decl_id === 'position' &&
						node.data.param?.value.Float === 0
				)
			);
			assert(first);
			const position = fields.find(
				(node) => first.children.includes(node.node_id) && node.decl_id === 'position'
			);
			const state = async () =>
				(await snapshot(gradient.node_id)).nodes
					.map((node) => ({
						uuid: node.uuid,
						type: node.node_type,
						decl: node.decl_id,
						meta: node.meta,
						data: node.data,
						children: node.children
					}))
					.sort((a, b) => a.uuid.localeCompare(b.uuid));
			const history = async () => (await snapshot(gradient.node_id)).history;
			const value = async () =>
				(await snapshot(position.node_id)).nodes.find((node) => node.node_id === position.node_id)
					.data.param.value.Float;
			const idle = () => wait(history, (value) => !value.active_edit_session);
			stage = 'open the actual declared Gradient Inspector';
			const outliner = page.locator('.outliner-panel');
			await outliner.locator('input.outliner-search').fill('Product Gradient Sampler');
			await outliner.getByRole('button', { name: 'Product Gradient Sampler', exact: true }).click();
			const inspector = page.locator('.inspector');
			const chain = [gradient];
			for (let child = gradient; child.node_id !== anode.node_id;) {
				child = fields.find((node) => node.children?.includes(child.node_id));
				assert(child);
				chain.unshift(child);
			}
			for (const node of chain) {
				const section = inspector
					.locator(`.node-inspector[data-node-id="${node.node_id}"]`)
					.first();
				if (await section.count()) {
					const title = section.locator('.node-title').first();
					if ((await title.count()) && (await title.getAttribute('aria-checked')) === 'false')
						await title.click();
				}
			}
			const owned = inspector
				.locator(`.node-inspector[data-node-id="${gradient.node_id}"]`)
				.first();
			await owned.getByTitle('Select Node in Outliner', { exact: true }).first().click();
			await outliner.locator('input.outliner-search').fill('');
			const editor = inspector.getByRole('application', { name: 'Gradient editor', exact: true });
			await editor.waitFor();
			await editor.scrollIntoViewIfNeeded();
			const marker = editor.locator(`.gradient-stop-handle[data-node-id="${first.node_id}"]`);
			const treeRow = (id) => outliner.locator(`[data-tree-node-id="${id}"]`);
			const selected = (id) =>
				wait(
					() => treeRow(id).getAttribute('aria-selected'),
					(value) => value === 'true'
				);
			const owner = () =>
				wait(
					() => inspector.getAttribute('data-inspected-uuid'),
					(value) => value === gradient.uuid
				);
			await owner();
			await selected(gradient.node_id);
			assert.equal(await editor.locator('.gradient-stop-handle.selected').count(), 0);
			const before = await state(),
				beforeHistory = await idle();
			stage = 'hover and before-release shared selection with stable capture';
			await marker.hover();
			await selected(gradient.node_id);
			assert.equal(await editor.locator('.gradient-stop-handle.selected').count(), 0);
			async function down() {
				await page.locator('.gc-loading-overlay').waitFor({ state: 'detached' });
				const expected = await value();
				await wait(
					() => marker.evaluate((element) => Number.parseFloat(element.style.left) / 100),
					(value) => Math.abs(value - expected) < 0.0001
				);
				await marker.scrollIntoViewIfNeeded();
				const original = await editor.elementHandle();
				await editor.evaluate((element) =>
					element.addEventListener(
						'pointerdown',
						(event) => {
							element.dataset.testPointer = String(event.pointerId);
						},
						{ capture: true, once: true }
					)
				);
				const box = await marker.boundingBox();
				assert(box);
				await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
				await page.mouse.down();
				await selected(first.node_id);
				await owner();
				await editor
					.locator(`.gradient-stop-editor .node-inspector[data-node-id="${first.node_id}"]`)
					.waitFor();
				assert(await editor.evaluate((element, previous) => element === previous, original));
				assert(
					await editor.evaluate((element) =>
						element.hasPointerCapture(Number(element.dataset.testPointer))
					),
					'primary-down retains capture on the actual stable product editor'
				);
			}
			await down();
			await page.mouse.up();
			await idle();
			await editor.focus();
			await page.keyboard.press('Escape');
			await selected(gradient.node_id);
			assert.equal(await editor.locator('.gradient-stop-handle.selected').count(), 0);
			assert.deepEqual(await state(), before);
			assert.deepEqual(await history(), beforeHistory);
			stage = 'cancel armed removal without authoring';
			await down();
			const track = await editor.locator('.gradient-track').boundingBox();
			assert(track);
			await page.mouse.move(track.x, track.y + track.height + 60);
			await editor.locator('.gradient-stop-handle.removing').waitFor();
			const pointerId = Number(await editor.getAttribute('data-test-pointer'));
			await editor.dispatchEvent('pointercancel', { pointerId: pointerId + 1 });
			assert(await editor.evaluate((element, id) => element.hasPointerCapture(id), pointerId));
			await editor.dispatchEvent('pointercancel', { pointerId });
			await wait(
				() => editor.locator('.gradient-stop-handle.removing').count(),
				(count) => count === 0
			);
			await page.mouse.move(track.x + track.width * 0.7, track.y + track.height / 2);
			await page.mouse.up();
			await idle();
			await selected(first.node_id);
			assert.deepEqual(await state(), before);
			assert.deepEqual(await history(), beforeHistory);
			stage = 'real Parameter selection, typed editing and exact history';
			await treeRow(first.node_id).focus();
			await treeRow(first.node_id).press('ArrowRight');
			await treeRow(position.node_id)
				.getByRole('button', { name: 'Position', exact: true })
				.click();
			await selected(position.node_id);
			await owner();
			const stopHeader = editor
				.locator(
					`.gradient-stop-editor .node-inspector[data-node-id="${first.node_id}"] .node-title`
				)
				.first();
			if ((await stopHeader.getAttribute('aria-checked')) === 'false') await stopHeader.click();
			const beforeNumber = await state(),
				numberHistory = await idle();
			const input = editor.locator(
				`.parameter-inspector[data-node-id="${position.node_id}"] input.number-field`
			);
			await input.focus();
			await input.press('Escape');
			await selected(position.node_id);
			assert.equal(
				await marker.getAttribute('class').then((value) => value.split(' ').includes('selected')),
				true
			);
			await input.fill('0.25');
			await input.press('Enter');
			await wait(value, (value) => value === 0.25);
			await idle();
			const edited = await state();
			assert.equal((await history()).undo_len, numberHistory.undo_len + 1);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(state, (value) => JSON.stringify(value) === JSON.stringify(beforeNumber));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(state, (value) => JSON.stringify(value) === JSON.stringify(edited));
			stage = 'accepted live drag and active Escape fence held motion';
			await editor.scrollIntoViewIfNeeded();
			await down();
			const liveTrack = await editor.locator('.gradient-track').boundingBox();
			await page.mouse.move(
				liveTrack.x + liveTrack.width * 0.4,
				liveTrack.y + liveTrack.height / 2,
				{ steps: 8 }
			);
			await wait(value, (value) => Math.abs(value - 0.4) < 0.015);
			await page.keyboard.press('Escape');
			await idle();
			const accepted = await state();
			await page.mouse.move(
				liveTrack.x + liveTrack.width * 0.8,
				liveTrack.y + liveTrack.height / 2,
				{ steps: 8 }
			);
			await page.mouse.up();
			assert.deepEqual(await state(), accepted);
			await selected(first.node_id);
			await editor.focus();
			await page.keyboard.press('Escape');
			await selected(gradient.node_id);
			assert.equal(await editor.locator('.gradient-stop-handle.selected').count(), 0);
			assert.equal((await history()).undo_len, numberHistory.undo_len + 2);
			await page.getByTitle('Ctrl/Cmd+Z', { exact: true }).click();
			await wait(state, (value) => JSON.stringify(value) === JSON.stringify(edited));
			await page.getByTitle('Ctrl/Cmd+Shift+Z or Ctrl+Y', { exact: true }).click();
			await wait(state, (value) => JSON.stringify(value) === JSON.stringify(accepted));
			stage = 'independent product Inspectors and lifetime';
			await verifyGradientProductLifetime({
				page,
				post,
				snapshot,
				intent,
				wait,
				emit,
				evidence,
				formula,
				primary: { anode, gradient, stop: first, position }
			});
			assert.deepEqual(rejected, []);
			emit({
				kind: 'gradient-product',
				passed: true,
				beforeReleaseSelection: true,
				stableCapture: true,
				typedParameterSelection: true,
				cancelDoesNotDelete: true,
				liveDragAndActiveEscape: true,
				exactUndoRedo: true,
				nativePixels: 'not qualified by headless browser input'
			});
		} catch (error) {
			throw new Error(`${stage}: ${error.message}`, { cause: error });
		}
	}
});
