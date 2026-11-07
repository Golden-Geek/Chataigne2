import { onDestroy } from 'svelte';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { SpatialBindingReader, bindingState } from './binding';
import { invokeMediaEdit } from '../media/operations';
import { MEDIA_NAMESPACE } from '../media/generated/constants';
import type { SpatialEditorQuery } from '../media/generated/SpatialEditorQuery';
export function createSpatialBinding(query: () => SpatialEditorQuery) {
	const session = $derived(appState.session),
		state = $state(bindingState());
	const reader = new SpatialBindingReader<WorkbenchSession>(() => session, state, invokeMediaEdit);
	observeEditor({
		query,
		topics: [MEDIA_NAMESPACE],
		refresh: (request) => reader.refresh(request)
	});
	onDestroy(() => reader.dispose());
	return {
		state,
		get binding() {
			return reader.binding;
		}
	};
}
