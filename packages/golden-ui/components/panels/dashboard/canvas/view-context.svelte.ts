import { onDestroy, tick } from 'svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import type { GraphState } from '../../../../store/graph.svelte';
import type { FreeLayoutRect } from './contracts';
import type { FreeSurfaceGeometry } from './free-surface-geometry';

export const dashboardViewContextKey = Symbol('dashboard-view');

export interface FreeSurfaceView {
	readonly geometry: FreeSurfaceGeometry;
	readonly element: HTMLElement;
}
export interface SurfacePresentation {
	readonly activeTabUuid: string | null;
	readonly openAccordionUuids: readonly string[];
}
export interface SurfacePresentationActions {
	activateTab(id: NodeId): boolean;
	toggleAccordion(id: NodeId): boolean;
}
export interface DashboardViewContext {
	readonly viewport: HTMLElement | null;
	readonly camera: unknown;
	readonly ready: boolean;
	readonly selected: readonly NodeId[];
	activateAt(surface: NodeId, clientX: number, clientY: number): NodeId | null;
	register(id: NodeId, view: FreeSurfaceView): () => void;
	query(id: NodeId, rect: FreeLayoutRect): NodeId[] | null;
	retainedFor(id: NodeId): readonly NodeId[];
	frameRects(ids: readonly NodeId[]): Promise<FreeLayoutRect[] | null>;
	readPresentation(uuid: string): SurfacePresentation | undefined;
	writePresentation(uuid: string, state: SurfacePresentation): void;
	registerPresentation(id: NodeId, actions: SurfacePresentationActions): () => void;
	activateTab(surface: NodeId, child: NodeId): boolean;
	toggleAccordion(surface: NodeId, child: NodeId): boolean;
}
interface DashboardViewSource {
	readonly viewport: HTMLElement | null;
	readonly frame: HTMLElement | null;
	readonly frameSize: { width: number; height: number };
	readonly page: UiNodeDto;
	readonly graph: GraphState | null;
	readonly camera: unknown;
	readonly ready: boolean;
	readonly selected: readonly NodeId[];
	select(id: NodeId): void;
}

