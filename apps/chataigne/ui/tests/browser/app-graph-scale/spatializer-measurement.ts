import { tick } from 'svelte';
import type { WorkbenchSession, UiNodeDto, ParamValue } from 'golden_ui';
import type SpatializerEditorPanel from '../../../src/lib/panels/modules/SpatializerEditorPanel.svelte';
import type { SpatializerEditResult } from '../../../src/lib/panels/modules/spatializer/generated/SpatializerEditResult';
import { SPATIALIZER_UNIT_REM } from '../../../src/lib/panels/modules/spatializer/constants';

type Panel = ReturnType<typeof SpatializerEditorPanel>;
interface Scene {
	caseId: string;
	mode: string;
	sources: number[];
	targets: number[];
	values: number;
	treeNodes: number;
	warmups: number;
	retained: number;
	gates: { feedbackP95: number; completionP95: number; maximumStall: number };
	limits: {
		endpointGlyphs: number;
		overviewGroups: number;
		regionPolygons: number;
		finderRows: number;
		relatedRows: number;
	};
}
interface Sample {
	warmup: boolean;
	feedback: number;
	completion: number;
	stall: number;
	peakGlyphs: number;
	peakRows: number;
}
const ensure: (value: unknown, message: string) => asserts value = (value, message) => {
	if (!value) throw new Error(message);
};
const frame = () => new Promise<number>(requestAnimationFrame);
const p95 = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1] ?? 0;

