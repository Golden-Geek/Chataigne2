import assert from 'node:assert/strict';
import { nativeWorkbench } from './native-workbench.mjs';

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-native',
	async exercise({ page, emit }) {
		await page.waitForFunction(
			() => window.spatializerFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.spatializerFixture[method](...args), {
				method,
				args
			});
		const fixture = await call('setup');
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
		}
		await call('configure', 'value_layout', 'targetCentric');
		await page.waitForFunction(
			() => document.querySelectorAll('.related-value-slider').length === 2
		);
		assert.deepEqual(
			(await page.locator('.related-value-label').allTextContents()).map((label) => label.trim()),
			['West', 'East']
		);
		emit({ kind: 'mode-and-matrix-layout-replacement', passed: true });
		await call('configure', 'dimensions', '3d');
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
		assert.equal(await page.locator('.toolbar-chip').first().textContent(), 'XZ');
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
	}
});
