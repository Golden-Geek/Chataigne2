import { getContext, setContext, onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { invokeScreenEdit } from './operations';
import { invokeMediaEdit } from '../media/operations';
import { SCREEN_NAMESPACE } from './generated/constants';
import { MEDIA_NAMESPACE } from '../media/generated/constants';
import { createScreenAuthoringState, ScreenAuthoringController } from './authoring';
import { SurfaceGesture, createSurfaceGestureState } from './surface-gesture';

const key = Symbol('mapgyver-screen-authoring');

/** One window owner: docked views share selection, inspection and the active gesture. */
export function provideScreenAuthoring() {
	let views = $state(0);
	const session = $derived(appState.session);
	const view = $state(createScreenAuthoringState());
	const controller = new ScreenAuthoringController<WorkbenchSession>(() => session, view, {
		screens: invokeScreenEdit,
		media: async (current) => {
			const result = await invokeMediaEdit(current, { kind: 'InspectManager' });
			if (result.kind !== 'Manager')
				throw new Error('Media inspection returned an unexpected result');
			return result.snapshot;
		}
	});
	const gestureState = $state(createSurfaceGestureState());
	const gesture = new SurfaceGesture<WorkbenchSession>(() => session, gestureState, {
		group: (current, handle) =>
			createUiEditSession(
				handle.kind === 'QuadCorner' ? 'Warp Surface' : 'Crop Surface',
				'surface-geometry',
				current
			),
		move: async (current, reference, edit) => {
			const result = await invokeScreenEdit(current, {
				kind: 'EditSurfaceGeometry',
				reference,
				edit
			});
			if (result.kind !== 'SurfaceChanged')
				throw new Error('Geometry edit returned an unexpected result');
			return result.snapshot;
		},
		accepted: (snapshot) => controller.acceptSurface(snapshot),
		active: (active) => controller.setInteracting(active),
		finished: () => {
			if (views > 0) void controller.refresh();
		}
	});
	$effect(() => {
		const inspecting = views > 0;
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(SCREEN_NAMESPACE);
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			gesture.observe();
			if (inspecting) void controller.refresh();
			else gesture.stop();
		});
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
	const owner = {
		attach() {
			views++;
			return () => {
				views--;
			};
		},
		view,
		controller,
		gestureState,
		gesture,
		get session() {
			return session;
		}
	};
	setContext(key, owner);
	return owner;
}

export const useScreenAuthoring = () => getContext<ReturnType<typeof provideScreenAuthoring>>(key);
