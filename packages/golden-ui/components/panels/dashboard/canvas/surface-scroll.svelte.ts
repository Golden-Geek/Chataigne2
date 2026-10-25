import { tick, untrack } from 'svelte';
import type { GraphState } from '../../../../store/graph.svelte';
import type { UiNodeDto } from '../../../../types';
import type { DashboardViewContext, SurfaceScroll } from './view-context.svelte';
import { revealInSurface } from './frame-bounds';

interface SurfaceScrollContext {
	readonly element: HTMLElement | null;
	readonly node: UiNodeDto;
	readonly graph: GraphState | null;
	readonly view: DashboardViewContext | null;
}

export function createSurfaceScroll(context: SurfaceScrollContext) {
	let binding: {
		element: HTMLElement;
		node: UiNodeDto;
		view: DashboardViewContext;
		last: SurfaceScroll;
	} | null = null;
	const save = (state: NonNullable<typeof binding>) => {
		if (context.graph?.nodesById.get(state.node.node_id)?.uuid === state.node.uuid)
			state.view.writeScroll(state.node.uuid, state.last);
	};
	const measure = (state: NonNullable<typeof binding>) => {
		state.last = { left: state.element.scrollLeft, top: state.element.scrollTop };
		save(state);
	};
	$effect(() => {
		const element = context.element,
			node = context.node,
			view = context.view;
		if (!element || !view) return;
		const cached = untrack(() => view.readScroll(node.uuid));
		const state = {
			element,
			node,
			view,
			last: cached ?? { left: element.scrollLeft, top: element.scrollTop }
		};
		binding = state;
		const changed = () => measure(state);
		element.addEventListener('scroll', changed, { passive: true });
		void tick().then(() => {
			if (binding !== state) return;
			if (cached) {
				element.scrollLeft = cached.left;
				element.scrollTop = cached.top;
			}
			measure(state);
		});
		return () =>
			untrack(() => {
				// Teardown may already have removed content and clamped DOM scrolling to zero.
				save(state);
				element.removeEventListener('scroll', changed);
				if (binding === state) binding = null;
			});
	});
	return {
		reveal(target: HTMLElement) {
			if (!binding) return;
			revealInSurface(binding.element, target);
			measure(binding);
		}
	};
}
