import { SpatialIndex, type SpatialBounds } from 'golden_graph_ui/spatial-index';
import type { GraphState } from '../../../../store/graph.svelte';
import type { NodeId, UiNodeDto } from '../../../../types';
import { getDashboardPlacement, type DashboardPlacement } from '../dashboard-model';
import {
	freeLayoutRectFromPlacement,
	minFreeWidgetHeightRem,
	minFreeWidgetWidthRem,
	rectsIntersect
} from './geometry';
import type { FreeLayoutInteraction, FreeLayoutRect } from './contracts';
import { FreeSurfaceGroups, type FreeSurfaceGroup } from './free-surface-groups';

export const freeSurfaceDetailLimit = 192;
export const freeSurfaceCandidateLimit = 2048;
export const freeSurfaceGroupLimit = 256;

export interface FreeSurfacePresentation {
	readonly overview: boolean;
	readonly ids: readonly NodeId[];
	readonly groups: readonly FreeSurfaceGroup[];
}

export type FreeSurfaceMetrics = Pick<
	FreeLayoutInteraction,
	| 'rootRemPx'
	| 'emPx'
	| 'surfaceWidthPx'
	| 'surfaceHeightPx'
	| 'viewportWidthPx'
	| 'viewportHeightPx'
>;

interface Entry {
	node: UiNodeDto;
	placement: DashboardPlacement;
	dependencies: NodeId[];
	rect: FreeLayoutRect;
}

export const spatialBounds = (rect: FreeLayoutRect): SpatialBounds => ({
	left: rect.left,
	top: rect.top,
	right: rect.left + rect.width,
	bottom: rect.top + rect.height
});

/** Authored free-layout geometry survives widget unmounting and camera movement. */
export class FreeSurfaceGeometry {
	readonly #entries = new Map<NodeId, Entry>();
	readonly #owners = new Map<NodeId, NodeId>();
	readonly #order = new Map<NodeId, number>();
	readonly #groups = new FreeSurfaceGroups();
	#spatial = new SpatialIndex<NodeId>(256);
	#graph: GraphState | null = null;
	#children: readonly NodeId[] | null = null;
	#metrics: FreeSurfaceMetrics | null = null;
	#updated = 0;

	get size(): number {
		return this.#entries.size;
	}
	get updatedEntries(): number {
		return this.#updated;
	}
	get(id: NodeId): FreeLayoutRect | undefined {
		return this.#entries.get(id)?.rect;
	}
	getNode(id: NodeId): UiNodeDto | undefined {
		return this.#entries.get(id)?.node;
	}
	clear(): void {
		this.#entries.clear();
		this.#owners.clear();
		this.#order.clear();
		this.#groups.clear();
		this.#spatial = new SpatialIndex<NodeId>(256);
		this.#graph = null;
		this.#children = null;
		this.#metrics = null;
		this.#updated = 0;
	}

