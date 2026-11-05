import { onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { ScreenEditorController, screenEditorState } from './authoring';
import { SurfaceGesture, createSurfaceGestureState } from './surface-gesture';
import { invokeScreenEdit } from './operations';
import { SCREEN_NAMESPACE } from './generated/constants';
import type { ScreenEditorQuery } from './generated/ScreenEditorQuery';

export function createScreenEditor(query: () => ScreenEditorQuery) {
	const session = $derived(appState.session);
	const view = $state(screenEditorState());
	const controller = new ScreenEditorController<WorkbenchSession>(
		() => session,
		view,
		invokeScreenEdit
	);
	const gestureState = $state(createSurfaceGestureState());
	const gesture = new SurfaceGesture<WorkbenchSession>(() => session, gestureState, {
		group: (current, handle) =>
			createUiEditSession(
				handle.kind === 'QuadCorner' ||
					handle.kind === 'QuadCornerPixels' ||
					handle.kind === 'QuadTranslation'
					? 'Warp Surface'
					: 'Crop Surface',
				'surface-geometry',
				current
			),
		move: async (current, reference, edit) => {
			const result = await invokeScreenEdit(current, {
				kind: 'EditSurfaceGeometry',
				reference,
				edit
			});
			if (result.kind !== 'SurfaceChanged') throw new Error('Unexpected Surface geometry response');
			return result.snapshot;
		},
		accepted: (snapshot) => controller.acceptSurface(snapshot),
		active: (active) => controller.setInteracting(active),
		finished: () => controller.refresh()
	});
	$effect(() => {
		const current = session,
			request = query();
		void view.pending;
		void view.interacting;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(SCREEN_NAMESPACE);
			// Live Inspector feedback reads only backend-resolved owners' typed leaves.
			if (current.hasActiveEditSession) {
				for (const uuid of [
					controller.page?.screen?.id,
					controller.page?.surface?.reference.surface
				]) {
					const node = uuid ? current.graph.state.nodesById.getByUuid(uuid) : null;
					const folder = node?.children
						.map((id) => current.getNodeData(id))
						.find((node) => node?.decl_id === 'parameters');
					for (const id of folder?.children ?? [])
						void current.graph.state.paramsById.get(id)?.value;
				}
			}
		}
		untrack(() => {
			gesture.observe();
			controller.refresh(request);
		});
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
	return { view, controller, gestureState, gesture };
}
