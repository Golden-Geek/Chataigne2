import assert from 'node:assert/strict';
import { nativeWorkbench } from './native-workbench.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-native',
	async exercise({ page, emit, evidence }) {
		await page.waitForFunction(
			() => window.spatializerFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.spatializerFixture[method](...args), {
				method,
				args
			});
		const saveCheckpoint = (name) => call('save', resolve(evidence, name + '.noisette'));
		await saveCheckpoint('empty-project');
		const fixture = await call('setup');
		await saveCheckpoint('setup');
		await page.locator('circle.endpoint-hit').first().waitFor();
		assert.equal(await page.locator('circle.endpoint-hit').count(), 3);
		assert.equal(await page.locator('article[data-node-id]').count(), 0);
		assert.equal(await page.locator('polygon.voronoi-cell').count(), 2);
		const world = await page.locator('.spatializer-map').evaluate((element) => {
			const rect = element.getBoundingClientRect();
			const viewport = element.closest('.graph-canvas').getBoundingClientRect();
			return {
				width: rect.width,
				height: rect.height,
				position: getComputedStyle(element).position,
				overflow: getComputedStyle(element).overflow,
				viewport: { width: viewport.width, height: viewport.height }
			};
		});
		assert.ok(world.viewport.width > 500 && world.viewport.height > 300);
		assert.ok(world.width > 0 && world.height > 0);
		assert.equal(world.position, 'absolute');
		assert.equal(world.overflow, 'visible');
		emit({ kind: 'empty-document-custom-world', passed: true, world });
		await call('select', fixture.source);
		await page.waitForFunction(
			() => document.querySelector('.endpoint-summary strong')?.textContent === 'Probe'
		);
		assert.equal(await page.locator('.related-value-row').count(), 2);
		assert.deepEqual(
			(await page.locator('.related-value-label').allTextContents()).map((label) => label.trim()),
			['West', 'East']
		);
		await call('debug', true);
		await page.locator('.debug-layer').waitFor();
		assert.ok((await page.locator('.debug-weight-line').count()) > 0);
		emit({ kind: 'selection-inspector-debug', passed: true });
		await call('position', fixture.source, 0.5, 0.25);
		await page.waitForFunction(
			() =>
				document.querySelector('g.endpoint.source')?.getAttribute('transform') ===
				'translate(32 16)'
		);
		const circle = page.locator('g.endpoint.source circle.endpoint-hit');
		await circle.evaluate((element) => element.focus());
		assert.equal(await call('home'), true);
		const visible = await circle.evaluate((element) => {
			const rect = element.getBoundingClientRect(),
				panel = document.querySelector('.fixture').getBoundingClientRect();
			return (
				rect.left >= panel.left &&
				rect.right <= panel.right &&
				rect.top >= panel.top &&
				rect.bottom <= panel.bottom
			);
		});
		assert.ok(visible, 'Home did not reveal the custom-world source');
		emit({ kind: 'authoritative-projection-and-home', passed: true });
		for (const [mode, radii] of [
			['targetRadius', 2],
			['sourceRadius', 1],
			['overlap', 3],
			['voronoi', 0]
		]) {
			await call('configure', 'mode', mode);
			await page.waitForFunction(
				(count) => document.querySelectorAll('.radius-fill').length === count,
				radii
			);
			assert.equal(await page.locator('.voronoi-cell').count(), mode === 'voronoi' ? 2 : 0);
			await saveCheckpoint(mode);
		}
		await call('configure', 'value_layout', 'targetCentric');
		await saveCheckpoint('layout');
		await page.waitForFunction(
			() => document.querySelectorAll('.related-value-slider').length === 2
		);
		assert.deepEqual(
			(await page.locator('.related-value-label').allTextContents()).map((label) => label.trim()),
			['West', 'East']
		);
		emit({ kind: 'mode-and-matrix-layout-replacement', passed: true });
		await call('configure', 'dimensions', '3d');
		await saveCheckpoint('dimensions');
		await call('position3d', fixture.source, 0.5, 7, 0.25);
		await page.waitForFunction((id) => {
			const value = window.spatializerFixture
				.endpoints()
				.find((endpoint) => endpoint.id === id)?.position;
			return (
				value?.kind === 'parameter' &&
				value.param.value.kind === 'vec3' &&
				value.param.value.value[1] === 7
			);
		}, fixture.source);
		assert.equal(await page.locator('span.toolbar-chip').first().textContent(), 'XZ');
		assert.equal(
			await page.locator('g.endpoint.source').getAttribute('transform'),
			'translate(32 16)'
		);
		emit({ kind: 'three-dimensional-projection', passed: true });
		const generation = (await call('status')).generation;
		assert.equal(await call('refresh'), true);
		assert.ok((await call('status')).generation > generation);
		await call('remount');
		assert.equal(await page.locator('circle.endpoint-hit').count(), 3);
		assert.equal(await page.locator('.endpoint-summary strong').textContent(), 'Probe');
		assert.equal(await page.locator('.debug-layer').count(), 1);
		emit({ kind: 'snapshot-and-remount', passed: true, endpoints: await call('endpoints') });
		const vector = async (id) =>
			(await call('endpoints')).find((endpoint) => endpoint.id === id).position.param.value.value;
		const waitVector = (id, expected) =>
			page.waitForFunction(
				({ id, expected }) => {
					const node = window.spatializerFixture.endpoints().find((endpoint) => endpoint.id === id);
					return (
						node?.position?.kind === 'parameter' &&
						node.position.param.value.kind === 'vec3' &&
						node.position.param.value.value.every((value, index) => value === expected[index])
					);
				},
				{ id, expected }
			);
		const beforeKeys = await call('historyLength');
		await assert.rejects(call('staleSpace', fixture.source), /coordinate space changed/);
		assert.equal(await call('historyLength'), beforeKeys);
		await waitVector(fixture.source, [0.5, 7, 0.25]);
		emit({ kind: 'stale-coordinate-space-rejection', passed: true });
		assert.equal(await call('key', fixture.source, 'ArrowRight'), true);
		await waitVector(fixture.source, [0.75, 7, 0.25]);
		assert.equal(await call('historyLength'), beforeKeys + 1);
		await call('history', 'undo');
		await waitVector(fixture.source, [0.5, 7, 0.25]);
		await call('history', 'redo');
		await waitVector(fixture.source, [0.75, 7, 0.25]);
		const beforeGesture = await call('historyLength');
		assert.equal(await call('begin', fixture.source, 'position', { x: 0.75, y: 0.25 }), true);
		await call('position3d', fixture.source, 0.75, 9, 0.25);
		await page.evaluate(() => {
			for (let i = 1; i <= 200; i++)
				window.spatializerFixture.update({
					x: 0.75 + (i / 200) * 0.75,
					y: 0.25 + (i / 200) * 0.75
				});
		});
		await call('finish');
		await waitVector(fixture.source, [1.5, 9, 1]);
		assert.equal(await call('historyLength'), beforeGesture + 1);
		await call('history', 'undo');
		await waitVector(fixture.source, [0.75, 7, 0.25]);
		await call('history', 'redo');
		await waitVector(fixture.source, [1.5, 9, 1]);
		emit({ kind: 'keyboard-gesture-and-history', passed: true });
		await saveCheckpoint('gesture-history');
		const radiusHandle = page.locator(
			'g[data-endpoint-id="' + fixture.west + '"] .freeze-radius-hit'
		);
		assert.ok(
			Number(await radiusHandle.getAttribute('r')) > 0,
			'Zero radius must retain a reachable handle'
		);
		await radiusHandle.evaluate((element) => element.focus());
		assert.equal(await call('key', fixture.west, 'ArrowUp', false, 'freezeRadius'), true);
		await page.waitForFunction(
			(id) => window.spatializerFixture.fieldValue(id, 'freeze_radius')?.value === 0.25,
			fixture.west
		);
		assert.equal(await call('key', fixture.west, 'ArrowDown', false, 'freezeRadius'), true);
		await page.waitForFunction(
			(id) => window.spatializerFixture.fieldValue(id, 'freeze_radius')?.value === 0,
			fixture.west
		);
		emit({ kind: 'zero-radius-keyboard-control', passed: true });
		await saveCheckpoint('radius');
		for (const action of ['refresh', 'remount']) {
			assert.equal(await call('begin', fixture.source, 'position', { x: 1.5, y: 1 }), true);
			await call('update', { x: 2, y: 1.25 });
			await call(action);
			await page.waitForFunction(() => window.spatializerFixture.status().pending === 0);
			const after = await vector(fixture.source),
				history = await call('historyLength');
			await call('update', { x: 100, y: 100 });
			await call('finish');
			await call('settle');
			assert.deepEqual(await vector(fixture.source), after);
			assert.equal(await call('historyLength'), history);
		}
		emit({ kind: 'active-gesture-snapshot-and-unmount', passed: true });
		const created = await call('createInView');
		assert.ok(created.created);
		await page.waitForFunction(
			(uuid) => window.spatializerFixture.endpoints().some((endpoint) => endpoint.uuid === uuid),
			created.created.uuid
		);
		const createdCircle = page.locator(
			'g[data-endpoint-id="' + created.created.node + '"] circle.endpoint-hit'
		);
		const inView = await createdCircle.evaluate((element) => {
			const box = element.getBoundingClientRect(),
				view = element.closest('.graph-canvas').getBoundingClientRect();
			return (
				box.left >= view.left &&
				box.right <= view.right &&
				box.top >= view.top &&
				box.bottom <= view.bottom
			);
		});
		assert.ok(inView, 'Add target did not create inside the visible canvas');
		const next = await call('createAt', { x: 40, y: 20 });
		assert.ok(next.created);
		assert.notEqual(next.created.uuid, created.created.uuid);
		await waitVector(next.created.node, [10, 0, 5]);
		const names = (await call('endpoints'))
			.filter((endpoint) => [next.created.uuid, created.created.uuid].includes(endpoint.uuid))
			.map((endpoint) => endpoint.label);
		assert.equal(new Set(names).size, 2);
		await call('history', 'undo');
		await page.waitForFunction(
			(uuid) => !window.spatializerFixture.endpoints().some((endpoint) => endpoint.uuid === uuid),
			next.created.uuid
		);
		await call('history', 'redo');
		await page.waitForFunction(
			(uuid) => window.spatializerFixture.endpoints().some((endpoint) => endpoint.uuid === uuid),
			next.created.uuid
		);
		emit({
			kind: 'backend-target-creation',
			passed: true,
			created: created.created,
			next: next.created
		});
		const saved = resolve(evidence, 'roundtrip.noisette'),
			readonly = resolve(evidence, 'readonly.noisette');
		await call('save', saved);
		const original = (await call('endpoints')).find((endpoint) => endpoint.id === fixture.source);
		const document = JSON.parse(readFileSync(saved, 'utf8'));
		let locked = 0;
		const visit = (node) => {
			if (node.uuid === original.uuid) {
				const position = node.children.find(
					(child) => child.meta.decl_id.split('/').at(-1) === 'position_3d'
				);
				assert.ok(position);
				position.data = { ...position.data, read_only: true };
				locked++;
			}
			for (const child of node.children ?? []) visit(child);
		};
		visit(document.root);
		assert.equal(locked, 1);
		writeFileSync(readonly, JSON.stringify(document));
		const beforeReadOnly = await vector(fixture.source);
		await call('holdReplacement');
		await call('load', readonly);
		await page.waitForFunction(() => window.spatializerFixture.status().heldResync);
		assert.equal(
			await call('begin', fixture.source, 'position', {
				x: beforeReadOnly[0],
				y: beforeReadOnly[2]
			}),
			true
		);
		await call('update', { x: 100, y: 100 });
		let rejected = false;
		try {
			await call('finish');
		} catch {
			rejected = true;
		}
		assert.ok(rejected, 'Old-view gesture unexpectedly committed to the replacement');
		await page.waitForFunction(
			() => document.querySelector('.edit-feedback')?.textContent === 'Endpoint edit failed.'
		);
		assert.deepEqual(await vector(fixture.source), beforeReadOnly);
		const transform = await page.locator('g.endpoint.source').getAttribute('transform');
		assert.equal(
			transform,
			'translate(' + beforeReadOnly[0] * 64 + ' ' + beforeReadOnly[2] * 64 + ')'
		);
		await call('resumeReplacement');
		await page.waitForFunction((uuid) => {
			const endpoint = window.spatializerFixture
				.endpoints()
				.find((endpoint) => endpoint.uuid === uuid);
			return endpoint && !window.spatializerFixture.fieldWritable(endpoint.id);
		}, original.uuid);
		const lockedSource = (await call('endpoints')).find(
			(endpoint) => endpoint.uuid === original.uuid
		);
		const lockedWest = (await call('endpoints')).find((endpoint) => endpoint.label === 'West');
		const readonlyHistory = await call('historyLength'),
			westBefore = await vector(lockedWest.id),
			sourceBefore = await vector(lockedSource.id);
		await assert.rejects(
			call('move', [
				{ id: lockedWest.id, position: { x: 9, y: 9 } },
				{ id: lockedSource.id, position: { x: 10, y: 10 } }
			]),
			/read-only/
		);
		assert.deepEqual(await vector(lockedWest.id), westBefore);
		assert.deepEqual(await vector(lockedSource.id), sourceBefore);
		assert.equal(await call('historyLength'), readonlyHistory);
		assert.equal(await call('key', lockedSource.id, 'ArrowRight'), false);
		assert.equal(await call('begin', lockedSource.id, 'position', { x: 0, y: 0 }), false);
		emit({ kind: 'read-only-replacement-rejection-and-rollback', passed: true });
		await call('load', saved);
		await page.waitForFunction((uuid) => {
			const endpoint = window.spatializerFixture
				.endpoints()
				.find((endpoint) => endpoint.uuid === uuid);
			return endpoint && window.spatializerFixture.fieldWritable(endpoint.id);
		}, original.uuid);
		const recovered = (await call('endpoints')).find((endpoint) => endpoint.uuid === original.uuid);
		assert.equal(await call('key', recovered.id, 'ArrowRight'), true);
		await waitVector(recovered.id, [
			beforeReadOnly[0] + 0.25,
			beforeReadOnly[1],
			beforeReadOnly[2]
		]);
		emit({ kind: 'saved-project-and-recovery', passed: true });
	}
});
