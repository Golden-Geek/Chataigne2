import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from '../native-workbench.mjs';
import { expand, pad, fingerprint } from './projects.mjs';
import { createWireDiagnostics } from './wire.mjs';

const wire = createWireDiagnostics();

// One real operation for phase attribution. This deliberately cannot qualify latency distributions.
await nativeWorkbench({
	fixture: new URL('../alchemist-graph-fixture/', import.meta.url),
	name: 'p11-formula-connection-diagnostic',
	observePage: wire.observe,
	async exercise({ page, evidence, emit }) {
		await page.waitForFunction(
			() => window.alchemistGraphFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.alchemistGraphFixture[method](...args), {
				method,
				args
			});
		const count = 1724,
			tier = 50000;
		const { project, metadata } = expand('formula', count);
		pad(project, 4);
		const path = resolve(evidence, 'fixture.noisette');
		writeFileSync(path, JSON.stringify(project));
		await call('load', path);
		await page.waitForFunction(
			(count) => window.alchemistGraphFixture.states().length === count,
			count
		);
		await call('scaleReady');
		assert.equal((await call('status')).treeNodes, tier);
		const states = await call('states'),
			byUuid = new Map(states.map((state) => [state.uuid, state]));
		const source = byUuid.get(metadata.items.at(-2)).id;
		const target = byUuid.get(metadata.items.at(-1)).id;
		emit({
			kind: 'diagnostic-contract',
			diagnosticOnly: true,
			retainedSamples: 1,
			tier,
			graphItems: count,
			sha256: fingerprint(project),
			engineTracing: process.env.GOLDEN_PERF_TRACE !== undefined
		});
		assert.equal(await call('reveal', target), true);
		assert.equal(await call('startConnection', source, 'result'), true);
		const started = performance.now();
		assert.equal(await call('completeConnection', target, 'value2'), true);
		await call('scaleReady');
		const topology = await call('scaleTopology', [source]);
		assert.ok(
			topology.some(
				(edge) =>
					edge.targetSocket === 'value2' && edge.target === byUuid.get(metadata.items.at(-1)).uuid
			)
		);
		emit({
			kind: 'connect-observation',
			elapsedMs: performance.now() - started,
			treeNodesAfter: (await call('status')).treeNodes,
			topology
		});
		emit({ kind: 'diagnostic-wire', rows: wire.rows });
		const undoStarted = performance.now();
		await call('history', 'undo');
		await call('scaleReady');
		emit({
			kind: 'undo-observation',
			elapsedMs: performance.now() - undoStarted,
			treeNodesAfter: (await call('status')).treeNodes
		});
	}
});
