import { onDestroy, untrack } from 'svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { createUiEditSession } from 'golden_ui/store/ui-intents';
import { createLatestEditGestureState } from 'golden_ui/store/latest-edit-gesture';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { createCellGesture, type CellHandle, type CellPresentation } from './cell-gesture';
import { invokeMediaEdit } from '../media/operations';
import type { CellLayoutReader } from './cell-layout';

export function createCellEditor(
	reader: CellLayoutReader<WorkbenchSession>,
	present: () => boolean
) {
	const state = $state(createLatestEditGestureState());
	const panel = usePanelVisibility();
	const projection = createEditorParameterProjection(() => appState.session);
	let activeCell = $state<string | null>(null);
	let presentation = $state.raw<CellPresentation | null>(null);
	const gesture = createCellGesture<WorkbenchSession>(() => appState.session, state, {
		group: (session, handle) =>
			createUiEditSession(`${handle.kind} Cell`, 'matrix-cell-geometry', session),
		invoke: (session, edit) => invokeMediaEdit(session, edit, projection.applied()),
		accepted: (result) => reader.acceptCell(result),
		present: (value) => {
			presentation = value;
		},
		active: (active) => {
			reader.setInteracting(active);
			if (!active) activeCell = null;
		},
		finished: () => reader.refresh()
	});
	$effect(() => {
		const session = appState.session;
		if (session) {
			void session.status;
			void session.loading.visible;
			void session.snapshotGeneration;
			void session.projectGeneration;
			void session.hasActiveEditSession;
			void session.editSessionEpoch;
		}
		const available = present() && panel.visible,
			selected = session?.selectedNodeUuid;
		untrack(() => {
			gesture.observe();
			if (!available || (activeCell && selected !== activeCell)) gesture.stop();
		});
	});
	onDestroy(() => gesture.dispose());
	return {
		state,
		gesture,
		projection,
		get presentation() {
			return presentation;
		},
		get activeCell() {
			return activeCell;
		},
		start(cell: string, handle: CellHandle) {
			const media = reader.page?.media;
			if (!media || reader.busy || !present() || !gesture.start({ media, cell }, handle))
				return false;
			activeCell = cell;
			return true;
		}
	};
}
