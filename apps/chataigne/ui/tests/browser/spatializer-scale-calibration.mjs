import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from './native-workbench.mjs';
import { expand, fingerprint, records } from './app-graph-scale/projects.mjs';
import {
	spatializerScaleContract,
	spatializerCalibrationScenes
} from './app-graph-scale/spatializer-contract.mjs';

// Declaration/publication calibration only. This cannot qualify action latency or scale.
await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-calibration',
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
			kind: 'calibration-contract',
			diagnosticOnly: true,
			modes: spatializerScaleContract.modes,
			scenes: spatializerCalibrationScenes,
			setupDeadlineMs: spatializerScaleContract.setupDeadlineMs
		});
		for (const mode of spatializerScaleContract.modes) {
			for (const { sources, targets: count } of spatializerCalibrationScenes) {
				const { project, metadata } = expand('spatializer', count, { mode, sources });
				const path = resolve(evidence, `${mode}-${sources}-${count}.noisette`);
				writeFileSync(path, JSON.stringify(project));
				const before = (await call('status')).generation;
				const started = performance.now();
				const remaining = () => {
					const budget = spatializerScaleContract.setupDeadlineMs - (performance.now() - started);
					assert.ok(budget > 0, 'Spatializer setup exceeded its cumulative deadline');
					return budget;
				};
				await call('load', path);
				await page.waitForFunction(
					({ sources, count, before }) => {
						const scene = window.spatializerFixture.scaleScene();
						return (
							scene.generation > before &&
							scene.sources === sources &&
							scene.targets === count &&
							scene.cells === sources * count &&
							scene.pending === 0
						);
					},
					{ sources, count, before },
					{ timeout: remaining() }
				);
				await call('settle');
				await page.waitForFunction(
					({ mode, sources, count }) => {
						const scene = window.spatializerFixture.scaleScene();
						const radiusParameters =
							mode === 'voronoi'
								? 0
								: mode === 'sourceRadius'
									? sources
									: mode === 'overlap'
										? sources + count
										: count;
						return (
							scene.mode === mode &&
							scene.radiusParameters === radiusParameters &&
							scene.freezeParameters === (mode === 'voronoi' ? count : 0) &&
							scene.cells === sources * count &&
							scene.pending === 0
						);
					},
					{ mode, sources, count },
					{ timeout: remaining() }
				);
				await call('settle');
				if (count === 8) await call('save', resolve(evidence, `${mode}-canonical.noisette`));
				const scene = await call('scaleScene');
				const ui = await page.evaluate(() => {
					const count = document.querySelector('.graph-canvas')?.getAttribute('data-node-count');
					if (count === null || count === undefined)
						throw new Error('Missing mounted logical canvas');
					return {
						logicalNodes: Number(count),
						endpointGlyphs: document.querySelectorAll('g.endpoint').length,
						regionPolygons: document.querySelectorAll('.voronoi-cell').length
					};
				});
				const endpoints = await call('endpoints');
				assert.equal(new Set(endpoints.map((node) => node.uuid)).size, count + sources);
				assert.ok(
					[...metadata.items, ...metadata.sourceItems].every((uuid) =>
						endpoints.some((node) => node.uuid === uuid)
					)
				);
				const authored = new Map(records(project.root).map((node) => [node.uuid, node]));
				for (const endpoint of endpoints) {
					const position = authored
						.get(endpoint.uuid)
						.children.find((node) => node.meta?.decl_id?.split('/').at(-1) === 'position_2d');
					assert.equal(endpoint.position.kind, 'parameter');
					assert.equal(endpoint.position.param.value.kind, 'vec2');
					assert.deepEqual(
						endpoint.position.param.value.value,
						position.data.value.Vec2,
						'Backend changed an authored endpoint position'
					);
				}
				remaining();
				emit({
					kind: 'declaration-calibration',
					diagnosticOnly: true,
					mode,
					targets: count,
					metadata,
					scene,
					ui,
					setupMs: performance.now() - started,
					sourceSha256: fingerprint(project)
				});
			}
		}
	}
});
