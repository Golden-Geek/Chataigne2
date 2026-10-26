import type { UiNodeDto, WorkbenchSession } from 'golden_ui';
import type { GraphNavigationItem, GraphWorldNavigation } from 'golden_graph_ui';
import type { SpatialEndpoint, SpatializerCanvas } from './types';
import type { SpatializerSpace } from './generated/SpatializerSpace';
import { SPATIALIZER_UNIT_REM } from './constants';

interface Source {
	readonly endpoints: readonly SpatialEndpoint[];
	readonly session: WorkbenchSession | null;
	readonly module: UiNodeDto | null;
	readonly space: SpatializerSpace;
	readonly mode: string;
	readonly canvas: SpatializerCanvas | null;
}

/** A point-world catalog; no synthetic graph nodes or backend mutation policy. */
export function createSpatializerNavigation(source: Source) {
	let endpoints = $derived(source.endpoints);
	let byKey = $derived(new Map(endpoints.map((endpoint) => [endpoint.key, endpoint])));
	let byNode = $derived(new Map(endpoints.map((endpoint) => [endpoint.node.node_id, endpoint])));
	let owner = $derived(source.session);
	let epoch = $derived(owner?.snapshotGeneration);
	let moduleUuid = $derived(source.module?.uuid);
	let space = $derived(source.space);
	let identity = $derived({ owner, epoch, moduleUuid, space });
	let presentationRevision = $derived({ endpoints, mode: source.mode });
	let previousItems: readonly GraphNavigationItem[] = [];
	let items = $derived.by(() => {
		if (
			previousItems.length === endpoints.length &&
			endpoints.every(
				(endpoint, index) =>
					previousItems[index].id === endpoint.key &&
					previousItems[index].label === endpoint.node.meta.label
			)
		) {
			return previousItems;
		}
		previousItems = endpoints.map((endpoint) => ({
			id: endpoint.key,
			label: endpoint.node.meta.label
		}));
		return previousItems;
	});

	const world: GraphWorldNavigation = {
		findLabel: 'Find endpoints',
		get source() {
			return identity;
		},
		get presentationRevision() {
			return presentationRevision;
		},
		get items() {
			return items;
		},
		get selectedIds() {
			return (source.session?.selectedNodesIds ?? []).flatMap((id) => {
				const endpoint = byNode.get(id);
				return endpoint ? [endpoint.key] : [];
			});
		},
		bounds(id) {
			const endpoint = byKey.get(id);
			const parameter = endpoint?.positionParam?.data;
			if (
				!endpoint ||
				parameter?.kind !== 'parameter' ||
				!['vec2', 'vec3'].includes(parameter.param.value.kind) ||
				!Number.isFinite(endpoint.x) ||
				!Number.isFinite(endpoint.y)
			)
				return null;
			const padding = Math.max(
				1,
				(endpoint.radius ?? 0) * SPATIALIZER_UNIT_REM,
				(endpoint.freezeRadius ?? 0) * SPATIALIZER_UNIT_REM
			);
			const x = endpoint.x * SPATIALIZER_UNIT_REM,
				y = endpoint.y * SPATIALIZER_UNIT_REM;
			return { left: x - padding, top: y - padding, right: x + padding, bottom: y + padding };
		},
		select(id, additive) {
			const endpoint = byKey.get(id);
			if (endpoint) source.session?.selectNode(endpoint.node.node_id, additive ? 'ADD' : 'REPLACE');
		},
		focusTarget(id, canvas) {
			const endpoint = byKey.get(id);
			if (!endpoint) return null;
			return canvas.querySelector<SVGCircleElement>(
				`g[data-endpoint-key="${CSS.escape(id)}"][data-endpoint-uuid="${CSS.escape(endpoint.node.uuid)}"] circle.endpoint-hit`
			);
		},
		itemIdForElement(element) {
			const group = element.closest('g[data-endpoint-key]');
			const key = group?.getAttribute('data-endpoint-key');
			const endpoint = key ? byKey.get(key) : null;
			return endpoint && group?.getAttribute('data-endpoint-uuid') === endpoint.node.uuid
				? endpoint.key
				: null;
		}
	};
	return {
		world,
		async find(query: string) {
			await source.canvas?.showNodeFinder();
			await source.canvas?.searchNodes(query);
			return source.canvas?.nodeFinderState();
		},
		async page(direction: 'next' | 'previous') {
			await source.canvas?.pageNodes(direction);
			return source.canvas?.nodeFinderState();
		},
		reveal(nodeId: number, additive = false) {
			const endpoint = byNode.get(nodeId);
			return endpoint && source.canvas
				? source.canvas.revealNode(endpoint.key, additive)
				: Promise.resolve(false);
		},
		frame: () => source.canvas?.frameSelection(),
		viewport: () => source.canvas?.navigationState()
	};
}
