import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from './native-workbench.mjs';
import { expand, pad, fingerprint, records } from './app-graph-scale/projects.mjs';
import { createWireDiagnostics } from './app-graph-scale/wire.mjs';

const trace = process.env.GC_GRAPH_SCALE_TRACE === '1' ? createWireDiagnostics() : null;
const allActions = ['discover', 'single-move', 'three-move', 'connection'];
const actions = process.env.GC_GRAPH_SCALE_ACTIONS?.split(',') ?? allActions;
assert.ok(actions.length && actions.every((action) => allActions.includes(action)));

const allTiers = [1000, 10000, 20000, 50000, 100000];
const tiers = process.env.GC_GRAPH_SCALE_TIERS
	? process.env.GC_GRAPH_SCALE_TIERS.split(',').map(Number)
	: allTiers;
assert.ok(
	tiers.length &&
		new Set(tiers).size === tiers.length &&
		tiers.every((tier) => allTiers.includes(tier))
);

await nativeWorkbench({
	fixture: new URL('./state-graph-fixture/', import.meta.url),
	name: 'p11-state-scale',
	observePage: trace?.observe,
	async exercise({ page, evidence, emit }) {
		await page.waitForFunction(() => window.stateGraphFixture?.status().connection === 'connected');
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.stateGraphFixture[method](...args), {
				method,
				args
			});
		const profiler =
			process.env.GC_GRAPH_SCALE_CPU_PROFILE === '1'
				? await page.context().newCDPSession(page)
				: null;
		if (profiler) await profiler.send('Profiler.enable');
		const load = async (project, label, expectedItems) => {
			const path = resolve(evidence, label + '.noisette');
			writeFileSync(path, JSON.stringify(project));
			const before = (await call('status')).generation,
				started = performance.now();
			await call('load', path);
			await page.waitForFunction(
				({ before, expectedItems }) =>
					window.stateGraphFixture.status().generation > before &&
					window.stateGraphFixture.states().length === expectedItems,
				{ before, expectedItems },
				{ timeout: 300000 }
			);
			await call('scaleReady');
			const status = await call('status'),
				states = await call('states');
			emit({
				kind: 'source-ready',
				label,
				graphItems: states.length,
				treeNodes: status.treeNodes,
				activeStates: states.filter((state) => state.enabled).length,
				serializedRecords: records(project.root).length,
				setupMs: performance.now() - started,
				sha256: fingerprint(project)
			});
			return { status, states };
		};
		emit({
			kind: 'fixed-contract',
			consumer: 'state',
			tiers,
			completeTierSelection: JSON.stringify(tiers) === JSON.stringify(allTiers),
			actions,
			completeActionSelection: JSON.stringify(actions) === JSON.stringify(allActions),
			diagnosticTracing: !!trace,
			cpuProfiling: !!profiler,
			warmups: 3,
			retained: 31,
			normal: { feedbackP95: 200, completionP95: 2000, maximumStall: 200 },
			stress: { feedbackP95: 400, completionP95: 4000, maximumStall: 400 },
			maxMountedNodes: 128,
			maxFinderRows: 32,
			setupDeadlineMs: 300000
		});
		// Calibrate declaration expansion, not timing gates. Every later tier has an exact tree budget.
		const small = await load(expand('state', 8).project, 'calibration-8', 8);
		const large = await load(expand('state', 16).project, 'calibration-16', 16);
		const perItem = (large.status.treeNodes - small.status.treeNodes) / 8;
		const fixed = small.status.treeNodes - perItem * 8;
		assert.ok(Number.isSafeInteger(perItem) && perItem > 0 && Number.isSafeInteger(fixed));
		emit({ kind: 'declaration-calibration', perItem, fixed });
		for (const tier of tiers) {
			const count = Math.max(8, Math.floor((tier - fixed) / perItem));
			const { project, metadata } = expand('state', count);
			pad(project, tier - (fixed + count * perItem));
			const ready = await load(project, `state-${tier}`, count);
			assert.equal(
				ready.status.treeNodes,
				tier,
				'Retained backend tree size differs from fixed tier'
			);
			assert.equal((await call('transitions')).length, count - 1);
			assert.equal(
				ready.states.filter((state) => state.enabled).length,
				1,
				'Connected State fixture must have one backend-authoritative active State'
			);
			const byUuid = new Map(ready.states.map((state) => [state.uuid, state]));
			assert.ok(
				metadata.items.every((uuid) => byUuid.has(uuid)),
				'Backend changed fixture identities'
			);
			const topology = await call(
				'scaleTopology',
				metadata.items.map((uuid) => byUuid.get(uuid).id)
			);
			assert.deepEqual(
				topology.map((edge) => `${edge.from}/${edge.target}`).sort(),
				metadata.items
					.slice(0, -1)
					.map((uuid, index) => `${uuid}/${metadata.items[index + 1]}`)
					.sort(),
				'Actual backend chain differs from the fixed source workload'
			);
			const ids = metadata.items.slice(-3).map((uuid) => byUuid.get(uuid).id);
			const anchor = byUuid.get(metadata.items[0]).id,
				label = byUuid.get(metadata.items.at(-1)).label;
			const dimensions = await page.locator('.fixture').evaluate((element) => ({
				width: element.getBoundingClientRect().width,
				height: element.getBoundingClientRect().height
			}));
			assert.equal(dimensions.width, 1120);
			assert.equal(dimensions.height, 576);
			emit({
				kind: 'scene',
				tier,
				graphItems: count,
				chainEdges: count - 1,
				unrelatedRecords: tier - (fixed + count * perItem),
				dimensions,
				tail: ids
			});
			for (const action of actions) {
				if (profiler) await profiler.send('Profiler.start');
				const result = await call('measureScale', action, ids, anchor, label, tier === 100000);
				if (profiler) {
					const { profile } = await profiler.send('Profiler.stop');
					writeFileSync(resolve(evidence, `${tier}-${action}.cpuprofile`), JSON.stringify(profile));
				}
				emit({ kind: 'timing', consumer: 'state', tier, graphItems: count, ...result });
				if (!result.passed && trace) emit({ kind: 'diagnostic-wire', rows: trace.rows });
				assert.equal(
					result.passed,
					true,
					`${tier}/${action}: ${result.error ?? JSON.stringify({ feedback: result.p95Feedback, completion: result.p95Completion, stall: result.maximumStall })}`
				);
			}
		}
	}
});
