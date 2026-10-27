import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { nativeWorkbench } from './native-workbench.mjs';
import { expand, pad, fingerprint } from './app-graph-scale/projects.mjs';
import { spatializerScaleContract as contract } from './app-graph-scale/spatializer-contract.mjs';

const select = (name, all, numeric = false) => {
	const value = process.env[name];
	if (value === undefined) return all;
	const selected = value.split(',').map((v) => (numeric ? Number(v) : v.trim()));
	assert.ok(
		selected.length &&
			new Set(selected).size === selected.length &&
			selected.every((v) => all.includes(v))
	);
	return selected;
};
const modes = select('GC_SPATIALIZER_SCALE_MODES', contract.modes);
const tiers = select('GC_SPATIALIZER_SCALE_TIERS', contract.tiers, true);
const selectedActions =
	process.env.GC_SPATIALIZER_SCALE_ACTIONS === undefined
		? null
		: select('GC_SPATIALIZER_SCALE_ACTIONS', contract.primaryActions);
const flag = (name) =>
	!['', '0', 'false', 'off'].includes((process.env[name] ?? '').trim().toLowerCase());
const diagnosticOnly =
	['GC_SPATIALIZER_SCALE_MODES', 'GC_SPATIALIZER_SCALE_TIERS', 'GC_SPATIALIZER_SCALE_ACTIONS'].some(
		(name) => process.env[name] !== undefined
	) ||
	flag('GOLDEN_PERF_TRACE') ||
	flag('GC_GRAPH_SCALE_CPU_PROFILE') ||
	flag('GC_GRAPH_SCALE_TRACE');
const coefficients = {
	voronoi: [3, 3],
	targetRadius: [3, 3],
	sourceRadius: [4, 2],
	overlap: [4, 3]
};

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-scale',
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
			kind: 'fixed-contract',
			modes,
			tiers,
			selectedActions,
			diagnosticOnly,
			actionTierCases: contract.actionTierCases,
			warmups: contract.warmups,
			retained: contract.retained,
			normal: contract.normal,
			stress: contract.stress,
			limits: contract.limits,
			setupDeadlineMs: contract.setupDeadlineMs
		});
		let cases = 0;
		for (const mode of modes) {
			const allActions = mode === 'voronoi' ? contract.primaryActions : contract.radiusModeActions;
			const actions = selectedActions
				? allActions.filter((a) => selectedActions.includes(a))
				: allActions;
			assert.ok(actions.length, 'Selection has no actions for this mode');
			for (const tier of tiers) {
				const [s, t] = coefficients[mode],
					sources = contract.sources;
				const targets = Math.floor((tier - 185 - s * sources) / (t + sources));
				const expected = 185 + s * sources + t * targets + sources * targets;
				const { project, metadata } = expand('spatializer', targets, { mode, sources });
				pad(project, tier - expected);
				const path = resolve(evidence, `${mode}-${tier}.noisette`);
				writeFileSync(path, JSON.stringify(project));
				const before = (await call('status')).generation,
					started = performance.now();
				const remaining = () => {
					const ms = contract.setupDeadlineMs - (performance.now() - started);
					assert.ok(ms > 0, 'Cumulative Spatializer setup deadline exceeded');
					return ms;
				};
				await call('load', path);
				await page.waitForFunction(
					({ before, mode, sources, targets, tier }) => {
						const scene = window.spatializerFixture.scaleScene();
						return (
							scene.generation > before &&
							scene.mode === mode &&
							scene.sources === sources &&
							scene.targets === targets &&
							scene.cells === sources * targets &&
							scene.treeNodes === tier &&
							scene.pending === 0
						);
					},
					{ before, mode, sources, targets, tier },
					{ timeout: remaining(), polling: 500 }
				);
				await call('scaleReady', remaining());
				remaining();
				const endpoints = await call('endpoints'),
					byUuid = new Map(endpoints.map((e) => [e.uuid, e]));
				assert.equal(byUuid.size, sources + targets);
				const sourceIds = metadata.sourceItems.map((uuid) => {
					assert.ok(byUuid.has(uuid));
					return byUuid.get(uuid).id;
				});
				const targetIds = metadata.items.map((uuid) => {
					assert.ok(byUuid.has(uuid));
					return byUuid.get(uuid).id;
				});
				const published = await call('scaleScene');
				const module = await call('readNode', published.module);
				const children = await Promise.all(module.children.map((id) => call('readNode', id)));
				const values = children.find(
					(n) => n.decl_id === 'values' || n.decl_id.endsWith('/values')
				);
				assert.ok(values);
				emit({
					kind: 'scene',
					mode,
					tier,
					sources,
					targets,
					cells: sources * targets,
					values: values.node_id,
					inputSha256: fingerprint(project),
					setupMs: performance.now() - started,
					treeNodes: published.treeNodes
				});
				for (const action of actions) {
					const caseId = `${mode}-${tier}-${action}`;
					let timer, lastProgress;
					let rejectDeadline;
					const deadline = new Promise((_, reject) => {
						rejectDeadline = reject;
					});
					const arm = (phase) => {
						clearTimeout(timer);
						const limit = phase === 'restore' ? contract.setupDeadlineMs : 10000;
						timer = setTimeout(() => {
							emit({
								kind: 'timing-timeout',
								mode,
								tier,
								action,
								lastProgress,
								phaseDeadlineMs: limit
							});
							rejectDeadline(new Error(`Spatializer ${phase} watchdog expired`));
						}, limit + 2000);
					};
					const progress = (message) => {
						const text = message.text(),
							prefix = 'spatializer-measurement:';
						if (!text.startsWith(prefix)) return;
						const value = JSON.parse(text.slice(prefix.length));
						if (value.caseId !== caseId) return;
						lastProgress = value;
						emit({ kind: 'sample-progress', mode, tier, ...value });
						arm(value.phase);
					};
					page.on('console', progress);
					arm('prepare');
					let result;
					try {
						result = await Promise.race([
							call('measureScale', action, {
								caseId,
								mode,
								sources: sourceIds,
								targets: targetIds,
								values: values.node_id,
								treeNodes: tier,
								warmups: contract.warmups,
								retained: contract.retained,
								gates: tier === 100000 ? contract.stress : contract.normal,
								limits: contract.limits
							}),
							deadline
						]);
					} finally {
						clearTimeout(timer);
						page.off('console', progress);
					}
					emit({ kind: 'timing', mode, tier, sources, targets, ...result });
					assert.equal(
						result.passed,
						true,
						`${mode}/${tier}/${action}: ${result.error ?? JSON.stringify({ feedback: result.p95Feedback, completion: result.p95Completion, stall: result.maximumStall })}`
					);
					cases++;
				}
			}
		}
		emit({
			kind: 'matrix-result',
			passed: true,
			cases,
			complete: !diagnosticOnly && cases === contract.actionTierCases
		});
		if (!diagnosticOnly) assert.equal(cases, contract.actionTierCases);
	}
});
