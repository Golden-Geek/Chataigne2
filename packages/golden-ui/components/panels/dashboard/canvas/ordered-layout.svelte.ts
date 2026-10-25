import { createConstrainedResize } from './constrained-resize.svelte';
import { createOrderedMove } from './ordered-move.svelte';
import type { DashboardOrderedLayoutContext } from './ordered-layout-context';
import type { WidgetLayoutEdit, FreeLayoutResizeEdges } from './contracts';

/** Composition and native adapters share the same move/resize operations. */
export function createDashboardOrderedLayout(context: DashboardOrderedLayoutContext) {
	const move = createOrderedMove(context),
		resize = createConstrainedResize(context);
	return {
		get widgetMoveInteraction() {
			return move.interaction;
		},
		get constrainedResizeInteraction() {
			return resize.interaction;
		},
		get supportsOrdering() {
			return context.editMode && context.parentLayoutKind !== 'free';
		},
		resetOrderedDropTarget: move.resetDropTarget,
		beginWidgetMoveInteraction(event: PointerEvent) {
			void resize.finish(true);
			move.begin(event);
		},
		beginConstrainedResize(event: PointerEvent, edges: FreeLayoutResizeEdges) {
			void move.finish(true);
			resize.begin(event, edges);
		},
		async edit(command: WidgetLayoutEdit): Promise<boolean> {
			switch (command.kind) {
				case 'move':
					void resize.finish(true);
					return move.start(command.origin, context.widgetElement);
				case 'resize':
					void move.finish(true);
					return resize.start(command.origin, command.edges, context.widgetElement);
				case 'update':
					return move.interaction ? move.update(command.point) : resize.update(command.point);
				case 'finish':
					return move.interaction ? move.finish(false) : resize.finish(false);
				case 'cancel':
					return move.interaction ? move.finish(true) : resize.finish(true);
			}
		}
	};
}
