import { onDestroy } from 'svelte';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { projectSurface } from './parameter-projection';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { ScreenEditorController, screenEditorState } from './authoring';
import { SurfaceGesture, createSurfaceGestureState } from './surface-gesture';
import { invokeScreenEdit } from './operations';
import { SCREEN_NAMESPACE } from './generated/constants';
import type { ScreenEditorQuery } from './generated/ScreenEditorQuery';
import type { SurfacePresentation } from './surface-gesture';
import { warpEditGroup, type CornerLinkEdit } from './warp-edit-group';

export function createScreenEditor(query: () => ScreenEditorQuery) {
	const panel = usePanelVisibility();
	const session = $derived(appState.session);
	const view = $state(screenEditorState());
	const controller = new ScreenEditorController<WorkbenchSession>(
		() => session,
		view,
		invokeScreenEdit
	);
	const gestureState = $state(createSurfaceGestureState());
	const projection = createEditorParameterProjection(() => session);
	const parameters = $derived(
		panel.visible && view.page ? projection.read(view.presentedGeneration, view.page.at) : null
	);
	const bindings = $derived(
		new Map(view.page?.parameters.map((binding) => [binding.surface, binding]) ?? [])
	);
	const surface = $derived(
		controller.surface && parameters
			? projectSurface(
					controller.surface,
					bindings.get(controller.surface.reference.surface),
					parameters
				)
			: controller.surface
	);
	const screen = $derived(
		controller.screen && parameters
			? {
					...controller.screen,
					surfaces: controller.screen.surfaces.map((value) =>
						projectSurface(value, bindings.get(value.reference.surface), parameters)
					)
				}
			: controller.screen
	);
	let presentation = $state.raw<SurfacePresentation | null>(null);
	let finalLink: CornerLinkEdit | null = null;
	const gesture = new SurfaceGesture<WorkbenchSession>(() => session, gestureState, {
		group: (current, handle) => {
			finalLink = null;
			const generation = current.snapshotGeneration,
				project = current.projectGeneration;
			const group = createUiEditSession(
				handle.kind === 'QuadCorner' ||
					handle.kind === 'QuadCornerPixels' ||
					handle.kind === 'QuadTranslation'
					? 'Warp Surface'
					: 'Crop Surface',
				'surface-geometry',
				current
			);
			return warpEditGroup(
				group,
				() =>
					current === session &&
					current.snapshotGeneration === generation &&
					current.projectGeneration === project &&
					current.status === 'connected' &&
					!current.loading.visible &&
					!gestureState.error,
				() => {
					const link = finalLink;
					finalLink = null;
					return link;
				},
				(edit) => invokeScreenEdit(current, edit, projection.applied())
			);
		},
		move: async (current, reference, edit) => {
			const result = await invokeScreenEdit(
				current,
				{
					kind: 'EditSurfaceGeometry',
					reference,
					edit
				},
				projection.applied()
			);
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
		finishWarp(link: CornerLinkEdit | null) {
			finalLink = link;
			gesture.finish();
		},
		get screen() {
			return screen;
		},
		get surface() {
			return surface;
		},
		get presentation() {
			return presentation;
		}
	};
}
