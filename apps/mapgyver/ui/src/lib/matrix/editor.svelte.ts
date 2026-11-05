import { onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { MatrixController, matrixState } from './authoring';
import { invokeMediaEdit } from '../media/operations';
import { MEDIA_NAMESPACE, MATRIX_PLAYBACK_EVENT } from '../media/generated/constants';
import { usePreview } from '../preview/context.svelte';
import { previewKey } from '../preview/identity';
import { matrixFeedback } from './render-feedback';
import { useDocumentSelection } from '../selection.svelte';
import { useApplicationRuntime } from '../applications/runtime-context.svelte';
import type { MatrixEditorQuery } from '../media/generated/MatrixEditorQuery';
import type { MatrixEdit } from '../media/generated/MatrixEdit';
export function createMatrixEditor(query: () => MatrixEditorQuery) {
	const session = $derived(appState.session),
		view = $state(matrixState());
	const controller = new MatrixController<WorkbenchSession>(() => session, view, invokeMediaEdit);
	const preview = usePreview(),
		runtime = useApplicationRuntime(),
		selection = useDocumentSelection();
	const window = $derived(controller.window);
	const observation = $derived(
		matrixFeedback(
			window,
			window
				? preview.statuses.get(
						previewKey({ kind: 'Channel', reference: window.final_output.reference })
					)
				: undefined,
			session?.status === 'connected' && !session.loading.visible
		)
	);
	$effect(() => {
		const current = session,
			request = query();
		void view.pending;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
			void current.getCustomEventSequence(MATRIX_PLAYBACK_EVENT);
		}
		untrack(() => {
			if (!current?.hasActiveEditSession) controller.observe(request);
		});
	});
	onDestroy(() => controller.dispose());
	return {
		view,
		controller,
		async edit(intent: MatrixEdit) {
			const selectParent = selection.capture(),
				parent = window?.final_output.reference.channel;
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
		get session() {
			return session;
		},
		get busy() {
			return controller.busy || session?.hasActiveEditSession === true;
		},
		get matrixWindow() {
			return window;
		},
		get observation() {
			return observation;
		},
		get applicationRuntime() {
			return runtime.live && runtime.snapshot?.project === window?.project
				? runtime.snapshot
				: null;
		}
	};
}
