<script lang="ts">
	import type { Snippet } from 'svelte';

	import WorkbenchSessionRoot from '../app/WorkbenchSessionRoot.svelte';
	import { useApplicationHost } from '../../host/application-context';
	const host = useApplicationHost();

	let { children } = $props<{
		children?: Snippet;
	}>();
</script>

<WorkbenchSessionRoot
	application={host.application}
	httpBaseUrl={host.endpoints.httpBaseUrl}
	wsUrl={host.endpoints.wsUrl}
	enableGlobalShortcuts={false}>
	{#if children}
		{@render children()}
	{/if}
</WorkbenchSessionRoot>
