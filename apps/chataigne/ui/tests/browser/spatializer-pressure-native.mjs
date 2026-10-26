import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from './native-workbench.mjs';
import { expand, pad, fingerprint } from './app-graph-scale/projects.mjs';
import { spatializerScaleContract as contract } from './app-graph-scale/spatializer-contract.mjs';

const coefficients = {
	voronoi: [3, 3],
	targetRadius: [3, 3],
	sourceRadius: [4, 2],
	overlap: [4, 3]
};
const scenes = contract.modes.flatMap((mode) => {
	const [s, t] = coefficients[mode];
	return [
		...contract.debugPressureTiers.map((tier) => ({
			mode,
			tier,
			sources: contract.sources,
			targets: Math.floor((tier - 185 - s * contract.sources) / (t + contract.sources))
		})),
		...contract.endpointCapScenes.map((scene) => ({ mode, ...scene, tier: null }))
	];
});

const requested = process.env.GC_SPATIALIZER_PRESSURE_CASES;
const selection =
	requested === undefined
		? scenes.map((_, index) => index)
		: requested.split(',').map((value) => {
				assert.match(value.trim(), /^\d+$/);
				return Number(value);
			});
assert.ok(selection.length && new Set(selection).size === selection.length);
assert.ok(selection.every((index) => Number.isSafeInteger(index) && index < scenes.length));
const trace = !['', '0', 'false', 'off'].includes(
	(process.env.GOLDEN_PERF_TRACE ?? '').trim().toLowerCase()
);
const diagnosticOnly = requested !== undefined || trace;

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-pressure',
	async exercise({ page, evidence, emit }) {
		await page.waitForFunction(
			() => window.spatializerFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.spatializerFixture[method](...args), {
				method,
				args
			});
		emit({
			kind: 'pressure-contract',
			scenes: scenes.length,
			selectedCases: selection,
			fullSelection: selection.length === scenes.length,
			diagnosticOnly,
			engineTrace: trace,
			actionTiming: false,
			limits: contract.limits
		});
		const mounted = async () => {
			const result = await page.evaluate(() => {
				const map = document.querySelector('.spatializer-map');
				const groups = [...map.querySelectorAll('.endpoint-overview')];
				return {
					presentation: map.getAttribute('data-presentation'),
					visible: Number(map.getAttribute('data-visible-endpoints')),
					glyphs: map.querySelectorAll('g.endpoint').length,
					groups: groups.length,
					groupedEndpoints: groups.reduce(
						(n, g) =>
							n +
							Number(g.getAttribute('data-source-count')) +
							Number(g.getAttribute('data-target-count')),
						0
					),
					groupGeometry: groups.every((g) => {
						const r = g.getBoundingClientRect();
						return r.width > 0 && r.height > 0;
					}),
					polygons: map.querySelectorAll('polygon').length,
					rows: document.querySelectorAll('.related-value-row').length,
					finderRows: document.querySelectorAll('[data-graph-result]').length,
					label: [...map.querySelectorAll('.map-summary')].map((e) => e.textContent).join(' ')
				};
			});
			assert.ok(result.glyphs <= contract.limits.endpointGlyphs, JSON.stringify(result));
			assert.ok(result.groups <= contract.limits.overviewGroups);
			assert.ok(result.polygons <= contract.limits.regionPolygons);
			assert.ok(result.rows <= contract.limits.relatedRows);
			assert.ok(result.finderRows <= contract.limits.finderRows);
			if (result.presentation === 'overview') {
				assert.match(result.label, /Endpoint overview/);
				assert.equal(result.groupedEndpoints, result.visible);
				assert.ok(result.groupGeometry);
			}
			return result;
		};
		for (const index of selection) {
			const scene = scenes[index];
			const { mode, sources, targets, tier } = scene;
			const [s, t] = coefficients[mode];
			const expected = 185 + s * sources + t * targets + sources * targets;
			const { project, metadata } = expand('spatializer', targets, { mode, sources });
			if (tier !== null) pad(project, tier - expected);
			const path = resolve(evidence, `${mode}-${sources}-${targets}.noisette`);
			writeFileSync(path, JSON.stringify(project));
			const before = (await call('status')).generation;
			const started = performance.now();
			const remaining = () => {
				const ms = contract.setupDeadlineMs - (performance.now() - started);
				assert.ok(ms > 0, 'Cumulative setup deadline exceeded');
				return ms;
			};
			await call('load', path);
			emit({
				kind: 'load-return',
				...scene,
				elapsedMs: performance.now() - started,
				publication: await call('scaleScene')
			});
			const progress = diagnosticOnly
				? setInterval(() => {
						void call('scaleScene')
							.then((publication) =>
								emit({ kind: 'diagnostic-publication', ...scene, publication })
							)
							.catch(() => {});
					}, 5000)
				: null;
			try {
				await page.waitForFunction(
					({ before, mode, sources, targets, expected }) => {
						const scene = window.spatializerFixture.scaleScene();
						return (
							scene.generation > before &&
							scene.mode === mode &&
							scene.sources === sources &&
							scene.targets === targets &&
							scene.cells === sources * targets &&
							scene.treeNodes === expected &&
							scene.pending === 0
						);
					},
					{ before, mode, sources, targets, expected: tier ?? expected },
					{ timeout: remaining(), polling: 500 }
				);
			} catch (error) {
				emit({ kind: 'publication-failure', ...scene, publication: await call('scaleScene') });
				throw error;
			} finally {
				if (progress !== null) clearInterval(progress);
			}
			emit({ kind: 'matrix-published', ...scene, elapsedMs: performance.now() - started });
			await call('debug', true);
			await page.locator('.graph-canvas').evaluate((element) => element.focus());
			assert.equal(await call('home'), true);
			await page.waitForFunction(() => !window.spatializerFixture.navigation().animating, null, {
				timeout: remaining()
			});
			await call('settle');
			const home = await mounted();
			emit({ kind: 'home-view', ...scene, home, navigation: await call('navigation') });
			assert.equal(home.presentation, 'overview');
			assert.equal(home.groupedEndpoints, sources + targets);
			const endpoints = await call('endpoints'),
				byUuid = new Map(endpoints.map((e) => [e.uuid, e]));
			assert.equal(byUuid.size, sources + targets);
			assert.ok([...metadata.items, ...metadata.sourceItems].every((id) => byUuid.has(id)));
			const far = byUuid.get((sources > targets ? metadata.sourceItems : metadata.items).at(-1));
			const found = await call('find', far.label);
			assert.equal(found.rows.length, 1);
			assert.equal(await call('reveal', far.id), true);
			await call('settle');
			const detail = await mounted();
			assert.equal(detail.presentation, 'detail');
			assert.equal(
				await page.evaluate(() =>
					document.activeElement.closest('g.endpoint')?.getAttribute('data-endpoint-uuid')
				),
				far.uuid
			);
			const circle = page.locator(`g[data-endpoint-uuid="${far.uuid}"] .endpoint-hit`);
			assert.ok(
				await circle.evaluate((e) => {
					const r = e.getBoundingClientRect(),
						c = e.closest('.graph-canvas').getBoundingClientRect();
					return r.left < c.right && r.right > c.left && r.top < c.bottom && r.bottom > c.top;
				})
			);
			await call('home');
			await page.waitForFunction(() => !window.spatializerFixture.navigation().animating);
			assert.equal(
				await page.evaluate(() =>
					document.activeElement.closest('g.endpoint')?.getAttribute('data-endpoint-uuid')
				),
				far.uuid
			);
			await mounted();
			assert.equal(await call('reveal', far.id), true);
			const owner = byUuid.get((sources > targets ? metadata.items : metadata.sourceItems)[0]);
			const related = byUuid.get(
				(sources > targets ? metadata.sourceItems : metadata.items).at(-1)
			);
			await call('select', owner.id);
			await page.waitForFunction(
				(count) =>
					Number(document.querySelector('.related-value-list')?.getAttribute('aria-rowcount')) ===
					count,
				Math.max(sources, targets)
			);
			assert.equal(await call('revealRelated', related.id), true);
			const row = page.locator(`.related-value-row[data-endpoint-uuid="${related.uuid}"]`);
			await row.waitFor();
			const cell = await call('readNode', Number(await row.getAttribute('data-value-node')));
			assert.equal(cell.data.kind, 'parameter');
			assert.equal(cell.data.param.value.kind, 'float');
			const folder = await call('readNode', await call('readParent', cell.node_id));
			for (const endpoint of [owner, related])
				assert.ok(
					[cell.decl_id, folder.decl_id].some((decl) =>
						decl.endsWith(endpoint.uuid.replaceAll('-', ''))
					)
				);
			assert.equal(
				Number(await row.locator('.slider-label').textContent()),
				Number(cell.data.param.value.value.toFixed(3))
			);
			await row.locator('.slider').evaluate((e) => e.focus());
			const firstRelated = byUuid.get(
				(sources > targets ? metadata.sourceItems : metadata.items)[0]
			);
			assert.equal(await call('revealRelated', firstRelated.id), true);
			await call('settle');
			assert.equal(
				await page.evaluate(() =>
					document.activeElement.closest('.related-value-row')?.getAttribute('data-endpoint-uuid')
				),
				related.uuid
			);
			const inspector = await mounted();
			remaining();
			emit({
				kind: 'spatializer-pressure',
				passed: true,
				diagnosticOnly,
				...scene,
				treeNodes: tier ?? expected,
				cells: sources * targets,
				inputSha256: fingerprint(project),
				home,
				detail,
				inspector,
				setupAndChecksMs: performance.now() - started
			});
		}
	}
});
