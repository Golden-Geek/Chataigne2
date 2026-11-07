import { onDestroy } from 'svelte';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { projectMatrixWindow } from '../spatial/parameter-projection';
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
	const panel = usePanelVisibility();
	const session = $derived(appState.session),
		view = $state(matrixState());
	const controller = new MatrixController<WorkbenchSession>(() => session, view, invokeMediaEdit);
	const preview = usePreview(),
		runtime = useApplicationRuntime(),
		selection = useDocumentSelection();
	const projection = createEditorParameterProjection(() => session);
	const parameters = $derived(
		panel.visible && view.page ? projection.read(view.generation, view.page.at) : null
	);
	const bindings = $derived(
		new Map(view.page?.parameters.map((binding) => [binding.item, binding]) ?? [])
	);
	const window = $derived(
		controller.window && parameters && !view.interacting
			? projectMatrixWindow(controller.window, bindings, parameters)
			: controller.window
	);
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
	observeEditor({
		query,
		topics: [MEDIA_NAMESPACE, MATRIX_PLAYBACK_EVENT],
		pending: () => view.pending,
		interacting: () => view.interacting,
		owners: () => (controller.window ? [controller.window.media] : []),
		refresh: (request, resolve) => controller.observe(request, resolve)
	});
	onDestroy(() => controller.dispose());
	return {
		view,
		controller,
		projection,
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