	sync(graph: GraphState, surface: UiNodeDto, metrics: FreeSurfaceMetrics): void {
		this.#updated = 0;
		const dirty = new Set<NodeId>();
		let membershipChanged = false;
		if (surface.children !== this.#children) {
			const members = new Set<NodeId>();
			this.#order.clear();
			let order = 0;
			for (const id of surface.children) {
				const node = graph.nodesById.get(id);
				if (!node || node.user_role !== 'itemRoot') continue;
				members.add(id);
				this.#order.set(id, order);
				const entry = this.#entries.get(id);
				if (entry) {
					entry.node = node;
				} else {
					membershipChanged = true;
					dirty.add(id);
				}
				order++;
			}
			for (const [id, entry] of this.#entries) {
				if (members.has(id)) continue;
				membershipChanged = true;
				for (const dependency of entry.dependencies) this.#owners.delete(dependency);
				this.#entries.delete(id);
				this.#spatial.remove(String(id));
			}
			this.#children = surface.children;
		}
		if (this.#graph && this.#graph.nodesById !== graph.nodesById) {
			graph.nodesById.forEachChange(this.#graph.nodesById, (id) => {
				const knownOwner = this.#owners.get(id);
				if (knownOwner !== undefined) dirty.add(knownOwner);
				else {
					const seen = new Set<NodeId>();
					let owner: NodeId | undefined = id;
					while (owner !== undefined && !seen.has(owner)) {
						seen.add(owner);
						if (graph.parentById.get(owner) === surface.node_id) {
							if (graph.nodesById.get(owner)?.user_role === 'itemRoot') dirty.add(owner);
							break;
						}
						owner = graph.parentById.get(owner);
					}
				}
			});
		}
		const metricsChanged =
			!this.#metrics ||
			Object.keys(metrics).some(
				(key) =>
					metrics[key as keyof FreeSurfaceMetrics] !==
					this.#metrics![key as keyof FreeSurfaceMetrics]
			);
		for (const id of dirty) {
			const node = graph.nodesById.get(id);
			if (!node || node.user_role !== 'itemRoot' || graph.parentById.get(id) !== surface.node_id)
				continue;
			const previous = this.#entries.get(id);
			for (const dependency of previous?.dependencies ?? []) this.#owners.delete(dependency);
			const dependencies: NodeId[] = [];
			const pending = [node];
			const seen = new Set<NodeId>();
			while (pending.length) {
				const current = pending.pop()!;
				if (seen.has(current.node_id)) continue;
				seen.add(current.node_id);
				dependencies.push(current.node_id);
				this.#owners.set(current.node_id, id);
				for (const childId of current.children) {
					const child = graph.nodesById.get(childId);
					if (child && child.user_role !== 'itemRoot') pending.push(child);
				}
			}
			const entry: Entry = {
				node,
				dependencies,
				placement: getDashboardPlacement(graph, node),
				rect: { left: 0, top: 0, width: 0, height: 0 }
			};
			this.#entries.set(id, entry);
			this.#updateRect(id, entry, metrics);
		}
		if (metricsChanged)
			for (const [id, entry] of this.#entries) {
				if (!dirty.has(id)) this.#updateRect(id, entry, metrics);
			}
		if (membershipChanged)
			this.#groups.rebuild([...this.#entries].map(([id, entry]) => [id, entry.rect] as const));
		this.#metrics = { ...metrics };
		this.#graph = graph;
	}

	query(rect: FreeLayoutRect): NodeId[] {
		return this.#spatial
			.query(spatialBounds(rect))
			.filter((id) => rectsIntersect(rect, this.#entries.get(id)!.rect))
			.sort((a, b) => this.#order.get(a)! - this.#order.get(b)!);
	}

	presentation(
		rect: FreeLayoutRect,
		detailLimit = freeSurfaceDetailLimit
	): FreeSurfacePresentation {
		const result = this.#spatial.queryLimited(spatialBounds(rect), freeSurfaceCandidateLimit);
		const ids = result.values.filter((id) => rectsIntersect(rect, this.#entries.get(id)!.rect));
		if (result.complete && ids.length <= Math.max(0, Math.min(freeSurfaceDetailLimit, detailLimit)))
			return {
				overview: false,
				ids: ids.sort((a, b) => this.#order.get(a)! - this.#order.get(b)!),
				groups: []
			};
		return {
			overview: true,
			ids: [],
			groups: this.#groups.query(rect, Math.max(1, Math.min(freeSurfaceGroupLimit, detailLimit)))
		};
	}

	/** Explicit activation resolves actual paint order; overview bounds never decide identity. */
	hitTest(x: number, y: number): NodeId | null {
		let result: NodeId | null = null;
		for (const id of this.#spatial.query({ left: x, right: x, top: y, bottom: y })) {
			const rect = this.#entries.get(id)!.rect;
			if (
				x < rect.left ||
				y < rect.top ||
				x >= rect.left + rect.width ||
				y >= rect.top + rect.height
			)
				continue;
			if (result === null || this.#order.get(id)! > this.#order.get(result)!) result = id;
		}
		return result;
	}

	orderedNodes(ids: Iterable<NodeId>): UiNodeDto[] {
		return [...ids]
			.filter((id) => this.#entries.has(id))
			.sort((a, b) => this.#order.get(a)! - this.#order.get(b)!)
			.map((id) => this.#entries.get(id)!.node);
	}

	#updateRect(id: NodeId, entry: Entry, metrics: FreeSurfaceMetrics): void {
		const rect = freeLayoutRectFromPlacement(entry.placement, metrics);
		// The free slot applies the same minimum to both its size and its anchor offset.
		const minimumWidth = minFreeWidgetWidthRem * metrics.rootRemPx;
		const minimumHeight = minFreeWidgetHeightRem * metrics.rootRemPx;
		const clamped = freeLayoutRectFromPlacement(
			{
				...entry.placement,
				size: {
					width: { value: Math.max(rect.width, minimumWidth), unit: 'px' },
					height: { value: Math.max(rect.height, minimumHeight), unit: 'px' }
				}
			},
			metrics
		);
		entry.rect = clamped;
		this.#groups.update(id, clamped);
		if (this.#spatial.get(String(id)) !== undefined)
			this.#spatial.update(String(id), spatialBounds(clamped), id);
		else this.#spatial.insert(String(id), spatialBounds(clamped), id);
		this.#updated++;
	}
}
