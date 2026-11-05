import { getContext, setContext, onDestroy, untrack } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { ApplicationRuntimeReader, runtimeState } from './runtime';
import { createRuntimeTransport } from './runtime-transport';
import { invokeMediaEdit } from '../media/operations';
const key = Symbol('mapgyver-application-runtime');
/** One event subscription per workbench; document editors never duplicate process supervision. */
export function provideApplicationRuntime() {
	const session = $derived(appState.session),
		state = $state(runtimeState());
	const reader = new ApplicationRuntimeReader<WorkbenchSession>(
		() => session,
		state,
		invokeMediaEdit,
		createRuntimeTransport()
	);
	$effect(() => {
		void session?.status;
		void session?.snapshotGeneration;
		void session?.loading.visible;
		untrack(() => reader.observe());
	});
	onDestroy(() => reader.dispose());
	setContext(key, state);
	return state;
}
export const useApplicationRuntime = () =>
	getContext<ReturnType<typeof provideApplicationRuntime>>(key);
