import { getContext, setContext, onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createCompositionAuthoringState, CompositionAuthoringController } from './authoring';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createInstanceGesture } from './instance-gesture';
import { invokeMediaEdit } from '../media/operations';
import { MEDIA_NAMESPACE } from '../media/generated/constants';
const key = Symbol('mapgyver-composition-editor');
export function provideCompositionAuthoring() {
	const session = $derived(appState.session);
	const view = $state(createCompositionAuthoringState());
	const controller = new CompositionAuthoringController<WorkbenchSession>(
		() => session,
		view,
		invokeMediaEdit
	);
	const channel = $derived(controller.channel);
	const instance = $derived(controller.instance);
	const busy = $derived(controller.busy || session?.hasActiveEditSession === true);
	const gestureState = $state(createLatestEditGestureState());
	const gesture = createInstanceGesture<WorkbenchSession>(() => session, gestureState, {
		group: (session, handle) =>
			createUiEditSession(`${handle.mode} instance`, 'composition-geometry', session),
		invoke: invokeMediaEdit,
		accepted: (snapshot) => controller.acceptInstance(snapshot),
		active: (active) => controller.setInteracting(active),
		finished: () => {
			void controller.refresh();
		}
	});
	const pageSize = 50;
	let page = $state(0);
	const instances = $derived(view.composition?.instances ?? []);
	const pages = $derived(Math.max(1, Math.ceil(instances.length / pageSize)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const start = $derived(currentPage * pageSize);
	const selectedIndex = $derived(
		instances.findIndex((item) => item.reference.instance === view.instance)
	);
	$effect(() => {
		if (selectedIndex >= 0) page = Math.floor(selectedIndex / pageSize);
	});
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			gesture.observe();
			if (!current?.hasActiveEditSession) void controller.refresh();
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
		const graph = current?.graph.state,
			catalog = view.catalog;
		if (!node || !graph || !catalog) return;
		let channelNode = node;
		if (['mapgyver.media.v5', 'mapgyver.media.reference.v1'].includes(node.node_type)) {
			const parent = graph.parentById.get(node.node_id);
			if (parent === undefined) return;
			const found = graph.nodesById.get(parent);
			if (found?.node_type === 'mapgyver.media.v5') {
				const output = found.children
					.map((id) => current?.getNodeData(id))
					.find((node) => node?.node_type === 'mapgyver.media.channel');
				if (output) channelNode = output;
			}
		}
		const found = catalog.media
			.flatMap((media) => media.channels)
			.find(
				(channel) =>
					channel.reference.channel === channelNode.uuid &&
					channel.definition.source.kind === 'Composition'
			);
		if (!found) return;
		untrack(() => {
			if (view.selected?.channel !== found.reference.channel) controller.select(found.reference);
			if (
				found.reference.media !== node.uuid &&
				['mapgyver.media.v5', 'mapgyver.media.reference.v1'].includes(node.node_type)
			)
				view.instance = node.uuid;
		});
	});
	const owner = {
		view,
		controller,
		gestureState,
		gesture,
		get session() {
			return session;
		},
		get channel() {
			return channel;
		},
		get instance() {
			return instance;
		},
		get busy() {
			return busy;
		},
		get instances() {
			return instances;
		},
		get pages() {
			return pages;
		},
		get currentPage() {
			return currentPage;
		},
		get start() {
			return start;
		},
		get selectedIndex() {
			return selectedIndex;
		},
		get page() {
			return page;
		},
		set page(value: number) {
			page = value;
		},
		pageSize
	};
	setContext(key, owner);
	return owner;
}
export const useCompositionAuthoring = () =>
	getContext<ReturnType<typeof provideCompositionAuthoring>>(key);
