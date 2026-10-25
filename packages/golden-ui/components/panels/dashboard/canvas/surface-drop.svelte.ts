import { onDestroy } from 'svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { UiNodeDto } from '../../../../types';
import type { SurfaceDropPreview } from './contracts';
import { captureSurfaceDrop } from './surface-fence';

interface SurfaceDropContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly enabled: boolean;
	clearIndicator(): void;
	create(source: UiNodeDto, preview: SurfaceDropPreview | null): Promise<boolean>;
}
type Drop = {
	preview: SurfaceDropPreview | null;
	fence: NonNullable<ReturnType<typeof captureSurfaceDrop>>;
};

/** Native adapters and explicit creation operations share one presentation lifetime. */
export function createSurfaceDrop(context: SurfaceDropContext) {
	let active = $state.raw<Drop | null>(null);
	let pending = $state.raw<Drop | null>(null);
	let depth = $state(0);
	let destroyed = false;
	const clear = () => {
		active = null;
		depth = 0;
		context.clearIndicator();
	};
	const capture = (source: UiNodeDto, preview: SurfaceDropPreview | null): Drop | null => {
		if (destroyed || !context.enabled) return null;
		const fence = captureSurfaceDrop(context.graph, context.node.node_id, source);
		return fence ? { preview, fence } : null;
	};
	$effect(() => {
		if (active && (!context.enabled || !active.fence.isCurrent(context.graph))) clear();
		if (pending && (!context.enabled || !pending.fence.isCurrent(context.graph))) pending = null;
	});
	$effect(() => {
		if (!pending) return;
		const timer = setTimeout(() => {
			pending = null;
		}, 2000);
		return () => clearTimeout(timer);
	});
	onDestroy(() => {
		destroyed = true;
		clear();
		pending = null;
	});
	return {
		clear,
		enter(source: UiNodeDto, preview: SurfaceDropPreview | null) {
			const next = capture(source, preview);
			if (!next) {
				clear();
				return;
			}
			active = next;
			depth++;
		},
		over(source: UiNodeDto, preview: SurfaceDropPreview | null) {
			active = capture(source, preview);
			if (!active) clear();
		},
		leave() {
			if (depth <= 1) clear();
			else depth--;
		},
		async create(source: UiNodeDto, preview: SurfaceDropPreview | null): Promise<boolean> {
			clear();
			const operation = capture(source, preview);
			if (!operation) return false;
			pending = operation;
			try {
				const created = await context.create(source, preview);
				if (!created && pending === operation) pending = null;
				return created;
			} catch (error) {
				if (pending === operation) pending = null;
				throw error;
			}
		},
		get depth() {
			return depth;
		},
		get preview() {
			return active?.preview ?? null;
		},
		get pending() {
			return pending;
		}
	};
}
