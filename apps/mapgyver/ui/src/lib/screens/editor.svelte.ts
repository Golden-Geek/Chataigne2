import { onDestroy } from 'svelte';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { ScreenEditorController, screenEditorState } from './authoring';
import { SurfaceGesture, createSurfaceGestureState } from './surface-gesture';
import { invokeScreenEdit } from './operations';
import { SCREEN_NAMESPACE } from './generated/constants';
import type { ScreenEditorQuery } from './generated/ScreenEditorQuery';
import type { SurfacePresentation } from './surface-gesture';

export function createScreenEditor(query: () => ScreenEditorQuery) {
	const session = $derived(appState.session);
	const view = $state(screenEditorState());
	const controller = new ScreenEditorController<WorkbenchSession>(
		() => session,
		view,
		invokeScreenEdit
	);
	const gestureState = $state(createSurfaceGestureState());
	let presentation = $state.raw<SurfacePresentation | null>(null);
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
		present: (value) => {
			presentation = value;
		},
		active: (active) => controller.setInteracting(active),
		finished: () => controller.refresh()
	});
	observeEditor({
		query,
		topics: [SCREEN_NAMESPACE],
		pending: () => view.pending,
		interacting: () => view.interacting,
		refresh: (request, resolve) => controller.refresh(request, resolve),
		observe: (shown) => {
			if (!shown) gesture.stop();
			gesture.observe();
		},
		owners: () => {
			const page = controller.page;
			return page?.screen ? [page.screen.id] : [];
		}
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
	return {
		view,
		controller,
		gestureState,
		gesture,
		get presentation() {
			return presentation;
		}
	};
}
