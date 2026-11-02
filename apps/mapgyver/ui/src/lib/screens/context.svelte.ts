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
	const inspectedOwner = $derived.by(() => {
		const current = session;
		let node = current?.selectedNodeId == null ? null : current.getNodeData(current.selectedNodeId);
		// A selected Parameter/folder still belongs to the same bounded authored owner.
		for (let depth = 0; node && current && depth < 3; depth++) {
			if (node.node_type === 'mapgyver.screen' || node.node_type === 'mapgyver.surface')
				return node;
			const parent = current.graph.state.parentById.get(node.node_id);
			node = parent == null ? null : current.getNodeData(parent);
		}
		return null;
	});
	const inspectedParameters = $derived.by(() => {
		const current = session;
		const folderId = inspectedOwner?.children.find(
			(id) => current?.getNodeData(id)?.decl_id === 'parameters'
		);
		const folder = folderId == null ? null : current?.getNodeData(folderId);
		return folder?.children ?? [];
	});
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
			if (inspecting) {
				if (!current?.hasActiveEditSession) void controller.refresh();
			} else gesture.stop();
		});
	});
	$effect(() => {
		const current = session;
		const owner = inspectedOwner;
		if (views === 0 || !current?.hasActiveEditSession || !owner) return;
		// Subscribe only to this owner's real Parameter leaves. Read intents project
		// backend definitions; the UI does not reconstruct domain geometry from them.
		for (const id of inspectedParameters) void current.graph.state.paramsById.get(id)?.value;
		const parentId = current.graph.state.parentById.get(owner.node_id);
		const parent = parentId == null ? null : current.getNodeData(parentId);
		untrack(() => {
			if (owner.node_type === 'mapgyver.surface' && parent?.node_type === 'mapgyver.screen')
				void controller.refreshOwner({
					kind: 'InspectSurface',
					reference: { screen: parent.uuid, surface: owner.uuid }
				});
			else if (owner.node_type === 'mapgyver.screen')
				void controller.refreshOwner({ kind: 'InspectScreen', screen: owner.uuid });
		});
	});
	onDestroy(() => {
		gesture.dispose();
		controller.dispose();
	});
	$effect(() => {
		const current = session;
		const node =
			current?.selectedNodeId == null ? null : current.getNodeData(current.selectedNodeId);
		const catalog = view.catalog;
		if (!node || !catalog) return;
		const screen = catalog.screens.find(
			(screen) =>
				screen.id === node.uuid ||
				screen.surfaces.some((surface) => surface.reference.surface === node.uuid)
		);
		if (!screen) return;
		untrack(() => {
			if (node.node_type === 'mapgyver.screen') controller.selectScreen(screen.id);
			else if (node.node_type === 'mapgyver.surface') {
				view.selectedScreen = screen.id;
				view.selectedSurface = node.uuid;
			}
		});
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
