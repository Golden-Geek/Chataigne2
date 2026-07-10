<script lang="ts">
	import '../../style/golden-core.css';

	import { onMount } from 'svelte';
	import type { Snippet } from 'svelte';

	import {
		appState,
		createWorkbenchSession,
		type WorkbenchSession,
		type WorkbenchSessionOptions
	} from '../../store/workbench.svelte';
	import { mountWindowExitHandling } from '../../store/window-exit.svelte';
	import WorkbenchLoadingOverlay from './WorkbenchLoadingOverlay.svelte';

	let {
		wsUrl,
		httpBaseUrl,
		pollIntervalMs,
		scope,
		bootstrapRetryMs,
		transportFactory,
		enableGlobalShortcuts = true,
		children
	} = $props<{
		wsUrl?: string;
		httpBaseUrl?: string;
		pollIntervalMs?: number;
		scope?: WorkbenchSessionOptions['scope'];
		bootstrapRetryMs?: number;
		transportFactory?: WorkbenchSessionOptions['transportFactory'];
		enableGlobalShortcuts?: boolean;
		children?: Snippet;
	}>();

	let preserveSessionOnDestroy = false;

	const createSession = (): WorkbenchSession =>
		createWorkbenchSession({
			wsUrl: wsUrl ?? 'ws://localhost:7010/api/ui/ws',
			httpBaseUrl: httpBaseUrl ?? 'http://localhost:7010/api/ui',
			pollIntervalMs: pollIntervalMs ?? 120,
			scope,
			bootstrapRetryMs: bootstrapRetryMs ?? 1000,
			transportFactory,
			enableGlobalShortcuts
		});

	const hotData = import.meta.hot?.data as { workbenchSession?: WorkbenchSession } | undefined;
	const session: WorkbenchSession = hotData?.workbenchSession ?? createSession();

	if (import.meta.hot) {
		import.meta.hot.dispose((data) => {
			preserveSessionOnDestroy = true;
			data.workbenchSession = session;
		});
	}

	appState.session = session;

	onMount(() => {
		const cleanup = session.mount();
		const closeHandlingCleanup = mountWindowExitHandling();

		return () => {
			closeHandlingCleanup();
			if (!preserveSessionOnDestroy) {
				cleanup();
			}
			if (!preserveSessionOnDestroy && appState.session === session) {
				appState.session = null;
			}
		};
	});
</script>

{#if children}
	{@render children()}
{/if}

<WorkbenchLoadingOverlay {session} />
