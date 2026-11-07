import { onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createLayerGesture, type LayerHandle, type LayerPresentation } from './layer-gesture';
import { invokeMediaEdit } from '../media/operations';
import type { createMatrixEditor } from '../matrix/editor.svelte';

export function createLayerEditor(
	owner: ReturnType<typeof createMatrixEditor>,
	present: () => boolean
) {
	const state = $state(createLatestEditGestureState());
	let activeLayer = $state<string | null>(null);
	let presentation = $state.raw<LayerPresentation | null>(null);
	const gesture = createLayerGesture<WorkbenchSession>(() => owner.session, state, {
		group: (session, handle) =>
			createUiEditSession(
				`${typeof handle.mode === 'string' ? handle.mode : 'Resize'} Layer`,
				'matrix-geometry',
				session
			),
		invoke: invokeMediaEdit,
		accepted: (result) => owner.controller.acceptLayer(result),
		present: (value) => {
			presentation = value;
		},
		active: (active) => {
			owner.controller.setInteracting(active);
			if (!active) activeLayer = null;
		},
		finished: () => owner.controller.refresh()
	});
	$effect(() => {
		const session = owner.session;
		if (session) {
			void session.status;
			void session.loading.visible;
			void session.snapshotGeneration;
			void session.projectGeneration;
			void session.hasActiveEditSession;
			void session.editSessionEpoch;
		}
		const available = present(),
			selected = appState.session?.getSelectedNodes()[0]?.uuid;
		untrack(() => {
			gesture.observe();
			if (!available || (activeLayer && selected !== activeLayer)) gesture.stop();
		});
	});
	onDestroy(() => gesture.dispose());
	return {
		state,
		gesture,
		get presentation() {
			return presentation;
		},
		get activeLayer() {
			return activeLayer;
		},
		start(layer: string, handle: LayerHandle) {
			const media = owner.matrixWindow?.media;
			if (!media || owner.busy || !present() || !gesture.start({ media, layer }, handle))
				return false;
			activeLayer = layer;
			return true;
		}
	};
}
