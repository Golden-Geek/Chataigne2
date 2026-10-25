import { onDestroy, tick } from 'svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import type { GraphState } from '../../../../store/graph.svelte';
import type { ParameterPreviews } from '../../../../store/session/edit-operations';
import type { FreeLayoutRect, SurfaceDropPreview, WidgetLayoutEdit } from './contracts';
import type { FreeSurfaceGeometry } from './free-surface-geometry';
import { framePreparation } from './frame-preparation';
import { framePresentationBounds } from './frame-bounds';
import { SurfaceItemOrder } from './surface-item-order';

export const dashboardViewContextKey = Symbol('dashboard-view');

export interface FreeSurfaceView {
	readonly geometry: FreeSurfaceGeometry;
	readonly element: HTMLElement;
}
export interface SurfacePresentation {
	readonly activeTabUuid: string | null;
	readonly openAccordionUuids: readonly string[];
}
export interface SurfaceScroll {
	readonly left: number;
	readonly top: number;
}
export interface SurfacePresentationActions {
	createNodeWidget(target: NodeId, preview: SurfaceDropPreview | null): Promise<boolean>;
	prepareForFrame(ids: readonly NodeId[]): void;
	chooseChild(id: NodeId): boolean;
	activateTab(id: NodeId): boolean;
	toggleAccordion(id: NodeId): boolean;
}
export interface DashboardViewContext {
	readonly itemOrder: SurfaceItemOrder;
	readonly previews: ParameterPreviews | null;
	registerLayout(id: NodeId, edit: (command: WidgetLayoutEdit) => Promise<boolean>): () => void;
	editLayout(id: NodeId, command: WidgetLayoutEdit): Promise<boolean>;
	createNodeWidget(
		surface: NodeId,
		target: NodeId,
		preview: SurfaceDropPreview | null
	): Promise<boolean>;
	readScroll(uuid: string): SurfaceScroll | undefined;
	writeScroll(uuid: string, position: SurfaceScroll): void;
	select(id: NodeId): void;
	registerBrowser(id: NodeId, browser: SurfaceBrowserActions): () => void;
	browse(id: NodeId): boolean;
	chooseChild(surface: NodeId, child: NodeId): boolean;
	searchChildren(surface: NodeId, query: string): void;
	revealChild(surface: NodeId, child: NodeId): Promise<boolean>;
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
	readonly previews: ParameterPreviews | null;
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

export interface SurfaceBrowserActions {
	openMenu(): void;
	chooseChild(id: NodeId): boolean;
	search(query: string): void;
	reveal(id: NodeId): Promise<boolean>;
}

export function createDashboardViewContext(context: DashboardViewSource): DashboardViewContext {
	const itemOrder = new SurfaceItemOrder();
	const surfaces = new Map<NodeId, FreeSurfaceView>();
	const presentations = new Map<string, SurfacePresentation>();
	const scrolls = new Map<string, SurfaceScroll>();
	const actions = new Map<NodeId, SurfacePresentationActions>();
	const layoutEdits = new Map<NodeId, (command: WidgetLayoutEdit) => Promise<boolean>>();
	const browsers = new Map<NodeId, SurfaceBrowserActions>();
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
			itemOrder.clear();
			scrolls.clear();
			retained = new Map();
			frameGeneration++;
		} else if (previousGraph) {
			graph.nodesById.forEachChange(previousGraph.nodesById, (_id, node, before) => {
				if (before && node?.uuid !== before.uuid) {
					presentations.delete(before.uuid);
					scrolls.delete(before.uuid);
				}
			});
		}
		previousGraph = graph;
	});
	onDestroy(() => {
		destroyed = true;
		frameGeneration++;
		surfaces.clear();
		itemOrder.clear();
		presentations.clear();
		scrolls.clear();
		actions.clear();
		layoutEdits.clear();
		browsers.clear();
		previousGraph = null;
	});
	const clientRect = (id: NodeId): { rect: FreeLayoutRect; anchor: HTMLElement } | null => {
		for (const view of surfaces.values()) {
			const rect = view.geometry.get(id);
			if (!rect || view.geometry.getNode(id)?.uuid !== context.graph?.nodesById.get(id)?.uuid)
				continue;
			const bounds = view.element.getBoundingClientRect();
			const scaleX = bounds.width / Math.max(view.element.clientWidth, 1);
			const scaleY = bounds.height / Math.max(view.element.clientHeight, 1);
			return {
				anchor: view.element,
				rect: {
					left: bounds.left + rect.left * scaleX,
					top: bounds.top + rect.top * scaleY,
					width: rect.width * scaleX,
					height: rect.height * scaleY
				}
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
		const preparation = framePreparation(context.graph, context.page.node_id, ids);
		const prepared = new WeakSet<SurfacePresentationActions>();
		const canceled = () =>
			destroyed || generation !== frameGeneration || context.page.uuid !== identity;
		const measure = (id: NodeId, dom: ReadonlyMap<NodeId, HTMLElement>): boolean => {
			if (!intended.get(id) || context.graph?.nodesById.get(id)?.uuid !== intended.get(id))
				return true;
			const logical = clientRect(id),
				element = dom.get(id);
			const frame = context.frame;
			if ((!logical && !element) || !frame) return false;
			const rect = framePresentationBounds(
				logical?.rect ?? element!.getBoundingClientRect(),
				logical?.anchor ?? element!,
				frame
			);
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
			if (canceled() || !preparation.isCurrent(context.graph, ids)) return null;
			const initial = domIndex();
			for (const id of ids) if (!measure(id, initial)) pending.push(id);
			// Temporary ancestry is bounded per batch and is released before camera animation.
			for (let offset = 0; offset < pending.length; offset += 32) {
				if (canceled()) return null;
				const batch = pending.slice(offset, offset + 32);
				if (!preparation.isCurrent(context.graph, batch)) return null;
				const pins = new Map<NodeId, Set<NodeId>>();
				for (const id of batch) {
					for (const { parent, child } of preparation.paths.get(id) ?? []) {
						let set = pins.get(parent);
						if (!set) pins.set(parent, (set = new Set()));
						set.add(child);
					}
				}
				if (!pins.size) continue;
				retained = new Map([...pins].map(([id, children]) => [id, [...children]]));
				await tick();
				if (canceled()) return null;
				for (const parent of pins.keys()) {
					const presentation = actions.get(parent);
					if (presentation && !prepared.has(presentation)) {
						presentation.prepareForFrame(preparation.currentChildren(context.graph, parent));
						prepared.add(presentation);
					}
				}
				await tick();
				await new Promise(requestAnimationFrame);
				await tick();
				if (canceled() || !preparation.isCurrent(context.graph, batch)) return null;
				const dom = domIndex();
				for (const id of batch) measure(id, dom);
			}
			return preparation.isCurrent(context.graph, ids) ? result : null;
		} finally {
			if (!destroyed && generation === frameGeneration) retained = new Map();
		}
	};
	return {
		itemOrder,
		get previews() {
			return context.previews;
		},
		registerLayout(id, edit) {
			layoutEdits.set(id, edit);
			return () => {
				if (layoutEdits.get(id) === edit) layoutEdits.delete(id);
			};
		},
		editLayout: async (id, command) => (await layoutEdits.get(id)?.(command)) ?? false,
		createNodeWidget: async (surface, target, preview) =>
			(await actions.get(surface)?.createNodeWidget(target, preview)) ?? false,
		readScroll: (uuid) => scrolls.get(uuid),
		writeScroll(uuid, position) {
			if (!destroyed && pageUuid === context.page.uuid) scrolls.set(uuid, position);
		},
		select: (id) => context.select(id),
		registerBrowser(id, browser) {
			browsers.set(id, browser);
			return () => {
				if (browsers.get(id) === browser) browsers.delete(id);
			};
		},
		browse(id) {
			const browser = browsers.get(id);
			if (!browser) return false;
			browser.openMenu();
			return true;
		},
		chooseChild: (surface, child) =>
			browsers.get(surface)?.chooseChild(child) ??
			actions.get(surface)?.chooseChild(child) ??
			false,
		searchChildren: (surface, query) => browsers.get(surface)?.search(query),
		revealChild: async (surface, child) => (await browsers.get(surface)?.reveal(child)) ?? false,
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
