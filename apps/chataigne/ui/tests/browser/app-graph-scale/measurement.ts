import { tick } from 'svelte';

interface Context {
	reveal(id: number, additive?: boolean): Promise<boolean>;
	find(query: string): void;
	move(key: string): Promise<boolean>;
	startConnection(id: number): Promise<boolean>;
	completeConnection(id: number): Promise<boolean>;
	connectionPair?(ids: number[]): { source: number; destination: number };
	connectionTargetSocket?: string;
	undo(): Promise<void>;
	position(id: number): readonly number[];
	targets(id: number): { id: number; uuid: string; target: string | null; targetSocket?: string }[];
	uuid(id: number): string;
	work():
		{ mounting: { nodes: number; edges: number }; routing: { pendingRoutes: number } } | undefined;
	pending(): number;
}
const frame = () => new Promise<number>(requestAnimationFrame);
const ensure = (value: unknown, message: string) => {
	if (!value) throw new Error(message);
};
const percentile = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1] ?? 0;

/** Measures actual backend publication, focused DOM/layout and drained canvas work in one realm. */
export function createNativeGraphMeasurement(context: Context) {
	const root = () => document.querySelector<HTMLElement>('.graph-canvas')!;
	const node = (id: number) => root().querySelector<HTMLElement>(`article[data-node-id="${id}"]`);
	const quiet = () => {
		const work = context.work();
		return (
			work &&
			!work.mounting.nodes &&
			!work.mounting.edges &&
			!work.routing.pendingRoutes &&
			!context.pending()
		);
	};
	const until = async (ready: () => unknown, limit = 10000) => {
		const start = performance.now();
		do {
			await tick();
			await frame();
			if (ready()) return;
		} while (performance.now() - start < limit);
		throw new Error('Native graph condition did not settle');
	};
	const focused = (id: number) =>
		document.activeElement?.closest('article')?.dataset.nodeId === String(id);
	const visible = (element: Element | null) => {
		if (!element) return false;
		const box = element.getBoundingClientRect(),
			view = root().getBoundingClientRect();
		return (
			box.width > 0 &&
			box.height > 0 &&
			box.right > view.left &&
			box.left < view.right &&
			box.bottom > view.top &&
			box.top < view.bottom
		);
	};
	const visibleWire = (path: SVGPathElement | null) => {
		if (!path || !path.getAttribute('d') || path.getTotalLength() <= 0) return false;
		const box = path.getBoundingClientRect(),
			view = root().getBoundingClientRect();
		const style = getComputedStyle(path);
		// A valid horizontal/vertical stroked path has a zero-area geometric bounding box.
		return (
			(box.width > 0 || box.height > 0) &&
			style.stroke !== 'none' &&
			Number.parseFloat(style.strokeWidth) > 0 &&
			box.right >= view.left &&
			box.left <= view.right &&
			box.bottom >= view.top &&
			box.top <= view.bottom
		);
	};
	return {
		topology: (ids: number[]) =>
			ids.flatMap((id) => context.targets(id).map((edge) => ({ from: context.uuid(id), ...edge }))),
		ready: (limit = 300000) => until(quiet, limit),
		async run(action: string, ids: number[], anchor: number, label: string, stress: boolean) {
			ensure(ids.length === 3, 'Native graph measurement requires three exact identities');
			const target = ids[2];
			const { source, destination } = context.connectionPair?.(ids) ?? {
				source: ids[2],
				destination: ids[1]
			};
			const samples: {
				warmup: boolean;
				feedback: number;
				completion: number;
				stall: number;
				peakNodes: number;
			}[] = [];
			for (let index = -3; index < 31; index++) {
				if (action === 'discover') {
					ensure(await context.reveal(anchor), 'Cannot prepare near source');
					context.find('No such fixture label');
					await until(
						() =>
							root().querySelector('ul[aria-label="Matching nodes"]')?.getAttribute('aria-busy') ===
							'false'
					);
				} else if (action === 'three-move') {
					for (let i = 0; i < ids.length; i++)
						ensure(await context.reveal(ids[i], i !== 0), 'Cannot select exact group');
				} else ensure(await context.reveal(target), 'Cannot reveal exact far node');
				if (action === 'connection')
					ensure(await context.startConnection(source), 'Cannot activate source socket');
				await until(quiet);
				const before = ids.map((id) => [...context.position(id)]);
				const oldEdges = new Set(context.targets(source).map((edge) => edge.uuid));
				const direction = index % 2 === 0 ? 1 : -1;
				let ended = false,
					failure: unknown;
				const start = performance.now();
				let feedback = -1,
					stall = 0,
					peakNodes = 0,
					last = start;
				const operation = (async () => {
					if (action === 'discover') {
						context.find(label);
						await until(() =>
							root().querySelector(`[data-graph-result="${target}"]`)?.textContent?.includes(label)
						);
						ensure(await context.reveal(target), 'Cannot reveal found backend identity');
					} else if (action === 'connection')
						ensure(await context.completeConnection(destination), 'Connection request refused');
					else
						ensure(
							await context.move(direction > 0 ? 'ArrowRight' : 'ArrowLeft'),
							'Movement request refused'
						);
				})().then(
					() => {
						ended = true;
					},
					(error) => {
						failure = error;
						ended = true;
					}
				);
				let complete = false;
				try {
					while (performance.now() - start < 10000) {
						await tick();
						const now = await frame();
						stall = Math.max(stall, now - last);
						last = now;
						peakNodes = Math.max(
							peakNodes,
							root().querySelectorAll('article[data-node-id]').length
						);
						ensure(peakNodes <= 128, `Mounted ${peakNodes} graph nodes`);
						ensure(
							root().querySelectorAll('[data-graph-result]').length <= 32,
							'Finder mounting exceeded 32'
						);
						if (failure) throw failure;
						let shown = false,
							applied = false;
						if (action === 'discover') {
							shown =
								root().querySelector<HTMLInputElement>('input[type="search"]')?.value === label ||
								focused(target);
							applied =
								focused(target) &&
								visible(node(target)) &&
								!!node(target)?.textContent?.includes(label);
						} else if (action === 'connection') {
							const button = node(source)?.querySelector('[data-socket-direction="output"]');
							shown = button?.getAttribute('aria-pressed') === 'false';
							const edge = context
								.targets(source)
								.find(
									(edge) =>
										!oldEdges.has(edge.uuid) &&
										edge.target === context.uuid(destination) &&
										(!context.connectionTargetSocket ||
											edge.targetSocket === context.connectionTargetSocket)
								);
							const path = edge
								? root().querySelector<SVGPathElement>(`path.edge[data-edge-id="${edge.id}"]`)
								: null;
							applied = visibleWire(path) && focused(target);
						} else {
							const expected = before[2][0] + direction * 0.25;
							shown = Number.parseFloat(node(target)?.style.left ?? '') === expected;
							applied =
								ids.every((id, i) => {
									const value = context.position(id),
										moves = action === 'three-move' || i === 2;
									return (
										value[0] === before[i][0] + (moves ? direction * 0.25 : 0) &&
										value[1] === before[i][1]
									);
								}) &&
								shown &&
								focused(target);
						}
						if (shown && feedback < 0) feedback = performance.now() - start;
						if (ended && applied && quiet()) {
							complete = true;
							break;
						}
					}
					ensure(
						complete && feedback >= 0,
						`${action} lacked committed source, visible geometry or focus`
					);
					await operation;
					samples.push({
						warmup: index < 0,
						feedback,
						completion: performance.now() - start,
						stall,
						peakNodes
					});
				} catch (error) {
					return {
						action,
						passed: false,
						error: String(error),
						diagnostics: {
							pendingRequests: context.pending(),
							feedback,
							ended,
							source,
							destination,
							target,
							focused: document.activeElement?.closest('article')?.dataset.nodeId,
							targets: context.targets(source),
							wires: [...root().querySelectorAll<SVGPathElement>('path.edge')].map((path) => ({
								id: path.dataset.edgeId,
								d: path.getAttribute('d'),
								box: path.getBoundingClientRect().toJSON()
							}))
						},
						index,
						samples,
						pending: context.work(),
						elapsed: performance.now() - start
					};
				}
				if (action === 'connection') {
					await context.undo();
					await until(quiet);
				}
			}
			const retained = samples.filter((sample) => !sample.warmup);
			const p95Feedback = percentile(retained.map((sample) => sample.feedback));
			const p95Completion = percentile(retained.map((sample) => sample.completion));
			const maximumStall = Math.max(...retained.map((sample) => sample.stall));
			return {
				action,
				passed:
					p95Feedback <= (stress ? 400 : 200) &&
					p95Completion <= (stress ? 4000 : 2000) &&
					maximumStall <= (stress ? 400 : 200),
				p95Feedback,
				p95Completion,
				maximumStall,
				samples
			};
		}
	};
}