/** Observes real public panel operations and committed backend data in the browser's clock realm. */
export function createSpatializerMeasurement(
	session: WorkbenchSession,
	panel: () => Panel | undefined
) {
	const view = () => {
		const current = panel();
		ensure(current, 'Spatializer panel is not mounted');
		return current;
	};
	const root = () => document.querySelector<HTMLElement>('.graph-canvas')!;
	const node = (id: number) => session.graph.state.nodesById.get(id);
	const child = (owner: UiNodeDto | undefined, declaration: string) =>
		owner?.children
			.map(node)
			.find((n) => n?.decl_id === declaration || n?.decl_id.endsWith('/' + declaration));
	const field = (id: number, declaration: string): ParamValue | null => {
		const n = child(node(id), declaration);
		return n?.data.kind === 'parameter' ? n.data.param.value : null;
	};
	const position = (id: number) => {
		const value = field(id, 'position_2d');
		ensure(value?.kind === 'vec2', 'Fixture endpoint lacks a canonical XY position');
		return value.value;
	};
	const scalar = (id: number, declaration: string) => {
		const value = field(id, declaration);
		ensure(value?.kind === 'float', 'Fixture endpoint lacks its radius field');
		return value.value;
	};
	const glyph = (id: number) =>
		root().querySelector<SVGGElement>(`g.endpoint[data-endpoint-id="${id}"]`);
	const visible = (element: Element | null, within: Element = root()) => {
		if (!element) return false;
		const box = element.getBoundingClientRect(),
			viewport = within.getBoundingClientRect();
		return (
			box.width > 0 &&
			box.height > 0 &&
			box.right > viewport.left &&
			box.left < viewport.right &&
			box.bottom > viewport.top &&
			box.top < viewport.bottom
		);
	};
	const focused = (id: number) =>
		document.activeElement?.closest('g.endpoint')?.getAttribute('data-endpoint-id') === String(id);
	const geometry = (id: number, point: readonly number[]) => {
		const element = glyph(id),
			matrix = element?.transform.baseVal.consolidate()?.matrix;
		const scale =
			SPATIALIZER_UNIT_REM * parseFloat(getComputedStyle(document.documentElement).fontSize);
		return (
			!!matrix &&
			Math.abs(matrix.e - point[0] * scale) < 1e-6 &&
			Math.abs(matrix.f - point[1] * scale) < 1e-6
		);
	};
	const quiet = () =>
		!session.editOperations.size && !view().endpointNavigation.viewport()?.animating;
	const until = async (ready: () => unknown, limit = 10000) => {
		const started = performance.now();
		do {
			await tick();
			await frame();
			if (ready()) return;
		} while (performance.now() - started < limit);
		throw new Error('Spatializer observation did not settle');
	};
	const reveal = async (id: number, additive = false) => {
		ensure(await view().endpointNavigation.reveal(id, additive), 'Cannot reveal exact endpoint');
		await until(() => focused(id) && visible(glyph(id)) && quiet());
	};
	const rows = (scene: Scene) =>
		node(scene.values)
			?.children.map(node)
			.filter((n): n is UiNodeDto => !!n) ?? [];
	const counts = (scene: Scene) => {
		const folders = rows(scene);
		return (
			folders.length === scene.sources.length &&
			folders.every((n) => n.children.length === scene.targets.length)
		);
	};
	const mounted = (scene: Scene) => {
		const map = root().querySelector('.spatializer-map')!;
		const values = {
			glyphs: map.querySelectorAll('g.endpoint').length,
			groups: map.querySelectorAll('.endpoint-overview').length,
			polygons: map.querySelectorAll('polygon').length,
			finder: root().querySelectorAll('[data-graph-result]').length,
			rows: document.querySelectorAll('.related-value-row').length
		};
		ensure(
			values.glyphs <= scene.limits.endpointGlyphs &&
				values.groups <= scene.limits.overviewGroups &&
				values.polygons <= scene.limits.regionPolygons &&
				values.finder <= scene.limits.finderRows &&
				values.rows <= scene.limits.relatedRows,
			'Spatializer mounting exceeded a fixed bound'
		);
		return values;
	};
	return {
		ready: (limit: number) => until(quiet, limit),
		async run(action: string, scene: Scene) {
			const targets = scene.targets.slice(-2),
				far = targets[1],
				anchor = scene.targets[0];
			const group = [scene.sources.at(-1)!, ...targets];
			const radiusOwner = scene.mode === 'sourceRadius' ? scene.sources.at(-1)! : far;
			const radiusField = scene.mode === 'voronoi' ? 'freeze_radius' : 'radius';
			const radiusKind = scene.mode === 'voronoi' ? 'freezeRadius' : 'radius';
			const samples: Sample[] = [];
			for (let index = 0; index < scene.warmups + scene.retained; index++) {
				const report = (phase: string, sample?: Sample) =>
					console.info(
						'spatializer-measurement:' +
							JSON.stringify({ caseId: scene.caseId, action, index, phase, sample })
					);
				report('prepare');
				const direction = index % 2 === 0 ? 1 : -1;
				const selectedSource = index % 2 === 0 ? scene.sources.at(-1)! : scene.sources[0];
				const previousSource = index % 2 === 0 ? scene.sources[0] : scene.sources.at(-1)!;
				if (action === 'discover') {
					await reveal(anchor);
					await view().endpointNavigation.find('No such Spatializer label');
				} else if (action === 'three-move') {
					for (let i = 0; i < group.length; i++) await reveal(group[i], i !== 0);
					ensure(await view().endpointNavigation.frame(), 'Cannot frame the selected group');
				} else if (action === 'related-values') await reveal(previousSource);
				else await reveal(action === 'radius' ? radiusOwner : far);
				await until(quiet);
				if (action === 'radius') {
					const handle = glyph(radiusOwner)?.querySelector<SVGCircleElement>(
						radiusKind === 'freezeRadius'
							? '.freeze-radius-hit'
							: '.radius-hit:not(.freeze-radius-hit)'
					);
					ensure(handle, 'Radius control was not mounted');
					handle.focus();
				}
				const before = group.map((id) => [...position(id)]);
				const oldRadius = action === 'radius' ? scalar(radiusOwner, radiusField) : 0;
				const oldCircleRadius =
					action === 'radius'
						? glyph(radiusOwner)
								?.querySelector(
									radiusKind === 'freezeRadius' ? '.freeze-radius-fill' : '.radius-fill'
								)
								?.getAttribute('r')
						: null;
				const createPosition = [position(far)[0] + 1, position(far)[1] + 1];
				const previousFeedback = document.querySelector('.edit-feedback')?.textContent;
				const rowIds = new Set(rows(scene).map((n) => n.node_id));
				let cursor = session.graph.state.nodesById;
				const createdCells = new Map<number, number>();
				let result: SpatializerEditResult | undefined;
				let ended = false,
					failure: unknown;
				report('observe');
				const start = performance.now();
				let feedback = -1,
					stall = 0,
					last = start,
					peakGlyphs = 0,
					peakRows = 0;
				const operation = (async () => {
					if (action === 'discover') {
						const label = node(far)!.meta.label;
						const found = await view().endpointNavigation.find(label);
						ensure(
							found?.rows.length === 1 && found.rows[0].label === label,
							'Finder did not resolve the exact label'
						);
						const key = CSS.escape(found.rows[0].id);
						await until(() =>
							root().querySelector(`[data-graph-result="${key}"]`)?.textContent?.includes(label)
						);
						await reveal(far);
					} else if (action === 'single-move') {
						ensure(
							await view().editEndpointKey(far, direction > 0 ? 'ArrowRight' : 'ArrowLeft'),
							'Keyboard move refused'
						);
					} else if (action === 'three-move') {
						result = await view().moveEndpoints(
							group.map((id, i) => ({
								id,
								position: { x: before[i][0] + direction * 0.25, y: before[i][1] }
							}))
						);
					} else if (action === 'radius') {
						ensure(
							await view().editEndpointKey(
								radiusOwner,
								direction > 0 ? 'ArrowRight' : 'ArrowLeft',
								false,
								radiusKind
							),
							'Radius request refused'
						);
					} else if (action === 'create') {
						result = await view().createTargetAtWorld({
							x: createPosition[0] * SPATIALIZER_UNIT_REM,
							y: createPosition[1] * SPATIALIZER_UNIT_REM
						});
					} else if (action === 'related-values') {
						await reveal(selectedSource);
						await until(
							() =>
								document.querySelector('.endpoint-summary strong')?.textContent ===
								node(selectedSource)!.meta.label
						);
						ensure(await view().revealRelatedValue(far), 'Related-value reveal refused');
					} else throw new Error('Unknown Spatializer measurement action: ' + action);
				})().then(
					() => {
						ended = true;
					},
					(error) => {
						failure = error;
						ended = true;
					}
				);
				try {
					let complete = false;
					while (performance.now() - start < 10000) {
						await tick();
						const now = await frame();
						ensure(
							performance.now() - start <= 10000,
							'Spatializer action observation deadline exceeded'
						);
						stall = Math.max(stall, now - last);
						last = now;
						const mount = mounted(scene);
						peakGlyphs = Math.max(peakGlyphs, mount.glyphs);
						peakRows = Math.max(peakRows, mount.rows);
						if (failure) throw failure;
						let shown = false,
							applied = false;
						if (action === 'discover') {
							shown =
								root().querySelector<HTMLInputElement>('input[type="search"]')?.value ===
									node(far)!.meta.label || focused(far);
							applied = focused(far) && visible(glyph(far)) && geometry(far, position(far));
						} else if (action === 'single-move' || action === 'three-move') {
							shown = geometry(far, [before[2][0] + direction * 0.25, before[2][1]]);
							applied =
								group.every((id, i) => {
									const p = position(id),
										moves = action === 'three-move' || i === 2;
									return (
										p[0] === before[i][0] + (moves ? direction * 0.25 : 0) &&
										p[1] === before[i][1] &&
										(!moves || geometry(id, p))
									);
								}) &&
								focused(far) &&
								visible(glyph(far));
							if (action === 'three-move')
								applied &&=
									group.every((id) => session.selectedNodesIds.includes(id)) &&
									result?.endpoints.length === 3;
						} else if (action === 'radius') {
							const circle = glyph(radiusOwner)?.querySelector(
								radiusKind === 'freezeRadius' ? '.freeze-radius-fill' : '.radius-fill'
							);
							const expected = oldRadius + direction * 0.25;
							const scale =
								SPATIALIZER_UNIT_REM *
								parseFloat(getComputedStyle(document.documentElement).fontSize);
							shown = circle?.getAttribute('r') !== oldCircleRadius;
							applied =
								scalar(radiusOwner, radiusField) === expected &&
								!!circle &&
								Math.abs(Number(circle.getAttribute('r')) - expected * scale) < 1e-6 &&
								focused(radiusOwner) &&
								visible(glyph(radiusOwner));
						} else if (action === 'create') {
							const created = result?.created;
							const rendered =
								!!created && geometry(created.node, createPosition) && visible(glyph(created.node));
							const status = document.querySelector('.edit-feedback');
							shown =
								rendered ||
								(!!status?.textContent?.trim() &&
									status.textContent !== previousFeedback &&
									visible(status, status.closest('.spatializer-editor-panel') ?? root()));
							if (created) {
								const current = session.graph.state.nodesById;
								current.forEachChange(cursor, (id, n) => {
									const parent = session.graph.state.parentById.get(id);
									if (
										n?.data.kind === 'parameter' &&
										n.data.param.value.kind === 'float' &&
										n.decl_id.endsWith(created.uuid.replaceAll('-', '')) &&
										parent !== undefined &&
										rowIds.has(parent)
									)
										createdCells.set(parent, id);
								});
								cursor = current;
								applied =
									rendered &&
									node(created.node)?.uuid === created.uuid &&
									createdCells.size === scene.sources.length &&
									focused(far) &&
									session.graph.state.nodesById.size ===
										scene.treeNodes + scene.sources.length + 3 &&
									rows(scene).every((row) => row.children.length === scene.targets.length + 1);
							}
						} else {
							const source = node(selectedSource)!,
								related = node(far)!;
							const row = document.querySelector<HTMLElement>(
								`.related-value-row[data-endpoint-uuid="${related.uuid}"]`
							);
							const cell = row ? node(Number(row.dataset.valueNode)) : null;
							const parent = cell ? node(session.graph.state.parentById.get(cell.node_id)!) : null;
							shown =
								document.querySelector('.endpoint-summary strong')?.textContent ===
								source.meta.label;
							applied =
								shown &&
								Number(
									document.querySelector('.related-value-list')?.getAttribute('aria-rowcount')
								) === scene.targets.length &&
								visible(row) &&
								cell?.data.kind === 'parameter' &&
								cell.data.param.value.kind === 'float' &&
								!!parent &&
								parent.decl_id.endsWith(source.uuid.replaceAll('-', '')) &&
								cell.decl_id.endsWith(related.uuid.replaceAll('-', '')) &&
								Number(row!.querySelector('.slider-label')?.textContent) ===
									Number(cell.data.param.value.value.toFixed(3)) &&
								focused(selectedSource);
						}
						if (shown && feedback < 0) feedback = performance.now() - start;
						if (ended && applied && quiet()) {
							complete = true;
							break;
						}
					}
					ensure(
						complete && feedback >= 0,
						'Action lacked committed state, rendered content, focus or drained editor work'
					);
					await operation;
					samples.push({
						warmup: index < scene.warmups,
						feedback,
						completion: performance.now() - start,
						stall,
						peakGlyphs,
						peakRows
					});
					report('sample', samples.at(-1));
				} catch (error) {
					return {
						action,
						passed: false,
						error: String(error),
						index,
						samples,
						elapsedMs: performance.now() - start,
						feedback,
						ended,
						pending: session.editOperations.size,
						selected: session.selectedNodesIds,
						active: document.activeElement?.outerHTML.slice(0, 600),
						result,
						createdCells: createdCells.size
					};
				}
				if (action === 'create') {
					report('restore');
					await session.runEditBarrier(() => session.sendIntent({ kind: 'undo' }));
					await until(
						() =>
							!node(result!.created!.node) &&
							session.graph.state.nodesById.size === scene.treeNodes &&
							counts(scene) &&
							quiet(),
						300000
					);
					report('restored');
				}
			}
			const retained = samples.filter((s) => !s.warmup);
			const p95Feedback = p95(retained.map((s) => s.feedback)),
				p95Completion = p95(retained.map((s) => s.completion));
			const maximumStall = Math.max(...retained.map((s) => s.stall));
			return {
				action,
				passed:
					p95Feedback <= scene.gates.feedbackP95 &&
					p95Completion <= scene.gates.completionP95 &&
					maximumStall <= scene.gates.maximumStall,
				p95Feedback,
				p95Completion,
				maximumStall,
				samples
			};
		}
	};
}
