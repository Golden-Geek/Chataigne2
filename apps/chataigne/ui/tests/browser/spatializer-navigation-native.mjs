import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from './native-workbench.mjs';
import { expand } from './app-graph-scale/projects.mjs';

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-navigation',
	async exercise({ page, evidence, emit }) {
		await page.waitForFunction(
			() => window.spatializerFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.spatializerFixture[method](...args), {
				method,
				args
			});
		const load = async (label, { sources = 32, targets = 64 } = {}) => {
			const { project, metadata } = expand('spatializer', targets, { sources });
			const path = resolve(evidence, label + '.noisette');
			writeFileSync(path, JSON.stringify(project));
			const before = (await call('status')).generation;
			await call('load', path);
			await page.waitForFunction(
				({ before, sources, targets }) => {
					const scene = window.spatializerFixture.scaleScene();
					return (
						scene.generation > before &&
						scene.sources === sources &&
						scene.targets === targets &&
						scene.cells === sources * targets
					);
				},
				{ before, sources, targets }
			);
			await call('settle');
			const endpoints = await call('endpoints');
			const byUuid = new Map(endpoints.map((endpoint) => [endpoint.uuid, endpoint]));
			return {
				metadata,
				targets: metadata.items.map((id) => byUuid.get(id)),
				sources: metadata.sourceItems.map((id) => byUuid.get(id))
			};
		};
		const focusedUuid = () =>
			page.evaluate(
				() =>
					document.activeElement?.closest('g.endpoint')?.getAttribute('data-endpoint-uuid') ?? null
			);
		const centered = async () => {
			const result = await page.evaluate(() => {
				const rect = document.activeElement.getBoundingClientRect();
				const canvas = document.querySelector('.graph-canvas').getBoundingClientRect();
				const inspector = document.querySelector('.endpoint-inspector');
				const inset = inspector.classList.contains('visible')
					? Number.parseFloat(inspector.style.width)
					: 0;
				return {
					x: Math.abs((rect.left + rect.right) / 2 - (canvas.left + (canvas.width + inset) / 2)),
					y: Math.abs((rect.top + rect.bottom) / 2 - (canvas.top + canvas.height / 2))
				};
			});
			assert.ok(result.x <= 0.5 && result.y <= 0.5, JSON.stringify(result));
		};
		const fixture = await load('initial');
		assert.equal(await page.locator('article[data-node-id]').count(), 0);
		assert.equal(
			await page.locator('.graph-canvas').getAttribute('data-navigable-item-count'),
			'96'
		);
		const pages = [await call('find', ''), await call('page', 'next'), await call('page', 'next')];
		assert.ok(pages.every((p) => p.rows.length === 32));
		assert.equal(new Set(pages.flatMap((p) => p.rows.map((r) => r.id))).size, 96);
		assert.equal(await page.locator('[data-graph-result]').count(), 32);
		emit({ kind: 'custom-world-catalog-paging', passed: true, logicalItems: 96, mountedRows: 32 });

		for (const endpoint of [fixture.sources.at(-1), fixture.targets.at(-1)]) {
			const found = await call('find', endpoint.label);
			assert.equal(found.rows.length, 1);
			assert.equal(await call('reveal', endpoint.id), true);
			assert.equal(await focusedUuid(), endpoint.uuid);
			assert.deepEqual((await call('status')).selected, [endpoint.id]);
			await centered();
		}
		emit({ kind: 'source-target-svg-reveal', passed: true });
		const last = fixture.targets.at(-1),
			neighbor = fixture.targets.at(-2);
		const before = (await call('fieldValue', last.id, 'position_2d')).value;
		assert.equal(await call('key', last.id, 'ArrowRight'), true);
		await call('settle');
		assert.deepEqual((await call('fieldValue', last.id, 'position_2d')).value, [
			before[0] + 0.25,
			before[1]
		]);
		assert.equal(await focusedUuid(), last.uuid);
		emit({ kind: 'keyboard-mutation-retains-svg-focus', passed: true });

		assert.equal(await call('reveal', neighbor.id), true);
		assert.equal(await call('reveal', last.id, true), true);
		assert.deepEqual(new Set((await call('status')).selected), new Set([neighbor.id, last.id]));
		assert.equal(await page.locator('g.endpoint.selected').count(), 2);
		assert.equal(await call('frame'), true);
		await page.waitForFunction(() => !window.spatializerFixture.navigation().animating);
		assert.ok((await call('navigation')).camera.zoom > 0.5);
		emit({ kind: 'custom-world-selection-framing', passed: true });

		await call('find', last.label);
		await call('position', last.id, before[0], before[1]);
		assert.equal(await page.locator('.node-navigator input').inputValue(), last.label);
		assert.equal(await page.locator('[data-graph-result]').count(), 1);
		emit({ kind: 'position-change-retains-discovery', passed: true });

		const scene = await call('scaleScene');
		await call('configure', 'dimensions', '3d', scene.module);
		await page.waitForFunction(() =>
			window.spatializerFixture
				.endpoints()
				.every((e) => e.position?.kind === 'parameter' && e.position.param.value.kind === 'vec3')
		);
		assert.equal(await page.locator('.node-navigator').count(), 0);
		await call('position3d', last.id, 6, 7, 9);
		assert.equal(await call('reveal', last.id), true);
		assert.equal(await focusedUuid(), last.uuid);
		await centered();
		emit({ kind: 'coordinate-space-navigation-fence', passed: true });

		await call('remove', last.id);
		await page.waitForFunction(() => window.spatializerFixture.scaleScene().cells === 2016);
		assert.equal(await focusedUuid(), null);
		assert.equal(await call('reveal', last.id), false);
		assert.equal((await call('find', last.label)).rows.length, 0);
		await call('history', 'undo');
		await page.waitForFunction(() => window.spatializerFixture.scaleScene().cells === 2048);
		assert.equal(await focusedUuid(), null);
		assert.equal(await call('reveal', last.id), true);
		emit({ kind: 'item-retirement-and-undo', passed: true });

		await call('find', last.label);
		await call('refresh');
		await call('settle');
		assert.equal(await page.locator('.node-navigator').count(), 0);
		emit({ kind: 'snapshot-navigation-fence', passed: true });
		const firstSource = fixture.sources[0];
		assert.equal(await call('reveal', firstSource.id), true);
		const replacement = await load('replacement');
		assert.equal(
			replacement.sources[0].id,
			firstSource.id,
			'Fixture must exercise reused backend IDs'
		);
		assert.notEqual(replacement.sources[0].uuid, firstSource.uuid);
		assert.equal(await focusedUuid(), null);
		assert.equal(await call('reveal', replacement.sources[0].id), true);
		assert.equal(await focusedUuid(), replacement.sources[0].uuid);
		emit({ kind: 'project-replacement-navigation-fence', passed: true });
		await call('find', replacement.sources[0].label);
		await call('remount');
		assert.equal(await page.locator('.node-navigator').count(), 0);
		assert.equal(await call('reveal', replacement.sources[0].id), true);
		assert.equal(await focusedUuid(), replacement.sources[0].uuid);
		emit({ kind: 'unmount-navigation-retirement', passed: true });

		const small = await load('mode-focus', { sources: 8, targets: 8 });
		const target = small.targets.at(-1);
		assert.equal(await call('reveal', target.id), true);
		await page
			.getByRole('button', { name: `Resize ${target.label} freeze radius`, exact: true })
			.evaluate((element) => element.focus());
		assert.equal(await focusedUuid(), target.uuid);
		const modeScene = await call('scaleScene');
		await call('configure', 'mode', 'targetRadius', modeScene.module);
		await page.waitForFunction(
			(uuid) =>
				document.activeElement?.classList.contains('endpoint-hit') &&
				document.activeElement.closest('g.endpoint')?.getAttribute('data-endpoint-uuid') === uuid,
			target.uuid,
			{ timeout: 2000 }
		);
		assert.equal((await call('status')).generation, modeScene.generation);
		emit({ kind: 'mode-control-focus-restoration', passed: true });
	}
});