export function createDashboardViewContext(context: DashboardViewSource): DashboardViewContext {
	const surfaces = new Map<NodeId, FreeSurfaceView>();
	const presentations = new Map<string, SurfacePresentation>();
	const actions = new Map<NodeId, SurfacePresentationActions>();
	let retained = $state.raw<ReadonlyMap<NodeId, readonly NodeId[]>>(new Map());
	let pageUuid: string | null = null;
	let previousGraph: GraphState | null = null;
	let frameGeneration = 0;
	let destroyed = false;

	$effect(() => {
		const graph = context.graph;
		if (context.page.uuid !== pageUuid || !graph) {
			pageUuid = context.page.uuid;
			presentations.clear();
			retained = new Map();
			frameGeneration++;
		} else if (previousGraph) {
			graph.nodesById.forEachChange(previousGraph.nodesById, (_id, node, before) => {
				if (before && node?.uuid !== before.uuid) presentations.delete(before.uuid);
			});
		}
		previousGraph = graph;
	});
	onDestroy(() => {
		destroyed = true;
		frameGeneration++;
		surfaces.clear();
		presentations.clear();
		actions.clear();
		previousGraph = null;
	});
	const clientRect = (id: NodeId): FreeLayoutRect | null => {
		for (const view of surfaces.values()) {
			const rect = view.geometry.get(id);
			if (!rect || view.geometry.getNode(id)?.uuid !== context.graph?.nodesById.get(id)?.uuid)
				continue;
			const bounds = view.element.getBoundingClientRect();
			const scaleX = bounds.width / Math.max(view.element.clientWidth, 1);
			const scaleY = bounds.height / Math.max(view.element.clientHeight, 1);
			return {
				left: bounds.left + rect.left * scaleX,
				top: bounds.top + rect.top * scaleY,
				width: rect.width * scaleX,
				height: rect.height * scaleY
			};
		}
		return null;
	};
	const frameRects = async (ids: readonly NodeId[]): Promise<FreeLayoutRect[] | null> => {
		const generation = ++frameGeneration;
		const identity = context.page.uuid;
		const intended = new Map(ids.map((id) => [id, context.graph?.nodesById.get(id)?.uuid]));
		const result: FreeLayoutRect[] = [];
		const pending: NodeId[] = [];
		const canceled = () =>
			destroyed || generation !== frameGeneration || context.page.uuid !== identity;
		const measure = (id: NodeId, dom: ReadonlyMap<NodeId, HTMLElement>): boolean => {
			if (!intended.get(id) || context.graph?.nodesById.get(id)?.uuid !== intended.get(id))
				return true;
			const rect = clientRect(id) ?? dom.get(id)?.getBoundingClientRect();
			const frame = context.frame;
			if (!rect || !frame) return false;
			const bounds = frame.getBoundingClientRect(),
				size = context.frameSize;
			const scaleX = bounds.width / size.width,
				scaleY = bounds.height / size.height;
			if (!(scaleX > 0 && scaleY > 0)) return false;
			result.push({
				left: (rect.left - bounds.left) / scaleX,
				top: (rect.top - bounds.top) / scaleY,
				width: rect.width / scaleX,
				height: rect.height / scaleY
			});
			return true;
		};
		const domIndex = (): Map<NodeId, HTMLElement> => {
			const index = new Map<NodeId, HTMLElement>();
			for (const element of context.frame?.querySelectorAll<HTMLElement>('[data-node-id]') ?? []) {
				const id = Number(element.dataset.nodeId);
				if (!index.has(id)) index.set(id, element);
			}
			return index;
		};
		try {
			await tick();
			if (canceled()) return null;
			const initial = domIndex();
			for (const id of ids) if (!measure(id, initial)) pending.push(id);
			// Temporary ancestry is bounded per batch and is released before camera animation.
			for (let offset = 0; offset < pending.length; offset += 32) {
				if (canceled()) return null;
				const batch = pending.slice(offset, offset + 32);
				const pins = new Map<NodeId, Set<NodeId>>();
				for (const id of batch) {
					const graph = context.graph;
					const path: NodeId[] = [];
					const seen = new Set<NodeId>();
					let ancestor = graph?.parentById.get(id);
					while (
						ancestor !== undefined &&
						ancestor !== context.page.node_id &&
						!seen.has(ancestor)
					) {
						seen.add(ancestor);
						path.push(ancestor);
						ancestor = graph?.parentById.get(ancestor);
					}
					if (ancestor !== context.page.node_id || !graph) continue;
					for (const member of path) {
						if (graph.nodesById.get(member)?.node_type !== 'dashboard_widget_container') continue;
						const parent = graph.parentById.get(member);
						if (parent === undefined) continue;
						let set = pins.get(parent);
						if (!set) pins.set(parent, (set = new Set()));
						set.add(member);
					}
				}
				if (!pins.size) continue;
				retained = new Map([...pins].map(([id, children]) => [id, [...children]]));
				await tick();
				await new Promise(requestAnimationFrame);
				await tick();
				if (canceled()) return null;
				const dom = domIndex();
				for (const id of batch) measure(id, dom);
			}
			return result;
		} finally {
			if (!destroyed && generation === frameGeneration) retained = new Map();
		}
	};
	return {
		get viewport() {
			return context.viewport;
		},
		get camera() {
			return [context.camera, retained];
		},
		get ready() {
			return context.ready;
		},
		get selected() {
			return context.selected;
		},
		activateAt(id, clientX, clientY) {
			const view = surfaces.get(id);
			if (!view) return null;
			const bounds = view.element.getBoundingClientRect();
			if (!(bounds.width > 0 && bounds.height > 0)) return null;
			const target = view.geometry.hitTest(
				((clientX - bounds.left) / bounds.width) * view.element.clientWidth,
				((clientY - bounds.top) / bounds.height) * view.element.clientHeight
			);
			if (target !== null) context.select(target);
			return target;
		},
		register(id, view) {
			surfaces.set(id, view);
			return () => {
				if (surfaces.get(id) === view) surfaces.delete(id);
			};
		},
		retainedFor: (id) => retained.get(id) ?? [],
		frameRects,
		query(id, rect) {
			const view = surfaces.get(id);
			if (!view) return null;
			const bounds = view.element.getBoundingClientRect();
			const scaleX = bounds.width / Math.max(view.element.clientWidth, 1),
				scaleY = bounds.height / Math.max(view.element.clientHeight, 1);
			if (!(scaleX > 0 && scaleY > 0)) return [];
			return view.geometry.query({
				left: (rect.left - bounds.left) / scaleX,
				top: (rect.top - bounds.top) / scaleY,
				width: rect.width / scaleX,
				height: rect.height / scaleY
			});
		},
		readPresentation: (uuid) =>
			pageUuid === context.page.uuid ? presentations.get(uuid) : undefined,
		writePresentation: (uuid, state) => {
			if (!destroyed && pageUuid === context.page.uuid)
				presentations.set(uuid, {
					activeTabUuid: state.activeTabUuid,
					openAccordionUuids: [...state.openAccordionUuids]
				});
		},
		registerPresentation(id, value) {
			actions.set(id, value);
			return () => {
				if (actions.get(id) === value) actions.delete(id);
			};
		},
		activateTab: (id, child) => actions.get(id)?.activateTab(child) ?? false,
		toggleAccordion: (id, child) => actions.get(id)?.toggleAccordion(child) ?? false
	};
}
