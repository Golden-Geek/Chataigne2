import { onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { SpatialBindingReader, bindingState } from './binding';
import { invokeMediaEdit } from '../media/operations';
import { MEDIA_NAMESPACE } from '../media/generated/constants';
import type { SpatialEditorQuery } from '../media/generated/SpatialEditorQuery';
export function createSpatialBinding(query: () => SpatialEditorQuery) {
	const session = $derived(appState.session),
		state = $state(bindingState());
	const reader = new SpatialBindingReader<WorkbenchSession>(() => session, state, invokeMediaEdit);
	$effect(() => {
		const request = query(),
			current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.projectGeneration;
			if (!current.hasActiveEditSession) void current.currentHistoryStateId;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => reader.refresh(request));
	});
	onDestroy(() => reader.dispose());
	return {
		state,
		get binding() {
			return reader.binding;
		}
	};
}
