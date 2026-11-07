import { onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { ScreenEditorController, screenEditorState } from './authoring';
import { SurfaceGesture, createSurfaceGestureState } from './surface-gesture';
import { invokeScreenEdit } from './operations';
import { SCREEN_NAMESPACE } from './generated/constants';
import type { ScreenEditorQuery } from './generated/ScreenEditorQuery';
import type { SurfacePresentation } from './surface-gesture';

export function createScreenEditor(query: () => ScreenEditorQuery, visible: () => boolean) {
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
	let previouslyShown = false;
	$effect(() => {
		const current = session,
			request = query(),
			shown = visible();
		void view.pending;
		void view.interacting;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			void current.hasActiveEditSession;
			if (!current.hasActiveEditSession) void current.currentHistoryStateId;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(SCREEN_NAMESPACE);
		}
		untrack(() => {
			if (!shown) gesture.stop();
			gesture.observe();
			controller.refresh(request, shown && !previouslyShown);
			previouslyShown = shown;
		});
	});
	let observed: { session: WorkbenchSession; owners: string; values: string } | null = null;
	$effect(() => {
		// Retained dock tabs still follow routing/metadata above, but do not query every
		// live Parameter change. Becoming visible explicitly refreshes their snapshot.
		if (!visible()) {
			observed = null;
			return;
		}
		const current = session,
			page = controller.page;
		if (!current || current.status !== 'connected' || current.loading.visible || !page?.screen) {
			observed = null;
			return;
		}
		// Follow bounded backend-resolved owners, including a selected off-page Surface.
		// Snapshot replies must not themselves schedule another identical inspection.
		const owners = [
			...new Set([
				page.screen.id,
				...page.screen.surfaces.map((item) => item.reference.surface),
				...(page.surface ? [page.surface.reference.surface] : [])
			])
		];
		const ownerKey = JSON.stringify([
			current.snapshotGeneration,
			current.projectGeneration,
			owners
		]);
		const values = JSON.stringify(
			owners.map((uuid) => {
				const node = current.graph.state.nodesById.getByUuid(uuid);
				const folder = node?.children
					.map((id) => current.getNodeData(id))
					.find((item) => item?.decl_id === 'parameters');
				return folder?.children.map((id) => current.graph.state.paramsById.get(id)?.value);
			})
		);
		const changed =
			observed?.session === current && observed.owners === ownerKey && observed.values !== values;
		observed = { session: current, owners: ownerKey, values };
		if (changed)
			untrack(() => {
				if (!view.interacting) controller.refresh();
			});
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
