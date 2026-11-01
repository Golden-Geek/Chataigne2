import { getContext, setContext, onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession, type UiNodeDto } from 'golden_ui';
import { MatrixController, matrixState } from './authoring';
import { choiceState } from './choices';
import { invokeMediaEdit } from '../media/operations';
import { channelKey } from '../media/identity';
import { MEDIA_NAMESPACE, MATRIX_PLAYBACK_EVENT } from '../media/generated/constants';
import { usePreview } from '../preview/context.svelte';
import { previewKey } from '../preview/identity';
import { matrixFeedback } from './render-feedback';
import { useDocumentSelection } from '../selection.svelte';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
const key = Symbol('mapgyver-matrix-editor');
export function provideMatrixAuthoring() {
	const session = $derived(appState.session);
	const documentSelection = useDocumentSelection();
	const previewOwner = usePreview();
	const view = $state(matrixState()),
		matrices = $state(choiceState()),
		sources = $state(choiceState());
	const controller = new MatrixController<WorkbenchSession>(
		() => session,
		view,
		matrices,
		sources,
		invokeMediaEdit
	);
	const busy = $derived(controller.busy || session?.hasActiveEditSession === true);
	const matrixWindow = $derived(controller.window);
	const observation = $derived(
		matrixFeedback(
			matrixWindow,
			matrixWindow
				? previewOwner.statuses.get(
						previewKey({
							kind: 'Channel',
							reference: matrixWindow.final_output.reference
						})
					)
				: undefined,
			session?.status === 'connected' && !session.loading.visible
		)
	);
	const selectedRow = $derived(
		matrixWindow?.rows.find((row) => row.item.id.id === view.selected?.item?.id)
	);
	const selectedColumn = $derived(
		matrixWindow?.columns.find((column) => column.id === view.selected?.column)
	);
	const selectedCell = $derived(
		matrixWindow?.cells.find(
			(value) =>
				value.cell.layer === view.selected?.item?.id &&
				value.cell.definition.column === view.selected?.column
		)
	);
	let draft = $state('');
	let draftKey = '';
	const definition = $derived.by(() => {
		const source = matrixWindow?.final_output.definition.source;
		return source?.kind === 'Matrix' && source.output.kind === 'Final'
			? source.output.definition
			: null;
	});
	$effect(() => {
		const source = selectedCell?.cell.definition.source;
		const key = JSON.stringify([view.media, view.selected, source]);
		untrack(() => {
			if (key !== draftKey) {
				draftKey = key;
				draft = source ? channelKey(source) : '';
			}
		});
	});
	$effect(() => {
		const current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.editSessionEpoch;
			void current.hasActiveEditSession;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => {
			if (!current?.hasActiveEditSession) controller.observe();
		});
	});
	$effect(() => {
		const current = session;
		if (current) void current.getCustomEventSequence(MATRIX_PLAYBACK_EVENT);
		untrack(() => {
			if (!current?.hasActiveEditSession) controller.observe(false);
		});
	});
	onDestroy(() => controller.dispose());

	$effect(() => {
		const current = session;
		const node =
			current?.selectedNodeId == null ? null : current.getNodeData(current.selectedNodeId);
		const graph = current?.graph.state;
		if (!node || !graph) return;
		let media: UiNodeDto | undefined = node;
		while (media && media.node_type !== 'mapgyver.media') {
			const parent = graph.parentById.get(media.node_id);
			media = parent === undefined ? undefined : graph.nodesById.get(parent);
		}
		const choices = matrices.page;
		const window = matrixWindow;
		untrack(() => {
			if (
				media?.node_type === 'mapgyver.media' &&
				choices?.choices.some((choice) => choice.reference.media === media.uuid)
			) {
				if (view.media !== media.uuid) controller.selectMedia(media.uuid);
			}
			const row = window?.rows.find((row) => row.item.id.id === node.uuid);
			const column = window?.columns.find((column) => column.id === node.uuid);
			if (row && view.selected?.item?.id !== row.item.id.id)
				controller.select(row.item.id, view.selected?.column ?? null);
			else if (column && view.selected?.column !== column.id)
				controller.select(view.selected?.item ?? null, column.id);
		});
	});
	const owner = {
		async create() {
			const selectCreated = documentSelection.capture();
			const result = await controller.create();
			if (result?.kind === 'Created') selectCreated(result.reference.channel);
			return result;
		},
		async edit(intent: MatrixEdit) {
			const selectParent = documentSelection.capture();
			const parent = matrixWindow?.final_output.reference.channel;
			const result = await controller.edit(intent);
			if (
				result?.kind === 'Changed' &&
				(intent.kind === 'RemoveItem' || intent.kind === 'RemoveColumn') &&
				parent
			) {
				if (selectParent(parent)) controller.select(null, null);
			}
			return result;
		},
		view,
		controller,
		matrices,
		sources,
		get session() {
			return session;
		},
		get busy() {
			return busy;
		},
		get matrixWindow() {
			return matrixWindow;
		},
		get observation() {
			return observation;
		},
		get selectedRow() {
			return selectedRow;
		},
		get selectedColumn() {
			return selectedColumn;
		},
		get selectedCell() {
			return selectedCell;
		},
		get definition() {
			return definition;
		},
		get draft() {
			return draft;
		},
		set draft(value: string) {
			draft = value;
		}
	};
	setContext(key, owner);
	return owner;
}
export const useMatrixAuthoring = () => getContext<ReturnType<typeof provideMatrixAuthoring>>(key);
